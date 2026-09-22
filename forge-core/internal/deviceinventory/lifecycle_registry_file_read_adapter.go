package deviceinventory

// This file provides the read-only registry-shaped restart boundary for a
// complete set of enrollment/heartbeat lifecycle images.  It deliberately
// restores values only: it does not enroll a device, authenticate a proof,
// consume a challenge, accept a heartbeat, or expose an HTTP route.

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"path/filepath"
	"sort"
	"strings"

	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/statefs"
)

const (
	persistedLifecycleRegistryFileSchema  = "forge.device-enrollment-heartbeat-lifecycle-file-set/v1"
	maxPersistedLifecycleRegistryFileSize = 2 << 20
	maxPersistedLifecycleRegistryStates   = 128
)

// PersistedLifecycleRegistryFileSchemaVersion is the stable wire identifier
// for the complete owner-scoped lifecycle registry file image.  The file
// adapters keep the value validation private, while candidate HTTP consumers
// may use this identifier when projecting an already validated image.
const PersistedLifecycleRegistryFileSchemaVersion = persistedLifecycleRegistryFileSchema

// LifecycleRegistryFileReadError identifies a missing, malformed, or
// owner-ineligible registry snapshot.  It is intentionally separate from the
// value-level lifecycle persistence errors: restoring a snapshot never implies
// that a registry transaction or a heartbeat write occurred.
type LifecycleRegistryFileReadError string

const (
	ErrPersistedLifecycleRegistryFileMissing   LifecycleRegistryFileReadError = "persisted_lifecycle_registry_file_missing"
	ErrPersistedLifecycleRegistryFileInvalid   LifecycleRegistryFileReadError = "persisted_lifecycle_registry_file_invalid"
	ErrPersistedLifecycleRegistryOwnerMismatch LifecycleRegistryFileReadError = "persisted_lifecycle_registry_owner_mismatch"
)

func (e LifecycleRegistryFileReadError) Error() string { return string(e) }

// PersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter restores one
// complete owner-scoped registry image.  Every member carries its identity,
// heartbeat, and inventory values together, so a caller cannot accidentally
// combine a device binding from one restart image with a Runner observation
// from another.  The adapter has no write, clock, listener, or authority
// dependency and is not wired into production routes.
type PersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter struct {
	path  string
	owner deviceidentity.Owner
}

// NewPersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter validates fixed
// parameters without touching the filesystem.  The private parent and file
// are checked by ReadStates, keeping construction side-effect free.
func NewPersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter(
	path string,
	owner deviceidentity.Owner,
) (PersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter, error) {
	if strings.TrimSpace(path) == "" || filepath.Clean(path) == "." || !validLifecycleOwner(owner) {
		return PersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter{}, ErrPersistedLifecycleRegistryFileInvalid
	}
	return PersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter{
		path:  filepath.Clean(path),
		owner: owner,
	}, nil
}

// ReadStates restores all members and verifies the exact owner tuple on both
// the envelope and each lifecycle image.  The result is a deterministic copy
// ordered by device ID then Runner instance ID.  Any malformed member,
// duplicate device/instance, alias, permission drift, or ambiguous JSON
// rejects the complete image so no partial fleet is exposed.
func (adapter PersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter) ReadStates() ([]PersistedEnrollmentHeartbeatLifecycleState, error) {
	data, err := readPrivatePersistedLifecycleRegistryFile(adapter.path)
	if err != nil {
		return nil, err
	}
	envelope, err := decodePersistedLifecycleRegistryFile(data)
	if err != nil {
		return nil, err
	}
	if envelope.Owner != adapter.owner {
		return nil, ErrPersistedLifecycleRegistryOwnerMismatch
	}
	if len(envelope.States) > maxPersistedLifecycleRegistryStates {
		return nil, fmt.Errorf("%w: too many states", ErrPersistedLifecycleRegistryFileInvalid)
	}

	states := make([]PersistedEnrollmentHeartbeatLifecycleState, 0, len(envelope.States))
	seenDevices := make(map[string]struct{}, len(envelope.States))
	seenInstances := make(map[string]struct{}, len(envelope.States))
	for index, image := range envelope.States {
		state, restoreErr := RestorePersistedEnrollmentHeartbeatLifecycle(
			image.Revision,
			image.Owner,
			image.Device,
			image.Heartbeat,
			image.Inventory,
		)
		if restoreErr != nil {
			return nil, fmt.Errorf("%w: state %d: %v", ErrPersistedLifecycleRegistryFileInvalid, index, restoreErr)
		}
		if state.Owner != adapter.owner {
			return nil, ErrPersistedLifecycleRegistryOwnerMismatch
		}
		deviceID := state.Device.DeviceID
		instanceID := state.Heartbeat.Instance.InstanceID
		if _, exists := seenDevices[deviceID]; exists {
			return nil, fmt.Errorf("%w: duplicate device %q", ErrPersistedLifecycleRegistryFileInvalid, deviceID)
		}
		if _, exists := seenInstances[instanceID]; exists {
			return nil, fmt.Errorf("%w: duplicate Runner instance %q", ErrPersistedLifecycleRegistryFileInvalid, state.Heartbeat.Instance.InstanceID)
		}
		seenDevices[deviceID] = struct{}{}
		seenInstances[instanceID] = struct{}{}
		states = append(states, state)
	}
	sort.Slice(states, func(left, right int) bool {
		if states[left].Device.DeviceID != states[right].Device.DeviceID {
			return states[left].Device.DeviceID < states[right].Device.DeviceID
		}
		return states[left].Heartbeat.Instance.InstanceID < states[right].Heartbeat.Instance.InstanceID
	})
	return states, nil
}

