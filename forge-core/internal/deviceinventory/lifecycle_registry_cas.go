package deviceinventory

import (
	"fmt"
	"sort"

	"forgeos/forge-core/internal/deviceidentity"
)

// PersistedEnrollmentHeartbeatLifecycleRegistryState is the complete,
// owner-scoped value image used by the future registry transaction. Each
// member's Revision is its per-device compare-and-swap revision; there is no
// global registry revision that could let one device update clobber another.
// The value has no storage handle, clock, credential, listener, or execution
// authority.
type PersistedEnrollmentHeartbeatLifecycleRegistryState struct {
	Owner  deviceidentity.Owner                         `json:"owner"`
	States []PersistedEnrollmentHeartbeatLifecycleState `json:"states"`
}

// CanonicalizePersistedEnrollmentHeartbeatLifecycleRegistry checks and copies
// a complete owner-scoped registry value image without reading storage or
// acquiring any authority. The returned members are canonical nested values
// in deterministic device/Runner order, so a transport cannot accidentally
// echo an injected unsorted or aliased slice.
func CanonicalizePersistedEnrollmentHeartbeatLifecycleRegistry(
	value PersistedEnrollmentHeartbeatLifecycleRegistryState,
) (PersistedEnrollmentHeartbeatLifecycleRegistryState, error) {
	if !validLifecycleOwner(value.Owner) {
		return PersistedEnrollmentHeartbeatLifecycleRegistryState{}, ErrLifecycleRegistryOwnerMismatch
	}
	states, err := cloneAndValidateLifecycleRegistry(&value, value.Owner)
	if err != nil {
		return PersistedEnrollmentHeartbeatLifecycleRegistryState{}, err
	}
	sortLifecycleRegistryStates(states)
	return PersistedEnrollmentHeartbeatLifecycleRegistryState{
		Owner:  value.Owner,
		States: states,
	}, nil
}

// ValidatePersistedEnrollmentHeartbeatLifecycleRegistry checks a complete
// owner-scoped registry value image without reading storage or acquiring any
// authority. It is the shared boundary for adapters that project the
// already-restored lifecycle image into another read-only transport.
func ValidatePersistedEnrollmentHeartbeatLifecycleRegistry(
	value PersistedEnrollmentHeartbeatLifecycleRegistryState,
) error {
	_, err := CanonicalizePersistedEnrollmentHeartbeatLifecycleRegistry(value)
	return err
}

// LifecycleRegistryPersistenceError identifies a rejected aggregate image or
// per-device CAS. It is deliberately distinct from filesystem errors: this
// function only computes a replacement and performs no write.
type LifecycleRegistryPersistenceError string

const (
	ErrLifecycleRegistryOwnerMismatch    LifecycleRegistryPersistenceError = "lifecycle_registry_owner_mismatch"
	ErrLifecycleRegistryRevisionConflict LifecycleRegistryPersistenceError = "lifecycle_registry_revision_conflict"
	ErrLifecycleRegistryInvalidState     LifecycleRegistryPersistenceError = "lifecycle_registry_invalid_state"
	ErrLifecycleRegistryDuplicateDevice  LifecycleRegistryPersistenceError = "lifecycle_registry_duplicate_device"
	ErrLifecycleRegistryDuplicateRunner  LifecycleRegistryPersistenceError = "lifecycle_registry_duplicate_runner"
	ErrLifecycleRegistryCapacityExceeded LifecycleRegistryPersistenceError = "lifecycle_registry_capacity_exceeded"
)

func (e LifecycleRegistryPersistenceError) Error() string { return string(e) }

