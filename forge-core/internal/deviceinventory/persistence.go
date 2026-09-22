package deviceinventory

import (
	"math"

	"forgeos/forge-core/internal/deviceheartbeat"
)

// PersistedInventoryState is the value-level shape a future authoritative
// inventory transaction may restore and replace. It has no storage handle,
// clock, listener, credential, or scheduling authority.
type PersistedInventoryState struct {
	Revision uint64               `json:"revision"`
	Device   DeviceRecord         `json:"device"`
	Runner   RunnerInstanceRecord `json:"runner"`
}

// DeviceRecord is the owner-tuple-bound device state needed by a read-only
// inventory projection. The tuple is still unverified input at this boundary;
// the state strings do not authorize enrollment, reservation, or execution.
type DeviceRecord struct {
	DeviceID         string        `json:"device_id"`
	Owner            SnapshotOwner `json:"owner"`
	ApprovalState    string        `json:"approval_state"`
	CordonState      string        `json:"cordon_state"`
	ReservationState string        `json:"reservation_state"`
}

// RunnerInstanceRecord is the bounded server-observed Runner value carried by
// a persisted inventory state. Capabilities remain observations even after
// validation and never become hardware proof.
type RunnerInstanceRecord struct {
	DeviceID                   string                             `json:"device_id"`
	InstanceID                 string                             `json:"instance_id"`
	Generation                 uint64                             `json:"generation"`
	HeartbeatSequence          uint64                             `json:"heartbeat_sequence"`
	ServerObservedAtMS         uint64                             `json:"server_observed_at_ms"`
	CapabilityLeaseExpiresAtMS uint64                             `json:"capability_lease_expires_at_ms"`
	Liveness                   string                             `json:"liveness"`
	Capabilities               deviceheartbeat.CapabilitySnapshot `json:"capabilities"`
}

// PersistedInventoryProjection is a display-only result at a caller-supplied
// evaluation time. Revision is included so a client can correlate a read
// with the value it observed; it is not a reservation or execution lease.
type PersistedInventoryProjection struct {
	Revision         uint64 `json:"revision"`
	DeviceID         string `json:"device_id"`
	InstanceID       string `json:"instance_id"`
	Status           string `json:"status"`
	Fresh            bool   `json:"fresh"`
	DeclaredEligible bool   `json:"declared_eligible"`
}

type PersistenceError string

const (
	ErrRevisionConflict       PersistenceError = "revision_conflict"
	ErrInvalidPersistedState  PersistenceError = "invalid_persisted_state"
	ErrRevisionOverflow       PersistenceError = "revision_overflow"
	ErrInvalidDeviceRecord    PersistenceError = "invalid_device_record"
	ErrInvalidRunnerRecord    PersistenceError = "invalid_runner_record"
	ErrRunnerDeviceMismatch   PersistenceError = "runner_device_mismatch"
	ErrInventoryOwnerMismatch PersistenceError = "owner_mismatch"
	ErrDeviceBindingChanged   PersistenceError = "device_binding_changed"
	ErrInvalidEvaluationOwner PersistenceError = "invalid_evaluation_owner"
)

func (e PersistenceError) Error() string { return string(e) }

// RestorePersistedInventory validates a value returned by a future storage
// adapter. It performs no I/O and rejects a zero revision before exposing the
// state to a projection or compare-and-swap evaluator.
func RestorePersistedInventory(
	revision uint64,
	device DeviceRecord,
	runner RunnerInstanceRecord,
) (PersistedInventoryState, error) {
	if revision == 0 {
		return PersistedInventoryState{}, ErrInvalidPersistedState
	}
	canonicalRunner, err := canonicalRunnerRecord(runner)
	if err != nil {
		return PersistedInventoryState{}, err
	}
	state := PersistedInventoryState{Revision: revision, Device: device, Runner: canonicalRunner}
	if err := validatePersistedInventory(state); err != nil {
		return PersistedInventoryState{}, err
	}
	return state, nil
}

// CommitPersistedInventory evaluates one complete replacement using an exact
// revision. A nil current value represents an empty transaction at revision
// zero. The returned replacement is a complete copy with the next revision;
// the supplied records are never mutated and no write is performed.
func CommitPersistedInventory(
	current *PersistedInventoryState,
	expectedRevision uint64,
	device DeviceRecord,
	runner RunnerInstanceRecord,
) (PersistedInventoryState, error) {
	actualRevision := uint64(0)
	if current != nil {
		if err := validatePersistedInventory(*current); err != nil {
			return PersistedInventoryState{}, err
		}
		actualRevision = current.Revision
	}
	if expectedRevision != actualRevision {
		return PersistedInventoryState{}, ErrRevisionConflict
	}
	if actualRevision == math.MaxUint64 {
		return PersistedInventoryState{}, ErrRevisionOverflow
	}
	if current != nil {
		if device.DeviceID != current.Device.DeviceID || device.Owner != current.Device.Owner ||
			runner.DeviceID != current.Runner.DeviceID {
			return PersistedInventoryState{}, ErrDeviceBindingChanged
		}
	}
	canonicalRunner, err := canonicalRunnerRecord(runner)
	if err != nil {
		return PersistedInventoryState{}, err
	}
	next := PersistedInventoryState{Revision: actualRevision + 1, Device: device, Runner: canonicalRunner}
	if err := validatePersistedInventory(next); err != nil {
		return PersistedInventoryState{}, err
	}
	return next, nil
}

