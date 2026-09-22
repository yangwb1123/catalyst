package deviceplacement

import (
	"errors"

	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceinventory"
)

var errInvalidPersistedInventoryPlacementInput = errors.New("invalid_persisted_inventory_placement_input")
var errUnsupportedPersistedInventoryPlacementEvaluation = errors.New("unsupported_persisted_placement_capability")

// PersistedInventoryPlacementStaleAfterMS matches the Rust registry's fixed
// heartbeat freshness boundary. The generic caller-supplied placement model
// permits a wider preview window, but persisted inventory must not silently
// use a different freshness policy across runtimes.
const PersistedInventoryPlacementStaleAfterMS int64 = 90_000

// PersistedInventoryPlacementInput is the value-only placement view of one
// persisted inventory row. It carries no route, store, clock, selection,
// reservation, or Runner authority. Owner and capability values remain
// unverified declarations.
//
// Persisted inventory has no residency, trust, sandbox, or concurrency data.
// Those attributes are therefore represented only by their closed defaults:
// empty residency and sandbox sets, unknown trust, and zero concurrency. A
// placement policy that requires any of those attributes must reject this
// input rather than infer a value.
type PersistedInventoryPlacementInput struct {
	Revision                   uint64                             `json:"revision"`
	DeviceID                   string                             `json:"device_id"`
	InstanceID                 string                             `json:"instance_id"`
	Generation                 uint64                             `json:"generation"`
	HeartbeatSequence          uint64                             `json:"heartbeat_sequence"`
	Owner                      Owner                              `json:"owner"`
	ApprovalState              string                             `json:"approval_state"`
	CordonState                string                             `json:"cordon_state"`
	ReservationState           string                             `json:"reservation_state"`
	Liveness                   string                             `json:"liveness"`
	SnapshotObservedAtMS       int64                              `json:"snapshot_observed_at_ms"`
	LeaseExpiresAtMS           int64                              `json:"lease_expires_at_ms"`
	Capabilities               deviceheartbeat.CapabilitySnapshot `json:"capabilities"`
	DataResidencyZones         []string                           `json:"data_residency_zones"`
	TrustZone                  string                             `json:"trust_zone"`
	SandboxLevels              []string                           `json:"sandbox_levels"`
	ConcurrencyLimit           uint16                             `json:"concurrency_limit"`
	ActiveConcurrency          uint16                             `json:"active_concurrency"`
	OwnerDeclarationUnverified bool                               `json:"owner_declaration_unverified"`
	PolicyAttributesUnverified bool                               `json:"policy_attributes_unverified"`
}

