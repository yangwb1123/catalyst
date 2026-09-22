package deviceinventory

// This file adds the next, still candidate-only, restart boundary for the
// lifecycle registry.  It persists a complete owner-scoped value image with
// an exact filesystem-image CAS.  It deliberately does not authenticate a
// device, accept a heartbeat, read a clock, expose a route, or make inventory
// authoritative.  Production constructors do not wire this adapter.

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"strings"

	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/statefs"
)

const maxLifecycleRegistryFileCASBytes = maxPersistedLifecycleRegistryFileSize

// PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot is an opaque read
// token for one complete registry image.  The token includes the exact file
// bytes and mode observed by ReadSnapshot; callers must pass it unchanged to
// ReplaceStatesIfUnchanged.  Its fields stay private so a caller cannot
// manufacture a token for another path or owner.
type PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot struct {
	path    string
	owner   deviceidentity.Owner
	data    []byte
	mode    os.FileMode
	present bool
	states  []PersistedEnrollmentHeartbeatLifecycleState
}

// Present reports whether the snapshot had a file.  A non-present snapshot is
// a valid revision-zero create expectation, provided its parent directory is
// already present and private.
func (snapshot PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot) Present() bool {
	return snapshot.present
}

// States returns an independent copy of the validated image represented by
// the token.  It is useful for constructing the next pure CAS value without
// sharing capability slices with the adapter.
func (snapshot PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot) States() []PersistedEnrollmentHeartbeatLifecycleState {
	return cloneLifecycleRegistryStates(snapshot.states)
}

// LifecycleRegistryFileCASError identifies a rejected write boundary.  A
// conflict means the exact expected file image is no longer current; rollback
// is a monotonic lifecycle violation; invalid and write errors mean the token
// or filesystem cannot be used safely.
type LifecycleRegistryFileCASError string

const (
	ErrPersistedLifecycleRegistryFileCASConflict LifecycleRegistryFileCASError = "persisted_lifecycle_registry_file_cas_conflict"
	ErrPersistedLifecycleRegistryFileCASInvalid  LifecycleRegistryFileCASError = "persisted_lifecycle_registry_file_cas_invalid"
	ErrPersistedLifecycleRegistryFileCASWrite    LifecycleRegistryFileCASError = "persisted_lifecycle_registry_file_cas_write"
	ErrPersistedLifecycleRegistryFileCASRollback LifecycleRegistryFileCASError = "persisted_lifecycle_registry_file_cas_rollback"
)

func (e LifecycleRegistryFileCASError) Error() string { return string(e) }

// PersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter owns no open file
// descriptor and has no authority beyond replacing a prevalidated value
// image.  It is intended for injected restart tests until a separately
// accepted durable lifecycle decision exists.
type PersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter struct {
	path  string
	owner deviceidentity.Owner
}

// NewPersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter validates fixed
// parameters without touching the filesystem.  The parent directory must
// already exist and be private when ReadSnapshot or ReplaceStatesIfUnchanged
// is called; construction remains side-effect free.
func NewPersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter(
	path string,
	owner deviceidentity.Owner,
) (PersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter, error) {
	if strings.TrimSpace(path) == "" || filepath.Clean(path) == "." || !validLifecycleOwner(owner) {
		return PersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter{}, ErrPersistedLifecycleRegistryFileCASInvalid
	}
	return PersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter{
		path:  filepath.Clean(path),
		owner: owner,
	}, nil
}

// ReadSnapshot reads and validates one complete private registry image.  A
// missing leaf is returned as a valid non-present token so a caller can make a
// revision-zero create attempt.  A missing or insecure parent remains an
// explicit error; this adapter never creates directories as a side effect.
func (adapter PersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter) ReadSnapshot() (PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot, error) {
	data, mode, present, err := readTrackedLifecycleRegistryFile(adapter.path)
	if err != nil {
		return PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, err
	}
	snapshot := PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{
		path:    adapter.path,
		owner:   adapter.owner,
		mode:    mode,
		present: present,
	}
	if !present {
		return snapshot, nil
	}
	states, err := decodeAndRestoreLifecycleRegistryFile(data, adapter.owner)
	if err != nil {
		return PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, err
	}
	snapshot.data = append([]byte(nil), data...)
	snapshot.states = states
	return snapshot, nil
}

