package deviceheartbeat

import "math"

// PersistedInstance is the bounded state that a future transaction would
// compare-and-swap. It is a value contract only; it does not own storage.
type PersistedInstance struct {
	Revision uint64   `json:"revision"`
	Instance Instance `json:"instance"`
}

// PersistenceError is a stable, authority-neutral transaction result.
type PersistenceError ErrorCode

const (
	ErrRevisionConflict      PersistenceError = "revision_conflict"
	ErrInvalidPersistedState PersistenceError = "invalid_persisted_state"
	ErrRevisionOverflow      PersistenceError = "revision_overflow"
)

func (e PersistenceError) Error() string { return string(e) }

// ValidatePersistedInstance checks the value-level invariants a future
// storage adapter must enforce when it restores one runner observation. It
// does not read storage, acquire a clock, or verify device identity.
func ValidatePersistedInstance(value PersistedInstance) error {
	if value.Revision == 0 || !validPersistedInstance(value.Instance) {
		return ErrInvalidPersistedState
	}
	return nil
}

// Commit applies one heartbeat only when expectedRevision still names the
// supplied snapshot. A caller can use the returned value as the transaction's
// complete replacement, but this function performs no write or retry.
func Commit(
	device Device,
	current *PersistedInstance,
	expectedRevision uint64,
	heartbeat Heartbeat,
	serverObservedAtMS, leaseTTLMS uint64,
) (PersistedInstance, error) {
	actualRevision := uint64(0)
	var prior *Instance
	if current != nil {
		if err := ValidatePersistedInstance(*current); err != nil {
			return PersistedInstance{}, err
		}
		actualRevision = current.Revision
		prior = &current.Instance
	}
	if expectedRevision != actualRevision {
		return PersistedInstance{}, ErrRevisionConflict
	}
	if actualRevision == math.MaxUint64 {
		return PersistedInstance{}, ErrRevisionOverflow
	}
	next, err := Apply(device, prior, heartbeat, serverObservedAtMS, leaseTTLMS)
	if err != nil {
		return PersistedInstance{}, err
	}
	return PersistedInstance{Revision: actualRevision + 1, Instance: next}, nil
}

// validPersistedInstance mirrors the value checks performed when a future
// storage adapter restores a Runner instance. Commit receives a value-level
// snapshot only, so malformed state must fail before heartbeat comparison or
// replacement; this function does not inspect storage or acquire authority.
func validPersistedInstance(value Instance) bool {
	canonicalCapabilities, err := value.Capabilities.canonicalize()
	if err != nil || !capabilitySnapshotsEqual(value.Capabilities, canonicalCapabilities) {
		return false
	}
	if value.DeviceID == "" || value.InstanceID == "" || value.Generation == 0 || value.HeartbeatSequence == 0 ||
		value.CapabilityLeaseExpiresAtMS < value.ServerObservedAtMS {
		return false
	}
	leaseTTL := value.CapabilityLeaseExpiresAtMS - value.ServerObservedAtMS
	return leaseTTL >= MinLeaseTTLMS && leaseTTL <= MaxLeaseTTLMS
}
