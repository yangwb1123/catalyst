package deviceinventory

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/statefs"
)

func TestPersistedEnrollmentHeartbeatLifecycleFileSetReadAdapterSortsAndRestoresOwnerScopedImages(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	first := lifecycleRegistryState(t, "device-z", "runner-z")
	second := lifecycleRegistryState(t, "device-a", "runner-a")
	path := filepath.Join(t.TempDir(), "lifecycle-registry.json")
	writeLifecycleRegistryFile(t, path, owner, []PersistedEnrollmentHeartbeatLifecycleState{first, second})

	adapter, err := NewPersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter(path, owner)
	if err != nil {
		t.Fatal(err)
	}
	states, err := adapter.ReadStates()
	if err != nil {
		t.Fatalf("read lifecycle registry: %v", err)
	}
	if len(states) != 2 || states[0].Device.DeviceID != "device-a" || states[1].Device.DeviceID != "device-z" {
		t.Fatalf("states are not deterministic: %#v", states)
	}
	if states[0].Owner != owner || states[0].Inventory.Device.Owner != lifecycleSnapshotOwner(owner) {
		t.Fatalf("owner binding was not preserved: %#v", states[0])
	}
	if states[0].Heartbeat.Instance.InstanceID != "runner-a" || states[1].Heartbeat.Instance.InstanceID != "runner-z" {
		t.Fatalf("Runner identity was not restored: %#v", states)
	}

	states[0].Heartbeat.Instance.Capabilities.Runtimes[0] = "mutated"
	reloaded, err := adapter.ReadStates()
	if err != nil {
		t.Fatal(err)
	}
	if reloaded[0].Heartbeat.Instance.Capabilities.Runtimes[0] != "oci" {
		t.Fatal("restored registry image aliases file-owned capability memory")
	}
}

func TestPersistedEnrollmentHeartbeatLifecycleFileSetReadAdapterRejectsOwnerDuplicatesAndMalformedImages(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	foreign := deviceidentity.Owner{Issuer: owner.Issuer, Subject: "user-foreign", TenantID: owner.TenantID}
	first := lifecycleRegistryState(t, "device-a", "runner-a")
	second := lifecycleRegistryState(t, "device-b", "runner-b")
	path := filepath.Join(t.TempDir(), "lifecycle-registry.json")
	adapter, err := NewPersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter(path, owner)
	if err != nil {
		t.Fatal(err)
	}

	cases := []struct {
		name string
		data []byte
		want error
	}{
		{
			name: "foreign envelope owner",
			data: lifecycleRegistryJSON(t, foreign, []PersistedEnrollmentHeartbeatLifecycleState{first}),
			want: ErrPersistedLifecycleRegistryOwnerMismatch,
		},
		{
			name: "foreign member owner",
			data: lifecycleRegistryJSON(t, owner, []PersistedEnrollmentHeartbeatLifecycleState{foreignLifecycleState(first, foreign)}),
			want: ErrPersistedLifecycleRegistryOwnerMismatch,
		},
		{
			name: "duplicate device",
			data: lifecycleRegistryJSON(t, owner, []PersistedEnrollmentHeartbeatLifecycleState{first, first}),
			want: ErrPersistedLifecycleRegistryFileInvalid,
		},
		{
			name: "duplicate Runner instance",
			data: lifecycleRegistryJSON(t, owner, []PersistedEnrollmentHeartbeatLifecycleState{first, duplicateRunnerState(second, first.Heartbeat.Instance.InstanceID)}),
			want: ErrPersistedLifecycleRegistryFileInvalid,
		},
		{
			name: "unknown field",
			data: lifecycleRegistryWithUnknownField(t, owner, []PersistedEnrollmentHeartbeatLifecycleState{first}),
			want: ErrPersistedLifecycleRegistryFileInvalid,
		},
		{
			name: "duplicate field",
			data: lifecycleRegistryWithDuplicateField(t, owner, []PersistedEnrollmentHeartbeatLifecycleState{first}),
			want: ErrPersistedLifecycleRegistryFileInvalid,
		},
		{
			name: "trailing JSON",
			data: append(lifecycleRegistryJSON(t, owner, []PersistedEnrollmentHeartbeatLifecycleState{first}), []byte(`{"trailing":true}`)...),
			want: ErrPersistedLifecycleRegistryFileInvalid,
		},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			data := testCase.data
			writeLifecycleRegistryFile(t, path, owner, nil)
			if err := statefs.AtomicWrite(path, data, 0o600); err != nil {
				t.Fatal(err)
			}
			_, err := adapter.ReadStates()
			if err == nil {
				t.Fatal("malformed lifecycle registry was accepted")
			}
			if testCase.want != nil && !errors.Is(err, testCase.want) {
				t.Fatalf("error=%v, want %v", err, testCase.want)
			}
		})
	}

	missing, err := NewPersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter(
		filepath.Join(t.TempDir(), "missing.json"), owner,
	)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := missing.ReadStates(); !errors.Is(err, ErrPersistedLifecycleRegistryFileMissing) {
		t.Fatalf("missing error=%v", err)
	}
}