// ReplaceStatesIfUnchanged canonicalizes and atomically publishes the next
// complete owner-scoped registry image only when token still describes the
// exact current file.  The returned token represents the published bytes and
// can be used for the next CAS.  All validation occurs before the file write.
func (adapter PersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter) ReplaceStatesIfUnchanged(
	snapshot PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot,
	next []PersistedEnrollmentHeartbeatLifecycleState,
) (published PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot, err error) {
	if strings.TrimSpace(adapter.path) == "" || filepath.Clean(adapter.path) == "." || !validLifecycleOwner(adapter.owner) {
		return PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, ErrPersistedLifecycleRegistryFileCASInvalid
	}
	// The exact-image check in statefs protects against stale snapshots, but a
	// read/check/rename sequence still needs one inter-process serialization
	// point.  Without it, two writers can both pass the final check and the
	// later rename can overwrite the earlier commit.  The lock is deliberately
	// a sibling control file owned by this candidate adapter; it grants no
	// device, inventory, or execution authority and production does not wire
	// this adapter.
	fileLock, lockErr := acquireLifecycleRegistryFileLock(adapter.path)
	if lockErr != nil {
		return PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, fmt.Errorf("%w: acquire lock: %v", ErrPersistedLifecycleRegistryFileCASWrite, lockErr)
	}
	defer func() {
		if unlockErr := fileLock.Close(); unlockErr != nil && err == nil {
			published = PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}
			err = fmt.Errorf("%w: release lock: %v", ErrPersistedLifecycleRegistryFileCASWrite, unlockErr)
		}
	}()

	if snapshot.path != adapter.path || snapshot.owner != adapter.owner ||
		(!snapshot.present && (snapshot.mode != 0 || len(snapshot.data) != 0)) ||
		(snapshot.present && snapshot.mode.Perm() != 0o600) {
		return PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, ErrPersistedLifecycleRegistryFileCASInvalid
	}
	var current []PersistedEnrollmentHeartbeatLifecycleState
	if snapshot.present {
		current, err = decodeAndRestoreLifecycleRegistryFile(snapshot.data, adapter.owner)
		if err != nil {
			return PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, fmt.Errorf("%w: expected image: %v", ErrPersistedLifecycleRegistryFileCASInvalid, err)
		}
	}
	canonical, err := canonicalLifecycleRegistryStates(next, adapter.owner)
	if err != nil {
		return PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, err
	}
	if err := validateLifecycleRegistryReplacementProgress(current, canonical); err != nil {
		return PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, err
	}
	data, err := marshalLifecycleRegistryFile(adapter.owner, canonical)
	if err != nil {
		return PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, err
	}
	if err := statefs.AtomicWriteTrackedIfUnchanged(
		adapter.path,
		snapshot.data,
		snapshot.mode,
		snapshot.present,
		data,
		0o600,
	); err != nil {
		if currentData, currentMode, currentPresent, readErr := readTrackedLifecycleRegistryFile(adapter.path); readErr == nil {
			if currentPresent != snapshot.present || currentMode.Perm() != snapshot.mode.Perm() ||
				!bytes.Equal(currentData, snapshot.data) {
				return PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, fmt.Errorf("%w: expected image is stale", ErrPersistedLifecycleRegistryFileCASConflict)
			}
		}
		return PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, fmt.Errorf("%w: %v", ErrPersistedLifecycleRegistryFileCASWrite, err)
	}
	return PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{
		path:    adapter.path,
		owner:   adapter.owner,
		data:    append([]byte(nil), data...),
		mode:    0o600,
		present: true,
		states:  canonical,
	}, nil
}

