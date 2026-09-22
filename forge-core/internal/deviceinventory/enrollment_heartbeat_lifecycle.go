package deviceinventory

import (
	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceidentity"
)

// EnrollmentHeartbeatLifecycleSchemaVersion identifies the joined, value-only
// identity -> heartbeat -> inventory transition. It is deliberately separate
// from the live enrollment and heartbeat API contracts.
const EnrollmentHeartbeatLifecycleSchemaVersion = "forge.device-enrollment-heartbeat-lifecycle/v1"

// EnrollmentHeartbeatLifecycleEvaluationMode documents that this adapter only
// computes a replacement plan from caller-supplied values. It does not verify
// a signature, consume a challenge, read a clock, write storage, or publish
// authoritative inventory.
const EnrollmentHeartbeatLifecycleEvaluationMode = "pure_joined_value_transition"

// LifecycleAuthority is intentionally all false. A successful identity proof
// is still only an input to this offline transition and does not grant any
// production authority.
type LifecycleAuthority struct {
	IdentityVerified    bool `json:"identity_verified"`
	ChallengeConsumed   bool `json:"challenge_consumed"`
	HeartbeatPersisted  bool `json:"heartbeat_persisted"`
	InventoryAuthority  bool `json:"inventory_authoritative"`
	ReservationCreated  bool `json:"reservation_created"`
	ExecutionAuthorized bool `json:"execution_authorized"`
	DispatchPerformed   bool `json:"dispatch_performed"`
}

// EnrollmentHeartbeatLifecycleInput is a complete caller-supplied transition
// image. Current state is optional at revision zero; all timestamps and CAS
// revisions are explicit so the function remains deterministic.
type EnrollmentHeartbeatLifecycleInput struct {
	Owner                     deviceidentity.Owner
	Device                    deviceidentity.DeviceBinding
	Challenge                 deviceidentity.Challenge
	Proof                     deviceidentity.Proof
	Heartbeat                 deviceheartbeat.Heartbeat
	CurrentHeartbeat          *deviceheartbeat.PersistedInstance
	ExpectedHeartbeatRevision uint64
	CurrentInventory          *PersistedInventoryState
	ExpectedInventoryRevision uint64
	CordonState               string
	ReservationState          string
	ServerObservedAtMS        uint64
	LeaseTTLMS                uint64
	IdentityNowMS             uint64
	EvaluatedAtMS             uint64
	StaleAfterMS              uint64
}

// EnrollmentHeartbeatLifecycleResult is a metadata-only replacement plan.
// The returned heartbeat and inventory values are not persisted by this
// package; callers must not treat them as authoritative state.
type EnrollmentHeartbeatLifecycleResult struct {
	IdentityBound    bool
	ApprovalRequired bool
	IdentityReason   string
	Heartbeat        deviceheartbeat.PersistedInstance
	Inventory        PersistedInventoryState
	Projection       PersistedInventoryProjection
	PreviewOnly      bool
	Authority        LifecycleAuthority
}

// LifecycleError is a stable joined-transition rejection reason.
type LifecycleError string

const (
	ErrApprovalRequired LifecycleError = "approval_required"
	ErrOwnerMismatch    LifecycleError = "owner_mismatch"
)

func (e LifecycleError) Error() string { return string(e) }

// ObserveEnrollmentHeartbeatLifecycle joins the existing pure identity,
// heartbeat CAS, inventory CAS, and fixed-time projection contracts. It
// computes no side effects and returns an error before exposing any partial
// replacement when one boundary rejects the image.
func ObserveEnrollmentHeartbeatLifecycle(
	input EnrollmentHeartbeatLifecycleInput,
) (EnrollmentHeartbeatLifecycleResult, error) {
	decision, err := deviceidentity.Evaluate(
		input.Owner,
		input.Device,
		input.Challenge,
		input.Proof,
		input.IdentityNowMS,
	)
	if err != nil {
		return EnrollmentHeartbeatLifecycleResult{}, err
	}
	if decision.ApprovalRequired {
		return EnrollmentHeartbeatLifecycleResult{}, ErrApprovalRequired
	}
	if input.Owner != input.Device.Owner {
		return EnrollmentHeartbeatLifecycleResult{}, ErrOwnerMismatch
	}

	device := deviceheartbeat.Device{
		DeviceID:      input.Device.DeviceID,
		TenantID:      input.Owner.TenantID,
		ApprovalState: input.Device.ApprovalState,
	}
	nextHeartbeat, err := deviceheartbeat.Commit(
		device,
		input.CurrentHeartbeat,
		input.ExpectedHeartbeatRevision,
		input.Heartbeat,
		input.ServerObservedAtMS,
		input.LeaseTTLMS,
	)
	if err != nil {
		return EnrollmentHeartbeatLifecycleResult{}, err
	}

	owner := SnapshotOwner{
		Issuer:   input.Owner.Issuer,
		Subject:  input.Owner.Subject,
		TenantID: input.Owner.TenantID,
	}
	instance := nextHeartbeat.Instance
	runner := RunnerInstanceRecord{
		DeviceID:                   instance.DeviceID,
		InstanceID:                 instance.InstanceID,
		Generation:                 instance.Generation,
		HeartbeatSequence:          instance.HeartbeatSequence,
		ServerObservedAtMS:         instance.ServerObservedAtMS,
		CapabilityLeaseExpiresAtMS: instance.CapabilityLeaseExpiresAtMS,
		Liveness:                   "online",
		Capabilities:               instance.Capabilities,
	}
	deviceRecord := DeviceRecord{
		DeviceID:         input.Device.DeviceID,
		Owner:            owner,
		ApprovalState:    input.Device.ApprovalState,
		CordonState:      input.CordonState,
		ReservationState: input.ReservationState,
	}
	nextInventory, err := CommitPersistedInventory(
		input.CurrentInventory,
		input.ExpectedInventoryRevision,
		deviceRecord,
		runner,
	)
	if err != nil {
		return EnrollmentHeartbeatLifecycleResult{}, err
	}
	projection, err := ProjectPersistedInventory(
		nextInventory,
		owner,
		input.EvaluatedAtMS,
		input.StaleAfterMS,
	)
	if err != nil {
		return EnrollmentHeartbeatLifecycleResult{}, err
	}

	return EnrollmentHeartbeatLifecycleResult{
		IdentityBound:    decision.IdentityBound,
		ApprovalRequired: decision.ApprovalRequired,
		IdentityReason:   decision.Reason,
		Heartbeat:        nextHeartbeat,
		Inventory:        nextInventory,
		Projection:       projection,
		PreviewOnly:      true,
		Authority:        LifecycleAuthority{},
	}, nil
}