// ProjectPersistedInventory validates an owner-scoped value and classifies it
// at the explicit evaluation time. It never reads a clock or changes state.
func ProjectPersistedInventory(
	value PersistedInventoryState,
	evaluationOwner SnapshotOwner,
	evaluatedAtMS uint64,
	staleAfterMS uint64,
) (PersistedInventoryProjection, error) {
	if err := validatePersistedInventory(value); err != nil {
		return PersistedInventoryProjection{}, err
	}
	if !validSnapshotOwner(evaluationOwner) {
		return PersistedInventoryProjection{}, ErrInvalidEvaluationOwner
	}
	if value.Device.Owner != evaluationOwner {
		return PersistedInventoryProjection{}, ErrInventoryOwnerMismatch
	}
	status, err := Project(Observation{
		ApprovalState:        value.Device.ApprovalState,
		CordonState:          value.Device.CordonState,
		Liveness:             value.Runner.Liveness,
		ReservationState:     value.Device.ReservationState,
		SnapshotObservedAtMS: value.Runner.ServerObservedAtMS,
		LeaseExpiresAtMS:     value.Runner.CapabilityLeaseExpiresAtMS,
		EvaluatedAtMS:        evaluatedAtMS,
	}, staleAfterMS)
	if err != nil {
		return PersistedInventoryProjection{}, err
	}
	return PersistedInventoryProjection{
		Revision:         value.Revision,
		DeviceID:         value.Device.DeviceID,
		InstanceID:       value.Runner.InstanceID,
		Status:           status.Status,
		Fresh:            status.Fresh,
		DeclaredEligible: status.DeclaredEligible,
	}, nil
}

func validatePersistedInventory(value PersistedInventoryState) error {
	if value.Revision == 0 {
		return ErrInvalidPersistedState
	}
	if err := validateDeviceRecord(value.Device); err != nil {
		return err
	}
	if err := validateRunnerRecord(value.Runner); err != nil {
		return err
	}
	if value.Runner.DeviceID != value.Device.DeviceID {
		return ErrRunnerDeviceMismatch
	}
	return nil
}

func validateDeviceRecord(value DeviceRecord) error {
	if !validSnapshotIdentifier(value.DeviceID) || !validSnapshotOwner(value.Owner) {
		return ErrInvalidDeviceRecord
	}
	if !valid(value.ApprovalState, "approved", "pending", "revoked") ||
		!valid(value.CordonState, "clear", "cordoned") ||
		!valid(value.ReservationState, "none", "reserved") {
		return ErrInvalidDeviceRecord
	}
	return nil
}

func validateRunnerRecord(value RunnerInstanceRecord) error {
	_, err := canonicalRunnerRecord(value)
	return err
}

func canonicalRunnerRecord(value RunnerInstanceRecord) (RunnerInstanceRecord, error) {
	if !validSnapshotIdentifier(value.DeviceID) || !validSnapshotIdentifier(value.InstanceID) ||
		value.Generation == 0 || value.HeartbeatSequence == 0 ||
		value.CapabilityLeaseExpiresAtMS < value.ServerObservedAtMS ||
		value.CapabilityLeaseExpiresAtMS-value.ServerObservedAtMS < deviceheartbeat.MinLeaseTTLMS ||
		value.CapabilityLeaseExpiresAtMS-value.ServerObservedAtMS > deviceheartbeat.MaxLeaseTTLMS ||
		!valid(value.Liveness, "online", "offline") {
		return RunnerInstanceRecord{}, ErrInvalidRunnerRecord
	}
	canonical, err := deviceheartbeat.NewCapabilitySnapshot(
		value.Capabilities.OperatingSystem,
		value.Capabilities.Architecture,
		value.Capabilities.CPUCores,
		value.Capabilities.AvailableCPUCores,
		value.Capabilities.MemoryBytes,
		value.Capabilities.AvailableMemoryBytes,
		value.Capabilities.StorageBytes,
		value.Capabilities.AvailableStorageBytes,
		value.Capabilities.GPUs,
		value.Capabilities.Runtimes,
	)
	if err != nil || !capabilitySnapshotsEqual(value.Capabilities, canonical) {
		return RunnerInstanceRecord{}, ErrInvalidRunnerRecord
	}
	value.Capabilities = canonical
	return value, nil
}

func capabilitySnapshotsEqual(left, right deviceheartbeat.CapabilitySnapshot) bool {
	if left.OperatingSystem != right.OperatingSystem || left.Architecture != right.Architecture ||
		left.CPUCores != right.CPUCores || left.AvailableCPUCores != right.AvailableCPUCores ||
		left.MemoryBytes != right.MemoryBytes || left.AvailableMemoryBytes != right.AvailableMemoryBytes ||
		left.StorageBytes != right.StorageBytes || left.AvailableStorageBytes != right.AvailableStorageBytes ||
		len(left.GPUs) != len(right.GPUs) || len(left.Runtimes) != len(right.Runtimes) {
		return false
	}
	for index := range left.GPUs {
		if left.GPUs[index] != right.GPUs[index] {
			return false
		}
	}
	for index := range left.Runtimes {
		if left.Runtimes[index] != right.Runtimes[index] {
			return false
		}
	}
	return true
}
