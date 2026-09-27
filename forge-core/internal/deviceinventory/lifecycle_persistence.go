package deviceinventory

import (
	"math"
	"strings"

	"forgeos/forge-core/internal/deviceapproval"
	"forgeos/forge-core/internal/devicecredential"
	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceidentity"
)

// PersistedEnrollmentHeartbeatLifecycleState is the complete replacement
// image produced by a future lifecycle transaction.  It deliberately keeps
// the human/device binding, heartbeat observation, and inventory observation
// in one value so a restart cannot expose a heartbeat from one image with an
// inventory projection from another image.  This type has no storage handle,
// clock, credential, listener, or execution authority.
type PersistedEnrollmentHeartbeatLifecycleState struct {
	Revision            uint64                            `json:"revision"`
	Owner               deviceidentity.Owner              `json:"owner"`
	Device              deviceidentity.DeviceBinding      `json:"device"`
	Heartbeat           deviceheartbeat.PersistedInstance `json:"heartbeat"`
	Inventory           PersistedInventoryState           `json:"inventory"`
	ApprovalCandidate   *deviceapproval.State             `json:"approval_candidate,omitempty"`
	CredentialCandidate *devicecredential.State           `json:"credential_candidate,omitempty"`
	ChallengeCandidate  *deviceidentity.Challenge         `json:"challenge_candidate,omitempty"`
}

// LifecyclePersistenceError is a stable value-level result for the future
// durable lifecycle adapter.  It is intentionally separate from the
// heartbeat and inventory CAS errors so a caller can tell which aggregate
// rejected a replacement without treating that result as a storage write.
type LifecyclePersistenceError string

const (
	ErrLifecycleRevisionConflict   LifecyclePersistenceError = "lifecycle_revision_conflict"
	ErrLifecycleInvalidState       LifecyclePersistenceError = "lifecycle_invalid_state"
	ErrLifecycleRevisionOverflow   LifecyclePersistenceError = "lifecycle_revision_overflow"
	ErrLifecycleBindingChanged     LifecyclePersistenceError = "lifecycle_binding_changed"
	ErrLifecycleServerStateChanged LifecyclePersistenceError = "lifecycle_server_state_changed"
)

func (e LifecyclePersistenceError) Error() string { return string(e) }

// RestorePersistedEnrollmentHeartbeatLifecycle validates and copies one
// complete lifecycle image returned by a future storage adapter.  The owner
// and device identity are immutable within this image, and heartbeat and
// inventory revisions must advance together.  No I/O or authority is
// acquired.
func RestorePersistedEnrollmentHeartbeatLifecycle(
	revision uint64,
	owner deviceidentity.Owner,
	device deviceidentity.DeviceBinding,
	heartbeat deviceheartbeat.PersistedInstance,
	inventory PersistedInventoryState,
) (PersistedEnrollmentHeartbeatLifecycleState, error) {
	if revision == 0 || !validLifecycleOwner(owner) || !validLifecycleDevice(device, owner) {
		return PersistedEnrollmentHeartbeatLifecycleState{}, ErrLifecycleInvalidState
	}
	if err := deviceheartbeat.ValidatePersistedInstance(heartbeat); err != nil {
		return PersistedEnrollmentHeartbeatLifecycleState{}, ErrLifecycleInvalidState
	}
	clonedHeartbeat, err := clonePersistedHeartbeat(heartbeat)
	if err != nil {
		return PersistedEnrollmentHeartbeatLifecycleState{}, ErrLifecycleInvalidState
	}
	clonedInventory, err := RestorePersistedInventory(
		inventory.Revision,
		inventory.Device,
		inventory.Runner,
	)
	if err != nil {
		return PersistedEnrollmentHeartbeatLifecycleState{}, ErrLifecycleInvalidState
	}
	if revision != heartbeat.Revision || revision != inventory.Revision ||
		clonedHeartbeat.Instance.DeviceID != device.DeviceID ||
		clonedInventory.Device.DeviceID != device.DeviceID ||
		clonedInventory.Device.Owner != lifecycleSnapshotOwner(owner) ||
		clonedInventory.Device.ApprovalState != device.ApprovalState ||
		clonedInventory.Runner.DeviceID != device.DeviceID ||
		clonedInventory.Runner.InstanceID != clonedHeartbeat.Instance.InstanceID ||
		clonedInventory.Runner.Generation != clonedHeartbeat.Instance.Generation ||
		clonedInventory.Runner.HeartbeatSequence != clonedHeartbeat.Instance.HeartbeatSequence ||
		clonedInventory.Runner.ServerObservedAtMS != clonedHeartbeat.Instance.ServerObservedAtMS ||
		clonedInventory.Runner.CapabilityLeaseExpiresAtMS != clonedHeartbeat.Instance.CapabilityLeaseExpiresAtMS ||
		!capabilitySnapshotsEqual(clonedInventory.Runner.Capabilities, clonedHeartbeat.Instance.Capabilities) {
		return PersistedEnrollmentHeartbeatLifecycleState{}, ErrLifecycleInvalidState
	}
	return PersistedEnrollmentHeartbeatLifecycleState{
		Revision:  revision,
		Owner:     owner,
		Device:    device,
		Heartbeat: clonedHeartbeat,
		Inventory: clonedInventory,
	}, nil
}