// BuildPersistedInventoryPlacementInput validates and copies one persisted
// inventory value into a placement input for the exact declared owner. It is
// deliberately not a placement evaluator: callers must supply an independent
// policy evaluator and cannot obtain selection or authority from this value.
func BuildPersistedInventoryPlacementInput(
	value deviceinventory.PersistedInventoryState,
	evaluationOwner deviceinventory.SnapshotOwner,
) (PersistedInventoryPlacementInput, error) {
	// Rebuild the public persisted value so a caller-mutated value cannot bypass
	// its revision, binding, lease, or capability-canonicality invariants.
	canonical, err := deviceinventory.RestorePersistedInventory(value.Revision, value.Device, value.Runner)
	if err != nil {
		return PersistedInventoryPlacementInput{}, err
	}
	if canonical.Device.Owner != evaluationOwner {
		return PersistedInventoryPlacementInput{}, deviceinventory.ErrInventoryOwnerMismatch
	}
	if !validDeviceID(canonical.Device.Owner.TenantID) {
		return PersistedInventoryPlacementInput{}, errInvalidPersistedInventoryPlacementInput
	}
	if canonical.Runner.ServerObservedAtMS > uint64(MaxSafeIntegerMS) ||
		canonical.Runner.CapabilityLeaseExpiresAtMS > uint64(MaxSafeIntegerMS) {
		return PersistedInventoryPlacementInput{}, errInvalidPersistedInventoryPlacementInput
	}
	input := PersistedInventoryPlacementInput{
		Revision:             canonical.Revision,
		DeviceID:             canonical.Device.DeviceID,
		InstanceID:           canonical.Runner.InstanceID,
		Generation:           canonical.Runner.Generation,
		HeartbeatSequence:    canonical.Runner.HeartbeatSequence,
		Owner:                ownerFromSnapshot(canonical.Device.Owner),
		ApprovalState:        canonical.Device.ApprovalState,
		CordonState:          canonical.Device.CordonState,
		ReservationState:     canonical.Device.ReservationState,
		Liveness:             canonical.Runner.Liveness,
		SnapshotObservedAtMS: int64(canonical.Runner.ServerObservedAtMS),
		LeaseExpiresAtMS:     int64(canonical.Runner.CapabilityLeaseExpiresAtMS),
		Capabilities: deviceheartbeat.CapabilitySnapshot{
			OperatingSystem:       canonical.Runner.Capabilities.OperatingSystem,
			Architecture:          canonical.Runner.Capabilities.Architecture,
			CPUCores:              canonical.Runner.Capabilities.CPUCores,
			AvailableCPUCores:     canonical.Runner.Capabilities.AvailableCPUCores,
			MemoryBytes:           canonical.Runner.Capabilities.MemoryBytes,
			AvailableMemoryBytes:  canonical.Runner.Capabilities.AvailableMemoryBytes,
			StorageBytes:          canonical.Runner.Capabilities.StorageBytes,
			AvailableStorageBytes: canonical.Runner.Capabilities.AvailableStorageBytes,
			GPUs:                  append(make([]deviceheartbeat.GPUCapability, 0, len(canonical.Runner.Capabilities.GPUs)), canonical.Runner.Capabilities.GPUs...),
			Runtimes:              append(make([]string, 0, len(canonical.Runner.Capabilities.Runtimes)), canonical.Runner.Capabilities.Runtimes...),
		},
		DataResidencyZones:         []string{},
		TrustZone:                  "unknown",
		SandboxLevels:              []string{},
		ConcurrencyLimit:           0,
		ActiveConcurrency:          0,
		OwnerDeclarationUnverified: true,
		PolicyAttributesUnverified: true,
	}
	if err := ValidatePersistedInventoryPlacementInput(input); err != nil {
		return PersistedInventoryPlacementInput{}, err
	}
	return input, nil
}

// ValidatePersistedInventoryPlacementInput keeps this boundary narrow. A
// value with supplied policy attributes is rejected so a future source for
// those attributes cannot be silently confused with persisted inventory.
func ValidatePersistedInventoryPlacementInput(value PersistedInventoryPlacementInput) error {
	if value.SnapshotObservedAtMS < 0 || value.LeaseExpiresAtMS < 0 ||
		value.SnapshotObservedAtMS > MaxSafeIntegerMS || value.LeaseExpiresAtMS > MaxSafeIntegerMS ||
		!validDeviceID(value.Owner.TenantID) ||
		!value.OwnerDeclarationUnverified || !value.PolicyAttributesUnverified ||
		len(value.DataResidencyZones) != 0 || value.TrustZone != "unknown" ||
		len(value.SandboxLevels) != 0 || value.ConcurrencyLimit != 0 || value.ActiveConcurrency != 0 {
		return errInvalidPersistedInventoryPlacementInput
	}
	_, err := deviceinventory.RestorePersistedInventory(value.Revision,
		deviceinventory.DeviceRecord{
			DeviceID: value.DeviceID,
			Owner: deviceinventory.SnapshotOwner{
				Issuer: value.Owner.Issuer, Subject: value.Owner.Subject, TenantID: value.Owner.TenantID,
			},
			ApprovalState: value.ApprovalState, CordonState: value.CordonState, ReservationState: value.ReservationState,
		},
		deviceinventory.RunnerInstanceRecord{
			DeviceID: value.DeviceID, InstanceID: value.InstanceID, Generation: value.Generation, HeartbeatSequence: value.HeartbeatSequence,
			ServerObservedAtMS: uint64(value.SnapshotObservedAtMS), CapabilityLeaseExpiresAtMS: uint64(value.LeaseExpiresAtMS),
			Liveness: value.Liveness, Capabilities: value.Capabilities,
		})
	return err
}

func ownerFromSnapshot(value deviceinventory.SnapshotOwner) Owner {
	return Owner{Issuer: value.Issuer, Subject: value.Subject, TenantID: value.TenantID}
}

// PersistedInventoryPlacementEvaluation is one offline comparison of a
// persisted inventory value. It carries no target selection or authority.
type PersistedInventoryPlacementEvaluation struct {
	Revision                   uint64   `json:"revision"`
	DeviceID                   string   `json:"device_id"`
	InstanceID                 string   `json:"instance_id"`
	MatchesRequirements        bool     `json:"matches_requirements"`
	ExclusionReasons           []string `json:"exclusion_reasons"`
	OwnerDeclarationUnverified bool     `json:"owner_declaration_unverified"`
	DeviceAttributesUnverified bool     `json:"device_attributes_unverified"`
}