func readPrivatePersistedLifecycleRegistryFile(path string) ([]byte, error) {
	file, present, err := statefs.InspectRegular(path)
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrPersistedLifecycleRegistryFileInvalid, err)
	}
	if !present {
		return nil, ErrPersistedLifecycleRegistryFileMissing
	}
	directory, present, err := statefs.InspectDir(filepath.Dir(path))
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrPersistedLifecycleRegistryFileInvalid, err)
	}
	if !present || directory.Mode().Perm()&0o077 != 0 {
		return nil, fmt.Errorf("%w: parent directory is not private", ErrPersistedLifecycleRegistryFileInvalid)
	}
	if file.Mode().Perm() != 0o600 {
		return nil, fmt.Errorf("%w: file mode must be 0600", ErrPersistedLifecycleRegistryFileInvalid)
	}
	data, present, err := statefs.ReadRegularUnmodified(path, maxPersistedLifecycleRegistryFileSize)
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrPersistedLifecycleRegistryFileInvalid, err)
	}
	if !present {
		return nil, ErrPersistedLifecycleRegistryFileMissing
	}
	return data, nil
}

type persistedLifecycleRegistryFileEnvelope struct {
	SchemaVersion string                                       `json:"schema_version"`
	Owner         deviceidentity.Owner                         `json:"owner"`
	States        []PersistedEnrollmentHeartbeatLifecycleState `json:"states"`
}

func decodePersistedLifecycleRegistryFile(data []byte) (persistedLifecycleRegistryFileEnvelope, error) {
	if len(data) == 0 {
		return persistedLifecycleRegistryFileEnvelope{}, ErrPersistedLifecycleRegistryFileInvalid
	}
	if err := rejectDuplicateJSONKeys(data); err != nil {
		return persistedLifecycleRegistryFileEnvelope{}, fmt.Errorf("%w: %v", ErrPersistedLifecycleRegistryFileInvalid, err)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var envelope persistedLifecycleRegistryFileEnvelope
	if err := decoder.Decode(&envelope); err != nil {
		return persistedLifecycleRegistryFileEnvelope{}, fmt.Errorf("%w: %v", ErrPersistedLifecycleRegistryFileInvalid, err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		if err == nil {
			return persistedLifecycleRegistryFileEnvelope{}, fmt.Errorf("%w: trailing JSON", ErrPersistedLifecycleRegistryFileInvalid)
		}
		return persistedLifecycleRegistryFileEnvelope{}, fmt.Errorf("%w: %v", ErrPersistedLifecycleRegistryFileInvalid, err)
	}
	if envelope.SchemaVersion != persistedLifecycleRegistryFileSchema || !validLifecycleOwner(envelope.Owner) {
		return persistedLifecycleRegistryFileEnvelope{}, fmt.Errorf("%w: schema or owner", ErrPersistedLifecycleRegistryFileInvalid)
	}
	if len(envelope.States) > maxPersistedLifecycleRegistryStates {
		return persistedLifecycleRegistryFileEnvelope{}, fmt.Errorf("%w: too many states", ErrPersistedLifecycleRegistryFileInvalid)
	}
	return envelope, nil
}