// CommitPersistedEnrollmentHeartbeatLifecycle computes one complete
// replacement under a single outer revision.  The current value is treated
// as the only source of nested heartbeat/inventory revisions; this prevents a
// caller from combining a fresh outer revision with stale nested CAS inputs.
// The returned state is a value image only and is never written by this
// package.
func CommitPersistedEnrollmentHeartbeatLifecycle(
	current *PersistedEnrollmentHeartbeatLifecycleState,
	expectedRevision uint64,
	input EnrollmentHeartbeatLifecycleInput,
) (PersistedEnrollmentHeartbeatLifecycleState, EnrollmentHeartbeatLifecycleResult, error) {
	actualRevision := uint64(0)
	if current != nil {
		if err := validatePersistedEnrollmentHeartbeatLifecycle(*current); err != nil {
			return PersistedEnrollmentHeartbeatLifecycleState{}, EnrollmentHeartbeatLifecycleResult{}, err
		}
		actualRevision = current.Revision
	}
	if expectedRevision != actualRevision {
		return PersistedEnrollmentHeartbeatLifecycleState{}, EnrollmentHeartbeatLifecycleResult{}, ErrLifecycleRevisionConflict
	}
	if actualRevision == math.MaxUint64 {
		return PersistedEnrollmentHeartbeatLifecycleState{}, EnrollmentHeartbeatLifecycleResult{}, ErrLifecycleRevisionOverflow
	}
	if current != nil {
		if input.Owner != current.Owner || input.Device != current.Device {
			return PersistedEnrollmentHeartbeatLifecycleState{}, EnrollmentHeartbeatLifecycleResult{}, ErrLifecycleBindingChanged
		}
		if input.CordonState != current.Inventory.Device.CordonState ||
			input.ReservationState != current.Inventory.Device.ReservationState {
			return PersistedEnrollmentHeartbeatLifecycleState{}, EnrollmentHeartbeatLifecycleResult{}, ErrLifecycleServerStateChanged
		}
		input.CurrentHeartbeat = &current.Heartbeat
		input.ExpectedHeartbeatRevision = current.Heartbeat.Revision
		input.CurrentInventory = &current.Inventory
		input.ExpectedInventoryRevision = current.Inventory.Revision
	} else {
		input.CurrentHeartbeat = nil
		input.ExpectedHeartbeatRevision = 0
		input.CurrentInventory = nil
		input.ExpectedInventoryRevision = 0
	}
	result, err := ObserveEnrollmentHeartbeatLifecycle(input)
	if err != nil {
		return PersistedEnrollmentHeartbeatLifecycleState{}, EnrollmentHeartbeatLifecycleResult{}, err
	}
	next, err := RestorePersistedEnrollmentHeartbeatLifecycle(
		actualRevision+1,
		input.Owner,
		input.Device,
		result.Heartbeat,
		result.Inventory,
	)
	if err != nil {
		return PersistedEnrollmentHeartbeatLifecycleState{}, EnrollmentHeartbeatLifecycleResult{}, err
	}
	if current != nil {
		next.ApprovalCandidate = cloneApprovalCandidate(current.ApprovalCandidate)
		next.CredentialCandidate = cloneCredentialCandidate(current.CredentialCandidate)
		next.ChallengeCandidate = cloneChallengeCandidate(current.ChallengeCandidate)
	}
	return next, result, nil
}

func validatePersistedEnrollmentHeartbeatLifecycle(value PersistedEnrollmentHeartbeatLifecycleState) error {
	_, err := RestorePersistedEnrollmentHeartbeatLifecycle(
		value.Revision,
		value.Owner,
		value.Device,
		value.Heartbeat,
		value.Inventory,
	)
	if err != nil {
		return err
	}
	if err := validateApprovalCandidate(value.ApprovalCandidate, value.Owner, value.Device); err != nil {
		return err
	}
	if err := validateCredentialCandidate(value.CredentialCandidate, value.Owner, value.Device); err != nil {
		return err
	}
	if err := validateChallengeCandidate(value.ChallengeCandidate); err != nil {
		return err
	}
	return nil
}

