package deviceinventory

// This file deliberately contains a read-only filesystem adapter.  It is a
// restart boundary for the persisted inventory value contract, not an
// enrollment store: no heartbeat, credential, owner verification, clock,
// route, reservation, or execution authority is added here.

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"path/filepath"
	"sort"
	"strings"

	"forgeos/forge-core/internal/statefs"
)

const (
	persistedInventoryFileSchema       = "forge.device-inventory-file/v1"
	persistedInventoryFileSetSchema    = "forge.device-inventory-file-set/v1"
	maxPersistedInventoryFileSize      = 2 << 20
	maxPersistedInventoryFileSetStates = 128
)

// ErrPersistedInventoryFile identifies an absent or malformed preview file.
// It remains an authority-neutral read error and is intentionally separate
// from the value-level persistence errors returned by RestorePersistedInventory.
type FileReadError string

const (
	ErrPersistedInventoryFileMissing FileReadError = "persisted_inventory_file_missing"
	ErrPersistedInventoryFileInvalid FileReadError = "persisted_inventory_file_invalid"
)

func (e FileReadError) Error() string { return string(e) }

// PersistedInventoryFileReadAdapter reads one complete, owner-scoped
// persisted value from a regular private file.  It is suitable for an
// explicitly injected restart/read test and is not wired into production HTTP
// routes.  Evaluation time is supplied by the caller so this adapter never
// acquires a clock.
type PersistedInventoryFileReadAdapter struct {
	path            string
	evaluationOwner SnapshotOwner
	evaluatedAtMS   uint64
	staleAfterMS    uint64
}

// PersistedInventoryFileSetReadAdapter reads one complete owner-scoped
// aggregate of persisted inventory values from a regular private file.  The
// aggregate is an atomically replaced observation image for the read path; it
// is not a heartbeat store, registry, or scheduler input.  The adapter has no
// clock or write capability and is only suitable for an explicitly injected
// read boundary until a separately accepted production inventory decision.
type PersistedInventoryFileSetReadAdapter struct {
	path            string
	evaluationOwner SnapshotOwner
}

// NewPersistedInventoryFileSetReadAdapter validates the fixed read
// parameters.  Parent-directory and file checks are deferred to ReadStates so
// construction remains side-effect free and a missing image is observable.
func NewPersistedInventoryFileSetReadAdapter(
	path string,
	evaluationOwner SnapshotOwner,
) (PersistedInventoryFileSetReadAdapter, error) {
	if strings.TrimSpace(path) == "" || filepath.Clean(path) == "." {
		return PersistedInventoryFileSetReadAdapter{}, ErrPersistedInventoryFileInvalid
	}
	if !validSnapshotOwner(evaluationOwner) {
		return PersistedInventoryFileSetReadAdapter{}, ErrInvalidEvaluationOwner
	}
	return PersistedInventoryFileSetReadAdapter{
		path:            filepath.Clean(path),
		evaluationOwner: evaluationOwner,
	}, nil
}

// NewPersistedInventoryFileReadAdapter validates the fixed read parameters.
// The parent directory and file are inspected only when Read is called; this
// keeps construction side-effect free and makes a fresh/missing state explicit.
func NewPersistedInventoryFileReadAdapter(
	path string,
	evaluationOwner SnapshotOwner,
	evaluatedAtMS uint64,
	staleAfterMS uint64,
) (PersistedInventoryFileReadAdapter, error) {
	if strings.TrimSpace(path) == "" || filepath.Clean(path) == "." {
		return PersistedInventoryFileReadAdapter{}, ErrPersistedInventoryFileInvalid
	}
	if !validSnapshotOwner(evaluationOwner) {
		return PersistedInventoryFileReadAdapter{}, ErrInvalidEvaluationOwner
	}
	if evaluatedAtMS == 0 {
		return PersistedInventoryFileReadAdapter{}, ErrInvalidEvaluationTime
	}
	if staleAfterMS == 0 || staleAfterMS > MaxStaleAfterMS {
		return PersistedInventoryFileReadAdapter{}, ErrInvalidStaleAfter
	}
	return PersistedInventoryFileReadAdapter{
		path:            filepath.Clean(path),
		evaluationOwner: evaluationOwner,
		evaluatedAtMS:   evaluatedAtMS,
		staleAfterMS:    staleAfterMS,
	}, nil
}

// Read restores and owner-checks one value, then returns only its display
// projection.  ReadRegularUnmodified rejects aliases and preserves file mode
// and mtime; it therefore does not silently repair or chmod a state file.
func (adapter PersistedInventoryFileReadAdapter) Read() (PersistedInventoryProjection, error) {
	state, err := adapter.ReadState()
	if err != nil {
		return PersistedInventoryProjection{}, err
	}
	return ProjectPersistedInventory(state, adapter.evaluationOwner, adapter.evaluatedAtMS, adapter.staleAfterMS)
}