// EvaluatePersistedInventoryPlacementInput adapts one already validated,
// owner-bound persisted inventory value to the existing pure placement
// evaluator. It uses the persisted owner tenant and caller-supplied fixed
// evaluation time; it never selects, reserves, dispatches, or executes.
func EvaluatePersistedInventoryPlacementInput(
	input PersistedInventoryPlacementInput,
	requirements Requirements,
	evaluatedAtMS int64,
) (PersistedInventoryPlacementEvaluation, error) {
	if err := ValidatePersistedInventoryPlacementInput(input); err != nil {
		return PersistedInventoryPlacementEvaluation{}, err
	}
	device, err := persistedInventoryPlacementDevice(input)
	if err != nil {
		return PersistedInventoryPlacementEvaluation{}, err
	}
	result, err := Evaluate(Request{
		SchemaVersion:    RequestSchemaVersion,
		EvaluatedAtMS:    evaluatedAtMS,
		Owner:            input.Owner,
		MaxSnapshotAgeMS: PersistedInventoryPlacementStaleAfterMS,
		Requirements:     requirements,
		Devices:          []Device{device},
	})
	if err != nil {
		return PersistedInventoryPlacementEvaluation{}, err
	}
	decision := result.DeviceResults[0]
	return PersistedInventoryPlacementEvaluation{
		Revision:                   input.Revision,
		DeviceID:                   input.DeviceID,
		InstanceID:                 input.InstanceID,
		MatchesRequirements:        decision.MatchesRequirements,
		ExclusionReasons:           append([]string(nil), decision.ExclusionReasons...),
		OwnerDeclarationUnverified: input.OwnerDeclarationUnverified,
		DeviceAttributesUnverified: decision.AttributesUnverified,
	}, nil
}

func persistedInventoryPlacementDevice(input PersistedInventoryPlacementInput) (Device, error) {
	if len(input.Capabilities.GPUs) != 0 {
		return Device{}, errUnsupportedPersistedInventoryPlacementEvaluation
	}
	return Device{
		DeviceID:             input.DeviceID,
		Owner:                input.Owner,
		ApprovalState:        input.ApprovalState,
		CordonState:          input.CordonState,
		Liveness:             input.Liveness,
		SnapshotObservedAtMS: input.SnapshotObservedAtMS,
		LeaseExpiresAtMS:     input.LeaseExpiresAtMS,
		OS:                   input.Capabilities.OperatingSystem,
		Architecture:         input.Capabilities.Architecture,
		AvailableCPUCores:    input.Capabilities.AvailableCPUCores,
		AvailableMemoryBytes: input.Capabilities.AvailableMemoryBytes,
		AvailableStorage:     input.Capabilities.AvailableStorageBytes,
		Runtimes:             append([]string(nil), input.Capabilities.Runtimes...),
		GPU:                  GPUDeclaration{},
		DataResidencyZones:   []string{},
		TrustZone:            "unknown",
		SandboxLevels:        []string{},
		ConcurrencyLimit:     0,
		ActiveConcurrency:    0,
	}, nil
}

const persistedInventoryPlacementBatchSchemaVersion = "forge.persisted-inventory-placement-evaluation/v1"

// PersistedInventoryPlacementBatchRequest evaluates all supplied persisted
// inputs in one deterministic, value-only comparison. Inputs are owner-bound
// declarations and do not establish identity or inventory authority.
type PersistedInventoryPlacementBatchRequest struct {
	Owner         Owner                              `json:"owner"`
	EvaluatedAtMS int64                              `json:"evaluated_at_ms"`
	Requirements  Requirements                       `json:"requirements"`
	Inputs        []PersistedInventoryPlacementInput `json:"inputs"`
}

// PersistedInventoryPlacementBatchEvaluation contains every candidate
// decision. It never selects, reserves, dispatches, or executes a target.
type PersistedInventoryPlacementBatchEvaluation struct {
	SchemaVersion              string                                     `json:"schema_version"`
	EvaluationMode             string                                     `json:"evaluation_mode"`
	Owner                      Owner                                      `json:"owner_declaration"`
	EvaluatedAtMS              int64                                      `json:"evaluated_at_ms"`
	OwnerDeclarationUnverified bool                                       `json:"owner_declaration_unverified"`
	DeviceAttributesUnverified bool                                       `json:"device_attributes_unverified"`
	Decisions                  []PersistedInventoryPlacementBatchDecision `json:"decisions"`
	SelectedDeviceID           *string                                    `json:"selected_device_id"`
	SelectedInstanceID         *string                                    `json:"selected_instance_id"`
	Authority                  PersistedInventoryPlacementBatchAuthority  `json:"authority"`
}