// validateApprovalCandidate keeps the owner/device binding of an approval
// plan explicit while leaving the live DeviceBinding immutable.  The pointer
// is intentionally optional so existing lifecycle v1 images remain byte and
// behavior compatible.  A candidate is a value plan only; it does not grant
// device, inventory, credential, or execution authority.
func validateApprovalCandidate(
	candidate *deviceapproval.State,
	owner deviceidentity.Owner,
	device deviceidentity.DeviceBinding,
) error {
	if candidate == nil {
		return nil
	}
	if err := candidate.Validate(); err != nil || candidate.Owner != owner || candidate.DeviceID != device.DeviceID {
		return ErrLifecycleInvalidState
	}
	return nil
}

func cloneApprovalCandidate(candidate *deviceapproval.State) *deviceapproval.State {
	if candidate == nil {
		return nil
	}
	copy := *candidate
	return &copy
}

// validateCredentialCandidate keeps the owner/device binding of a credential
// metadata plan explicit. Credential material is absent from this value and
// the candidate never changes the live DeviceBinding.
func validateCredentialCandidate(
	candidate *devicecredential.State,
	owner deviceidentity.Owner,
	device deviceidentity.DeviceBinding,
) error {
	if candidate == nil {
		return nil
	}
	if err := candidate.Validate(); err != nil || candidate.Owner != owner || candidate.DeviceID != device.DeviceID {
		return ErrLifecycleInvalidState
	}
	return nil
}

func cloneCredentialCandidate(candidate *devicecredential.State) *devicecredential.State {
	if candidate == nil {
		return nil
	}
	copy := *candidate
	return &copy
}

// validateChallengeCandidate keeps one issued challenge as a value-only
// candidate attached to the already owner/device-bound lifecycle image. The
// enclosing state supplies the owner and device scope; this value validates
// only the challenge shape and expiry window.
func validateChallengeCandidate(candidate *deviceidentity.Challenge) error {
	if candidate == nil {
		return nil
	}
	if err := candidate.Validate(); err != nil {
		return ErrLifecycleInvalidState
	}
	return nil
}

func cloneChallengeCandidate(candidate *deviceidentity.Challenge) *deviceidentity.Challenge {
	if candidate == nil {
		return nil
	}
	copy := *candidate
	return &copy
}

func validLifecycleOwner(owner deviceidentity.Owner) bool {
	return validSnapshotOwner(lifecycleSnapshotOwner(owner))
}

func validLifecycleDevice(device deviceidentity.DeviceBinding, owner deviceidentity.Owner) bool {
	if device.DeviceID == "" || device.KeyID == "" ||
		device.PublicKeySHA256 == "" || device.Owner != owner ||
		!validSnapshotIdentifier(device.DeviceID) || !validSnapshotIdentifier(device.KeyID) ||
		!validLifecycleDigest(device.PublicKeySHA256) {
		return false
	}
	if device.ApprovalState != "pending" && device.ApprovalState != "approved" && device.ApprovalState != "revoked" {
		return false
	}
	return device.CredentialState == "active" || device.CredentialState == "expired" || device.CredentialState == "revoked"
}

func lifecycleSnapshotOwner(owner deviceidentity.Owner) SnapshotOwner {
	return SnapshotOwner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
}

func validLifecycleDigest(value string) bool {
	return len(value) == deviceidentity.DigestHexBytes && strings.Trim(value, "0123456789abcdef") == ""
}

func clonePersistedHeartbeat(value deviceheartbeat.PersistedInstance) (deviceheartbeat.PersistedInstance, error) {
	capabilities, err := deviceheartbeat.NewCapabilitySnapshot(
		value.Instance.Capabilities.OperatingSystem,
		value.Instance.Capabilities.Architecture,
		value.Instance.Capabilities.CPUCores,
		value.Instance.Capabilities.AvailableCPUCores,
		value.Instance.Capabilities.MemoryBytes,
		value.Instance.Capabilities.AvailableMemoryBytes,
		value.Instance.Capabilities.StorageBytes,
		value.Instance.Capabilities.AvailableStorageBytes,
		value.Instance.Capabilities.GPUs,
		value.Instance.Capabilities.Runtimes,
	)
	if err != nil || !capabilitySnapshotsEqual(value.Instance.Capabilities, capabilities) {
		return deviceheartbeat.PersistedInstance{}, ErrLifecycleInvalidState
	}
	value.Instance.Capabilities = capabilities
	return value, nil
}