// ReadState restores one complete value and checks that its stored owner is
// exactly the adapter owner.  The returned state remains an unverified
// declaration; callers that need a display result should use Read.
func (adapter PersistedInventoryFileReadAdapter) ReadState() (PersistedInventoryState, error) {
	data, err := readPrivatePersistedInventoryFile(adapter.path)
	if err != nil {
		return PersistedInventoryState{}, err
	}
	envelope, err := decodePersistedInventoryFile(data)
	if err != nil {
		return PersistedInventoryState{}, err
	}
	state, err := RestorePersistedInventory(envelope.State.Revision, envelope.State.Device, envelope.State.Runner)
	if err != nil {
		return PersistedInventoryState{}, err
	}
	if state.Device.Owner != adapter.evaluationOwner {
		return PersistedInventoryState{}, ErrInventoryOwnerMismatch
	}
	return state, nil
}

// ReadStates restores the complete aggregate and checks the exact owner tuple
// on both the envelope and every state.  It returns a deterministic copy
// ordered by device ID then Runner instance ID.  A malformed or ambiguous
// member rejects the entire image so callers never observe a partial fleet.
func (adapter PersistedInventoryFileSetReadAdapter) ReadStates() ([]PersistedInventoryState, error) {
	data, err := readPrivatePersistedInventoryFileSet(adapter.path)
	if err != nil {
		return nil, err
	}
	envelope, err := decodePersistedInventoryFileSet(data)
	if err != nil {
		return nil, err
	}
	if envelope.Owner != adapter.evaluationOwner {
		return nil, ErrInventoryOwnerMismatch
	}
	if len(envelope.States) > maxPersistedInventoryFileSetStates {
		return nil, fmt.Errorf("%w: too many states", ErrPersistedInventoryFileInvalid)
	}
	states := make([]PersistedInventoryState, 0, len(envelope.States))
	seenDevices := make(map[string]struct{}, len(envelope.States))
	seenInstances := make(map[string]struct{}, len(envelope.States))
	for index, value := range envelope.States {
		state, restoreErr := RestorePersistedInventory(value.Revision, value.Device, value.Runner)
		if restoreErr != nil {
			return nil, fmt.Errorf("%w: state %d: %v", ErrPersistedInventoryFileInvalid, index, restoreErr)
		}
		if state.Device.Owner != adapter.evaluationOwner {
			return nil, ErrInventoryOwnerMismatch
		}
		if _, exists := seenDevices[state.Device.DeviceID]; exists {
			return nil, fmt.Errorf("%w: duplicate device %q", ErrPersistedInventoryFileInvalid, state.Device.DeviceID)
		}
		if _, exists := seenInstances[state.Runner.InstanceID]; exists {
			return nil, fmt.Errorf("%w: duplicate instance %q", ErrPersistedInventoryFileInvalid, state.Runner.InstanceID)
		}
		seenDevices[state.Device.DeviceID] = struct{}{}
		seenInstances[state.Runner.InstanceID] = struct{}{}
		states = append(states, state)
	}
	sort.Slice(states, func(left, right int) bool {
		if states[left].Device.DeviceID != states[right].Device.DeviceID {
			return states[left].Device.DeviceID < states[right].Device.DeviceID
		}
		return states[left].Runner.InstanceID < states[right].Runner.InstanceID
	})
	return states, nil
}

func readPrivatePersistedInventoryFile(path string) ([]byte, error) {
	return readPrivatePersistedInventoryFileWithMode(path, false)
}

func readPrivatePersistedInventoryFileSet(path string) ([]byte, error) {
	return readPrivatePersistedInventoryFileWithMode(path, true)
}

func readPrivatePersistedInventoryFileWithMode(path string, requireExact0600 bool) ([]byte, error) {
	file, present, err := statefs.InspectRegular(path)
	if err != nil {
		return nil, err
	}
	if !present {
		return nil, ErrPersistedInventoryFileMissing
	}
	directory, present, err := statefs.InspectDir(filepath.Dir(path))
	if err != nil {
		return nil, err
	}
	if !present {
		return nil, ErrPersistedInventoryFileMissing
	}
	if directory.Mode().Perm()&0o077 != 0 {
		return nil, fmt.Errorf("%w: parent directory is not private", ErrPersistedInventoryFileInvalid)
	}
	if requireExact0600 && file.Mode().Perm() != 0o600 {
		return nil, fmt.Errorf("%w: file mode must be 0600", ErrPersistedInventoryFileInvalid)
	}
	if file.Mode().Perm()&0o077 != 0 {
		return nil, fmt.Errorf("%w: file is not private", ErrPersistedInventoryFileInvalid)
	}
	data, present, err := statefs.ReadRegularUnmodified(path, maxPersistedInventoryFileSize)
	if err != nil {
		return nil, err
	}
	if !present {
		return nil, ErrPersistedInventoryFileMissing
	}
	return data, nil
}