// validateLifecycleRegistryReplacementProgress keeps the file boundary from
// becoming a rollback oracle.  The pure lifecycle Commit path already emits
// one next image, but the candidate PUT adapter also accepts complete images;
// without this check a caller holding the current opaque token could write an
// older revision, generation, or heartbeat sequence back to disk.  Existing
// members are retained and advance by exactly one outer revision.  An exact
// same-image retry is allowed for idempotent transport retries.
func validateLifecycleRegistryReplacementProgress(
	current []PersistedEnrollmentHeartbeatLifecycleState,
	next []PersistedEnrollmentHeartbeatLifecycleState,
) error {
	currentByDevice := make(map[string]PersistedEnrollmentHeartbeatLifecycleState, len(current))
	for _, state := range current {
		currentByDevice[state.Device.DeviceID] = state
	}
	nextByDevice := make(map[string]PersistedEnrollmentHeartbeatLifecycleState, len(next))
	for _, state := range next {
		nextByDevice[state.Device.DeviceID] = state
	}
	for deviceID, prior := range currentByDevice {
		candidate, present := nextByDevice[deviceID]
		if !present {
			return lifecycleRegistryRollbackError("existing device was removed")
		}
		if candidate.Revision == prior.Revision {
			if !sameLifecycleImageExceptCandidates(candidate, prior) {
				return lifecycleRegistryRollbackError("same revision changed")
			}
			continue
		}
		if prior.Revision == ^uint64(0) || candidate.Revision != prior.Revision+1 {
			return lifecycleRegistryRollbackError("revision did not advance by one")
		}
		if candidate.Device != prior.Device ||
			candidate.Inventory.Device.CordonState != prior.Inventory.Device.CordonState ||
			candidate.Inventory.Device.ReservationState != prior.Inventory.Device.ReservationState {
			return lifecycleRegistryRollbackError("immutable binding or server state changed")
		}
		priorInstance := prior.Heartbeat.Instance
		candidateInstance := candidate.Heartbeat.Instance
		if candidateInstance.ServerObservedAtMS < priorInstance.ServerObservedAtMS {
			return lifecycleRegistryRollbackError("server observation time regressed")
		}
		if candidateInstance.Generation < priorInstance.Generation {
			return lifecycleRegistryRollbackError("generation regressed")
		}
		if candidateInstance.Generation == priorInstance.Generation {
			if candidateInstance.InstanceID != priorInstance.InstanceID ||
				candidateInstance.HeartbeatSequence <= priorInstance.HeartbeatSequence {
				return lifecycleRegistryRollbackError("heartbeat sequence did not advance")
			}
			continue
		}
		if priorInstance.Generation == ^uint64(0) || candidateInstance.Generation != priorInstance.Generation+1 ||
			candidateInstance.HeartbeatSequence != 1 {
			return lifecycleRegistryRollbackError("generation transition is invalid")
		}
	}
	for _, candidate := range next {
		if _, present := currentByDevice[candidate.Device.DeviceID]; !present &&
			(candidate.Revision != 1 || candidate.Heartbeat.Instance.Generation != 1 || candidate.Heartbeat.Instance.HeartbeatSequence != 1) {
			return lifecycleRegistryRollbackError("new device did not start at revision one")
		}
	}
	return nil
}

// Approval and credential candidates are the only same-revision mutations
// accepted by this file CAS. They are owner-scoped replacement plans and
// intentionally do not alter the live device, heartbeat, inventory, or Runner.
func sameLifecycleImageExceptCandidates(
	left PersistedEnrollmentHeartbeatLifecycleState,
	right PersistedEnrollmentHeartbeatLifecycleState,
) bool {
	left.ApprovalCandidate = nil
	right.ApprovalCandidate = nil
	left.CredentialCandidate = nil
	right.CredentialCandidate = nil
	return reflect.DeepEqual(left, right)
}

func lifecycleRegistryRollbackError(reason string) error {
	// Include the ordinary CAS conflict classification so existing candidate
	// transports surface rollback as a retryable 409 while callers that need a
	// precise persistence audit can still match the dedicated reason.
	return fmt.Errorf("%w: %w: %s", ErrPersistedLifecycleRegistryFileCASConflict, ErrPersistedLifecycleRegistryFileCASRollback, reason)
}