// CommitPersistedEnrollmentHeartbeatLifecycleRegistry applies one complete
// enrollment/heartbeat/inventory replacement to an owner-scoped registry
// image. expectedDeviceRevision is compared only with the input device's
// current lifecycle image: zero creates a new device, while a nonzero value
// must equal that image's outer Revision. Every existing member is validated
// before the replacement is exposed, and a duplicate Runner identity across
// devices rejects the complete operation.
//
// The returned registry and lifecycle result are value images only. The
// function does not acquire a lock, read a clock, authenticate a proof, write
// a file/database, accept a heartbeat, publish inventory, or grant execution
// authority. A future durable adapter must persist the returned complete image
// under its own transaction and recheck the same owner/CAS boundary.
func CommitPersistedEnrollmentHeartbeatLifecycleRegistry(
	current *PersistedEnrollmentHeartbeatLifecycleRegistryState,
	owner deviceidentity.Owner,
	expectedDeviceRevision uint64,
	input EnrollmentHeartbeatLifecycleInput,
) (PersistedEnrollmentHeartbeatLifecycleRegistryState, EnrollmentHeartbeatLifecycleResult, error) {
	if !validLifecycleOwner(owner) || input.Owner != owner || !validLifecycleDevice(input.Device, owner) {
		return PersistedEnrollmentHeartbeatLifecycleRegistryState{}, EnrollmentHeartbeatLifecycleResult{}, ErrLifecycleRegistryOwnerMismatch
	}

	states, err := cloneAndValidateLifecycleRegistry(current, owner)
	if err != nil {
		return PersistedEnrollmentHeartbeatLifecycleRegistryState{}, EnrollmentHeartbeatLifecycleResult{}, err
	}

	index := -1
	for candidate, state := range states {
		if state.Device.DeviceID == input.Device.DeviceID {
			index = candidate
			break
		}
	}
	var existing *PersistedEnrollmentHeartbeatLifecycleState
	if index >= 0 {
		if states[index].Revision != expectedDeviceRevision {
			return PersistedEnrollmentHeartbeatLifecycleRegistryState{}, EnrollmentHeartbeatLifecycleResult{}, ErrLifecycleRegistryRevisionConflict
		}
		existing = &states[index]
	} else if expectedDeviceRevision != 0 {
		return PersistedEnrollmentHeartbeatLifecycleRegistryState{}, EnrollmentHeartbeatLifecycleResult{}, ErrLifecycleRegistryRevisionConflict
	}

	next, result, err := CommitPersistedEnrollmentHeartbeatLifecycle(existing, expectedDeviceRevision, input)
	if err != nil {
		return PersistedEnrollmentHeartbeatLifecycleRegistryState{}, EnrollmentHeartbeatLifecycleResult{}, err
	}

	if index >= 0 {
		states[index] = next
	} else {
		if len(states) >= maxPersistedLifecycleRegistryStates {
			return PersistedEnrollmentHeartbeatLifecycleRegistryState{}, EnrollmentHeartbeatLifecycleResult{}, ErrLifecycleRegistryCapacityExceeded
		}
		states = append(states, next)
	}
	if err := validateLifecycleRegistryMembers(states, owner); err != nil {
		return PersistedEnrollmentHeartbeatLifecycleRegistryState{}, EnrollmentHeartbeatLifecycleResult{}, err
	}
	sort.Slice(states, func(left, right int) bool {
		if states[left].Device.DeviceID != states[right].Device.DeviceID {
			return states[left].Device.DeviceID < states[right].Device.DeviceID
		}
		return states[left].Heartbeat.Instance.InstanceID < states[right].Heartbeat.Instance.InstanceID
	})

	return PersistedEnrollmentHeartbeatLifecycleRegistryState{
		Owner:  owner,
		States: states,
	}, result, nil
}

func cloneAndValidateLifecycleRegistry(
	current *PersistedEnrollmentHeartbeatLifecycleRegistryState,
	owner deviceidentity.Owner,
) ([]PersistedEnrollmentHeartbeatLifecycleState, error) {
	if current == nil {
		return nil, nil
	}
	if current.Owner != owner {
		return nil, ErrLifecycleRegistryOwnerMismatch
	}
	if len(current.States) > maxPersistedLifecycleRegistryStates {
		return nil, ErrLifecycleRegistryCapacityExceeded
	}
	states := make([]PersistedEnrollmentHeartbeatLifecycleState, len(current.States))
	copy(states, current.States)
	if err := validateLifecycleRegistryMembers(states, owner); err != nil {
		return nil, err
	}
	return states, nil
}

func validateLifecycleRegistryMembers(
	states []PersistedEnrollmentHeartbeatLifecycleState,
	owner deviceidentity.Owner,
) error {
	if len(states) > maxPersistedLifecycleRegistryStates {
		return ErrLifecycleRegistryCapacityExceeded
	}
	seenDevices := make(map[string]struct{}, len(states))
	seenRunners := make(map[string]struct{}, len(states))
	for index := range states {
		state := states[index]
		if state.Owner != owner {
			return ErrLifecycleRegistryOwnerMismatch
		}
		canonical, err := RestorePersistedEnrollmentHeartbeatLifecycle(
			state.Revision,
			state.Owner,
			state.Device,
			state.Heartbeat,
			state.Inventory,
		)
		if err != nil {
			return fmt.Errorf("%w: state %d: %v", ErrLifecycleRegistryInvalidState, index, err)
		}
		if err := validateApprovalCandidate(state.ApprovalCandidate, state.Owner, state.Device); err != nil {
			return fmt.Errorf("%w: state %d: approval candidate: %v", ErrLifecycleRegistryInvalidState, index, err)
		}
		if err := validateCredentialCandidate(state.CredentialCandidate, state.Owner, state.Device); err != nil {
			return fmt.Errorf("%w: state %d: credential candidate: %v", ErrLifecycleRegistryInvalidState, index, err)
		}
		if err := validateChallengeCandidate(state.ChallengeCandidate); err != nil {
			return fmt.Errorf("%w: state %d: challenge candidate: %v", ErrLifecycleRegistryInvalidState, index, err)
		}
		canonical.ApprovalCandidate = cloneApprovalCandidate(state.ApprovalCandidate)
		canonical.CredentialCandidate = cloneCredentialCandidate(state.CredentialCandidate)
		canonical.ChallengeCandidate = cloneChallengeCandidate(state.ChallengeCandidate)
		states[index] = canonical
		deviceID := canonical.Device.DeviceID
		if _, exists := seenDevices[deviceID]; exists {
			return fmt.Errorf("%w: %s", ErrLifecycleRegistryDuplicateDevice, deviceID)
		}
		seenDevices[deviceID] = struct{}{}
		runnerID := canonical.Heartbeat.Instance.InstanceID
		if _, exists := seenRunners[runnerID]; exists {
			return fmt.Errorf("%w: %s", ErrLifecycleRegistryDuplicateRunner, runnerID)
		}
		seenRunners[runnerID] = struct{}{}
	}
	return nil
}