type PersistedInventoryPlacementBatchDecision struct {
	Revision            uint64   `json:"revision"`
	DeviceID            string   `json:"device_id"`
	InstanceID          string   `json:"instance_id"`
	MatchesRequirements bool     `json:"matches_requirements"`
	ExclusionReasons    []string `json:"exclusion_reasons"`
}

type PersistedInventoryPlacementBatchAuthority struct {
	IdentityVerified       bool `json:"identity_verified"`
	HeartbeatPersisted     bool `json:"heartbeat_persisted"`
	InventoryAuthoritative bool `json:"inventory_authoritative"`
	PlacementSelected      bool `json:"placement_selected"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
}

// EvaluatePersistedInventoryPlacement reuses the existing offline comparator
// for a bounded batch while preserving persisted revision and Runner identity.
// A foreign owner or duplicate device/instance fails closed.
func EvaluatePersistedInventoryPlacement(
	request PersistedInventoryPlacementBatchRequest,
) (PersistedInventoryPlacementBatchEvaluation, error) {
	if !validOwner(request.Owner) || !validDeviceID(request.Owner.TenantID) || request.EvaluatedAtMS <= 0 || request.EvaluatedAtMS > MaxSafeIntegerMS || len(request.Inputs) > MaxDevices {
		return PersistedInventoryPlacementBatchEvaluation{}, errInvalidPersistedInventoryPlacementInput
	}
	devices := make([]Device, 0, len(request.Inputs))
	metadata := make(map[string]PersistedInventoryPlacementBatchDecision, len(request.Inputs))
	seenInstances := make(map[string]struct{}, len(request.Inputs))
	for _, input := range request.Inputs {
		if err := ValidatePersistedInventoryPlacementInput(input); err != nil {
			return PersistedInventoryPlacementBatchEvaluation{}, err
		}
		if input.Owner != request.Owner {
			return PersistedInventoryPlacementBatchEvaluation{}, errors.New("owner_mismatch")
		}
		if _, exists := metadata[input.DeviceID]; exists {
			return PersistedInventoryPlacementBatchEvaluation{}, errInvalidPersistedInventoryPlacementInput
		}
		if _, exists := seenInstances[input.InstanceID]; exists {
			return PersistedInventoryPlacementBatchEvaluation{}, errInvalidPersistedInventoryPlacementInput
		}
		device, err := persistedInventoryPlacementDevice(input)
		if err != nil {
			return PersistedInventoryPlacementBatchEvaluation{}, err
		}
		devices = append(devices, device)
		metadata[input.DeviceID] = PersistedInventoryPlacementBatchDecision{
			Revision: input.Revision, DeviceID: input.DeviceID, InstanceID: input.InstanceID,
		}
		seenInstances[input.InstanceID] = struct{}{}
	}
	result, err := Evaluate(Request{
		SchemaVersion: RequestSchemaVersion, EvaluatedAtMS: request.EvaluatedAtMS,
		Owner: request.Owner, MaxSnapshotAgeMS: PersistedInventoryPlacementStaleAfterMS,
		Requirements: request.Requirements, Devices: devices,
	})
	if err != nil {
		return PersistedInventoryPlacementBatchEvaluation{}, err
	}
	decisions := make([]PersistedInventoryPlacementBatchDecision, 0, len(result.DeviceResults))
	for _, device := range result.DeviceResults {
		decision := metadata[device.DeviceID]
		decision.MatchesRequirements = device.MatchesRequirements
		decision.ExclusionReasons = append([]string(nil), device.ExclusionReasons...)
		decisions = append(decisions, decision)
	}
	return PersistedInventoryPlacementBatchEvaluation{
		SchemaVersion:  persistedInventoryPlacementBatchSchemaVersion,
		EvaluationMode: "pure_persisted_inventory_placement_dry_run",
		Owner:          request.Owner, EvaluatedAtMS: request.EvaluatedAtMS,
		OwnerDeclarationUnverified: true, DeviceAttributesUnverified: true,
		Decisions: decisions, Authority: PersistedInventoryPlacementBatchAuthority{},
	}, nil
}