func TestPersistedEnrollmentHeartbeatLifecycleFileSetReadAdapterRejectsAliasesAndPermissions(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	state := lifecycleRegistryState(t, "device-a", "runner-a")
	root := t.TempDir()
	path := filepath.Join(root, "lifecycle-registry.json")
	writeLifecycleRegistryFile(t, path, owner, []PersistedEnrollmentHeartbeatLifecycleState{state})
	if err := os.Chmod(path, 0o640); err != nil {
		t.Fatal(err)
	}
	adapter, err := NewPersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter(path, owner)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := adapter.ReadStates(); !errors.Is(err, ErrPersistedLifecycleRegistryFileInvalid) {
		t.Fatalf("broad file mode error=%v", err)
	}

	alias := filepath.Join(root, "alias.json")
	outside := filepath.Join(t.TempDir(), "outside.json")
	if err := os.WriteFile(outside, lifecycleRegistryJSON(t, owner, []PersistedEnrollmentHeartbeatLifecycleState{state}), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, alias); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	aliasAdapter, err := NewPersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter(alias, owner)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := aliasAdapter.ReadStates(); !errors.Is(err, ErrPersistedLifecycleRegistryFileInvalid) {
		t.Fatalf("symlink error=%v", err)
	}
}

func lifecycleRegistryState(t *testing.T, deviceID, instanceID string) PersistedEnrollmentHeartbeatLifecycleState {
	t.Helper()
	input := lifecyclePersistenceInput(t, 1, 1)
	input.Device.DeviceID = deviceID
	input.Device.KeyID = "key-" + deviceID
	input.Proof.DeviceID = deviceID
	input.Proof.KeyID = input.Device.KeyID
	input.Heartbeat.DeviceID = deviceID
	input.Heartbeat.InstanceID = instanceID
	state, _, err := CommitPersistedEnrollmentHeartbeatLifecycle(nil, 0, input)
	if err != nil {
		t.Fatalf("build lifecycle state %s: %v", deviceID, err)
	}
	return state
}

func foreignLifecycleState(value PersistedEnrollmentHeartbeatLifecycleState, owner deviceidentity.Owner) PersistedEnrollmentHeartbeatLifecycleState {
	value.Owner = owner
	value.Device.Owner = owner
	value.Inventory.Device.Owner = lifecycleSnapshotOwner(owner)
	return value
}

func duplicateRunnerState(value PersistedEnrollmentHeartbeatLifecycleState, instanceID string) PersistedEnrollmentHeartbeatLifecycleState {
	value.Heartbeat.Instance.InstanceID = instanceID
	value.Inventory.Runner.InstanceID = instanceID
	return value
}

func lifecycleRegistryJSON(t *testing.T, owner deviceidentity.Owner, states []PersistedEnrollmentHeartbeatLifecycleState) []byte {
	t.Helper()
	data, err := json.Marshal(persistedLifecycleRegistryFileEnvelope{
		SchemaVersion: persistedLifecycleRegistryFileSchema,
		Owner:         owner,
		States:        states,
	})
	if err != nil {
		t.Fatal(err)
	}
	return append(data, '\n')
}

func lifecycleRegistryWithUnknownField(t *testing.T, owner deviceidentity.Owner, states []PersistedEnrollmentHeartbeatLifecycleState) []byte {
	t.Helper()
	trimmed := strings.TrimSpace(string(lifecycleRegistryJSON(t, owner, states)))
	return []byte(trimmed[:len(trimmed)-1] + `,"unexpected":true}`)
}

func lifecycleRegistryWithDuplicateField(t *testing.T, owner deviceidentity.Owner, states []PersistedEnrollmentHeartbeatLifecycleState) []byte {
	t.Helper()
	trimmed := strings.TrimSpace(string(lifecycleRegistryJSON(t, owner, states)))
	comma := strings.IndexByte(trimmed, ',')
	if comma < 0 {
		t.Fatal("registry JSON has no second field")
	}
	return []byte(trimmed[:comma] + `,"schema_version":"` + persistedLifecycleRegistryFileSchema + `"` + trimmed[comma:])
}

func writeLifecycleRegistryFile(t *testing.T, path string, owner deviceidentity.Owner, states []PersistedEnrollmentHeartbeatLifecycleState) {
	t.Helper()
	if err := statefs.AtomicWrite(path, lifecycleRegistryJSON(t, owner, states), 0o600); err != nil {
		t.Fatal(err)
	}
}