type persistedInventoryFileEnvelope struct {
	SchemaVersion string                  `json:"schema_version"`
	State         PersistedInventoryState `json:"state"`
}

type persistedInventoryFileSetEnvelope struct {
	SchemaVersion string                    `json:"schema_version"`
	Owner         SnapshotOwner             `json:"owner"`
	States        []PersistedInventoryState `json:"states"`
}

func decodePersistedInventoryFile(data []byte) (persistedInventoryFileEnvelope, error) {
	if len(data) == 0 {
		return persistedInventoryFileEnvelope{}, ErrPersistedInventoryFileInvalid
	}
	if err := rejectDuplicateJSONKeys(data); err != nil {
		return persistedInventoryFileEnvelope{}, fmt.Errorf("%w: %v", ErrPersistedInventoryFileInvalid, err)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var envelope persistedInventoryFileEnvelope
	if err := decoder.Decode(&envelope); err != nil {
		return persistedInventoryFileEnvelope{}, fmt.Errorf("%w: %v", ErrPersistedInventoryFileInvalid, err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		if err == nil {
			return persistedInventoryFileEnvelope{}, fmt.Errorf("%w: trailing JSON", ErrPersistedInventoryFileInvalid)
		}
		return persistedInventoryFileEnvelope{}, fmt.Errorf("%w: %v", ErrPersistedInventoryFileInvalid, err)
	}
	if envelope.SchemaVersion != persistedInventoryFileSchema {
		return persistedInventoryFileEnvelope{}, fmt.Errorf("%w: schema_version", ErrPersistedInventoryFileInvalid)
	}
	return envelope, nil
}

func decodePersistedInventoryFileSet(data []byte) (persistedInventoryFileSetEnvelope, error) {
	if len(data) == 0 {
		return persistedInventoryFileSetEnvelope{}, ErrPersistedInventoryFileInvalid
	}
	if err := rejectDuplicateJSONKeys(data); err != nil {
		return persistedInventoryFileSetEnvelope{}, fmt.Errorf("%w: %v", ErrPersistedInventoryFileInvalid, err)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var envelope persistedInventoryFileSetEnvelope
	if err := decoder.Decode(&envelope); err != nil {
		return persistedInventoryFileSetEnvelope{}, fmt.Errorf("%w: %v", ErrPersistedInventoryFileInvalid, err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		if err == nil {
			return persistedInventoryFileSetEnvelope{}, fmt.Errorf("%w: trailing JSON", ErrPersistedInventoryFileInvalid)
		}
		return persistedInventoryFileSetEnvelope{}, fmt.Errorf("%w: %v", ErrPersistedInventoryFileInvalid, err)
	}
	if envelope.SchemaVersion != persistedInventoryFileSetSchema || !validSnapshotOwner(envelope.Owner) {
		return persistedInventoryFileSetEnvelope{}, fmt.Errorf("%w: schema or owner", ErrPersistedInventoryFileInvalid)
	}
	if len(envelope.States) > maxPersistedInventoryFileSetStates {
		return persistedInventoryFileSetEnvelope{}, fmt.Errorf("%w: too many states", ErrPersistedInventoryFileInvalid)
	}
	return envelope, nil
}

// rejectDuplicateJSONKeys catches duplicate keys recursively because
// encoding/json otherwise keeps the last occurrence.  Persisted state must
// have one unambiguous image across process restarts.
func rejectDuplicateJSONKeys(data []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	if err := walkJSONValue(decoder); err != nil {
		return err
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		if err == nil {
			return errors.New("multiple JSON values")
		}
		return err
	}
	return nil
}

func walkJSONValue(decoder *json.Decoder) error {
	token, err := decoder.Token()
	if err != nil {
		return err
	}
	switch delimiter := token.(type) {
	case json.Delim:
		switch delimiter {
		case '{':
			seen := map[string]struct{}{}
			for decoder.More() {
				key, err := decoder.Token()
				if err != nil {
					return err
				}
				keyString, ok := key.(string)
				if !ok {
					return errors.New("object key is not a string")
				}
				if _, exists := seen[keyString]; exists {
					return fmt.Errorf("duplicate object key %q", keyString)
				}
				seen[keyString] = struct{}{}
				if err := walkJSONValue(decoder); err != nil {
					return err
				}
			}
			_, err := decoder.Token()
			return err
		case '[':
			for decoder.More() {
				if err := walkJSONValue(decoder); err != nil {
					return err
				}
			}
			_, err := decoder.Token()
			return err
		default:
			return fmt.Errorf("unexpected delimiter %q", delimiter)
		}
	default:
		return nil
	}
}