// readTrackedLifecycleRegistryFile verifies the private parent and reads the
// exact leaf image without chmod or other repair.  It intentionally permits a
// missing leaf for the create CAS path.
func readTrackedLifecycleRegistryFile(path string) ([]byte, os.FileMode, bool, error) {
	directory, present, err := statefs.InspectDir(filepath.Dir(path))
	if err != nil {
		return nil, 0, false, fmt.Errorf("%w: parent: %v", ErrPersistedLifecycleRegistryFileCASInvalid, err)
	}
	if !present || directory.Mode().Perm()&0o077 != 0 {
		return nil, 0, false, ErrPersistedLifecycleRegistryFileCASInvalid
	}
	data, mode, present, err := statefs.ReadTracked(path, maxLifecycleRegistryFileCASBytes)
	if err != nil {
		return nil, 0, false, fmt.Errorf("%w: %v", ErrPersistedLifecycleRegistryFileCASInvalid, err)
	}
	if !present {
		return nil, 0, false, nil
	}
	if mode.Perm() != 0o600 {
		return nil, 0, false, fmt.Errorf("%w: file mode must be 0600", ErrPersistedLifecycleRegistryFileCASInvalid)
	}
	return data, mode.Perm(), true, nil
}

func decodeAndRestoreLifecycleRegistryFile(data []byte, owner deviceidentity.Owner) ([]PersistedEnrollmentHeartbeatLifecycleState, error) {
	envelope, err := decodePersistedLifecycleRegistryFile(data)
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrPersistedLifecycleRegistryFileCASInvalid, err)
	}
	if envelope.Owner != owner {
		return nil, ErrPersistedLifecycleRegistryOwnerMismatch
	}
	canonical, err := canonicalLifecycleRegistryStates(envelope.States, owner)
	if err != nil {
		return nil, err
	}
	return canonical, nil
}

func canonicalLifecycleRegistryStates(
	states []PersistedEnrollmentHeartbeatLifecycleState,
	owner deviceidentity.Owner,
) ([]PersistedEnrollmentHeartbeatLifecycleState, error) {
	if !validLifecycleOwner(owner) {
		return nil, ErrPersistedLifecycleRegistryFileCASInvalid
	}
	copyStates := make([]PersistedEnrollmentHeartbeatLifecycleState, len(states))
	copy(copyStates, states)
	if err := validateLifecycleRegistryMembers(copyStates, owner); err != nil {
		return nil, err
	}
	sortLifecycleRegistryStates(copyStates)
	return copyStates, nil
}

func sortLifecycleRegistryStates(states []PersistedEnrollmentHeartbeatLifecycleState) {
	sort.Slice(states, func(left, right int) bool {
		if states[left].Device.DeviceID != states[right].Device.DeviceID {
			return states[left].Device.DeviceID < states[right].Device.DeviceID
		}
		return states[left].Heartbeat.Instance.InstanceID < states[right].Heartbeat.Instance.InstanceID
	})
}

func cloneLifecycleRegistryStates(states []PersistedEnrollmentHeartbeatLifecycleState) []PersistedEnrollmentHeartbeatLifecycleState {
	if states == nil {
		return nil
	}
	clone := make([]PersistedEnrollmentHeartbeatLifecycleState, len(states))
	for index, state := range states {
		canonical, err := RestorePersistedEnrollmentHeartbeatLifecycle(
			state.Revision,
			state.Owner,
			state.Device,
			state.Heartbeat,
			state.Inventory,
		)
		if err == nil {
			canonical.ApprovalCandidate = cloneApprovalCandidate(state.ApprovalCandidate)
			canonical.CredentialCandidate = cloneCredentialCandidate(state.CredentialCandidate)
			clone[index] = canonical
		} else {
			clone[index] = state
		}
	}
	return clone
}

func marshalLifecycleRegistryFile(owner deviceidentity.Owner, states []PersistedEnrollmentHeartbeatLifecycleState) ([]byte, error) {
	payload, err := json.Marshal(persistedLifecycleRegistryFileEnvelope{
		SchemaVersion: persistedLifecycleRegistryFileSchema,
		Owner:         owner,
		States:        states,
	})
	if err != nil {
		return nil, fmt.Errorf("%w: encode: %v", ErrPersistedLifecycleRegistryFileCASInvalid, err)
	}
	if len(payload)+1 > maxLifecycleRegistryFileCASBytes {
		return nil, fmt.Errorf("%w: encoded image exceeds %d bytes", ErrPersistedLifecycleRegistryFileCASInvalid, maxLifecycleRegistryFileCASBytes)
	}
	return append(payload, '\n'), nil
}
