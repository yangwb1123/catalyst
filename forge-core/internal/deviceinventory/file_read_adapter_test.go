package deviceinventory

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/statefs"
)

const (
	fileReadAdapterChildEnv = "FORGE_DEVICE_INVENTORY_FILE_READ_CHILD"
	fileReadAdapterPathEnv  = "FORGE_DEVICE_INVENTORY_FILE_READ_PATH"
)

func TestPersistedInventoryFileReadAdapterProjectsOwnerScopedState(t *testing.T) {
	state := validPersistedInventory(t, 3)
	path := writePersistedInventoryPreviewFile(t, state)

	adapter, err := NewPersistedInventoryFileReadAdapter(
		path, state.Device.Owner, 1_500, DefaultStaleAfterMS,
	)
	if err != nil {
		t.Fatalf("construct file read adapter: %v", err)
	}
	projection, err := adapter.Read()
	if err != nil {
		t.Fatalf("read persisted inventory: %v", err)
	}
	want := PersistedInventoryProjection{
		Revision:         3,
		DeviceID:         "device-a",
		InstanceID:       "runner-a",
		Status:           StatusOnline,
		Fresh:            true,
		DeclaredEligible: true,
	}
	if projection != want {
		t.Fatalf("projection=%#v, want %#v", projection, want)
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatalf("stat persisted inventory: %v", err)
	}
	if got := info.Mode().Perm(); got != 0o600 {
		t.Fatalf("file mode=%#o, want 0600", got)
	}
}

func TestPersistedInventoryFileReadAdapterRejectsForeignOwnerAndMissingState(t *testing.T) {
	state := validPersistedInventory(t, 1)
	path := writePersistedInventoryPreviewFile(t, state)
	foreign := state.Device.Owner
	foreign.Subject = "other-user"
	adapter, err := NewPersistedInventoryFileReadAdapter(path, foreign, 1_500, DefaultStaleAfterMS)
	if err != nil {
		t.Fatalf("construct foreign adapter: %v", err)
	}
	if _, err := adapter.Read(); err != ErrInventoryOwnerMismatch {
		t.Fatalf("foreign owner error=%v, want %v", err, ErrInventoryOwnerMismatch)
	}

	missingPath := filepath.Join(t.TempDir(), "missing.json")
	missingAdapter, err := NewPersistedInventoryFileReadAdapter(missingPath, state.Device.Owner, 1_500, DefaultStaleAfterMS)
	if err != nil {
		t.Fatalf("construct missing adapter: %v", err)
	}
	if _, err := missingAdapter.Read(); err != ErrPersistedInventoryFileMissing {
		t.Fatalf("missing state error=%v, want %v", err, ErrPersistedInventoryFileMissing)
	}
}

func TestPersistedInventoryFileReadAdapterSurvivesProcessRestartAndRevisionReplacement(t *testing.T) {
	if os.Getenv(fileReadAdapterChildEnv) == "1" {
		path := os.Getenv(fileReadAdapterPathEnv)
		owner := SnapshotOwner{Issuer: "issuer", Subject: "user", TenantID: "tenant"}
		adapter, err := NewPersistedInventoryFileReadAdapter(path, owner, 2_500, DefaultStaleAfterMS)
		if err != nil {
			fmt.Printf("construct: %v", err)
			return
		}
		projection, err := adapter.Read()
		if err != nil {
			fmt.Printf("read: %v", err)
			return
		}
		fmt.Printf("%d %s %s %t", projection.Revision, projection.DeviceID, projection.InstanceID, projection.Fresh)
		return
	}

	state := validPersistedInventory(t, 1)
	nextRunner := state.Runner
	nextRunner.HeartbeatSequence = 2
	nextRunner.ServerObservedAtMS = 2_000
	nextRunner.CapabilityLeaseExpiresAtMS = 6_000
	next, err := CommitPersistedInventory(&state, 1, state.Device, nextRunner)
	if err != nil {
		t.Fatalf("commit replacement: %v", err)
	}
	if _, err := CommitPersistedInventory(&next, 1, next.Device, next.Runner); err != ErrRevisionConflict {
		t.Fatalf("replayed revision error=%v, want %v", err, ErrRevisionConflict)
	}
	path := writePersistedInventoryPreviewFile(t, next)

	command := exec.Command(os.Args[0], "-test.run=^TestPersistedInventoryFileReadAdapterSurvivesProcessRestartAndRevisionReplacement$")
	command.Env = append(os.Environ(), fileReadAdapterChildEnv+"=1", fileReadAdapterPathEnv+"="+path)
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("restart reader: %v output=%q", err, output)
	}
	if got, want := strings.TrimSpace(string(output)), "2 device-a runner-a true"; got != want && got != want+"PASS" {
		t.Fatalf("restart projection=%q, want %q (with optional test PASS suffix)", got, want)
	}
}

func TestPersistedInventoryFileReadAdapterRejectsAmbiguousOrAliasedFiles(t *testing.T) {
	state := validPersistedInventory(t, 1)
	canonical, err := json.Marshal(persistedInventoryFileEnvelope{
		SchemaVersion: persistedInventoryFileSchema,
		State:         state,
	})
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(t.TempDir(), "inventory.json")
	if err := os.WriteFile(path, append([]byte(`{"schema_version":"forge.device-inventory-file/v1","schema_version":"forge.device-inventory-file/v1","state":`), append(canonical[strings.IndexByte(string(canonical), '{')+1:], '}')...), 0o600); err != nil {
		t.Fatal(err)
	}
	adapter, err := NewPersistedInventoryFileReadAdapter(path, state.Device.Owner, 1_500, DefaultStaleAfterMS)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := adapter.Read(); err == nil || !strings.Contains(err.Error(), string(ErrPersistedInventoryFileInvalid)) {
		t.Fatalf("duplicate file error=%v, want invalid file", err)
	}

	validPath := writePersistedInventoryPreviewFile(t, state)
	aliasPath := filepath.Join(t.TempDir(), "alias.json")
	if err := os.Symlink(validPath, aliasPath); err != nil {
		t.Fatal(err)
	}
	aliasAdapter, err := NewPersistedInventoryFileReadAdapter(aliasPath, state.Device.Owner, 1_500, DefaultStaleAfterMS)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := aliasAdapter.Read(); err == nil {
		t.Fatal("symlink inventory file was accepted")
	}

	permissivePath := writePersistedInventoryPreviewFile(t, state)
	if err := os.Chmod(permissivePath, 0o640); err != nil {
		t.Fatal(err)
	}
	permissiveAdapter, err := NewPersistedInventoryFileReadAdapter(permissivePath, state.Device.Owner, 1_500, DefaultStaleAfterMS)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := permissiveAdapter.Read(); err == nil || !strings.Contains(err.Error(), string(ErrPersistedInventoryFileInvalid)) {
		t.Fatalf("permissive file error=%v, want invalid file", err)
	}
}

func TestPersistedInventoryFileSetReadAdapterReadsAllStatesAndAtomicReplacement(t *testing.T) {
	owner := SnapshotOwner{Issuer: "issuer", Subject: "user", TenantID: "tenant"}
	first := validPersistedInventory(t, 1)
	second := validPersistedInventory(t, 2)
	second.Device.DeviceID = "device-b"
	second.Runner.DeviceID = "device-b"
	second.Runner.InstanceID = "runner-b"
	path := writePersistedInventoryFileSetPreviewFile(t, owner, []PersistedInventoryState{second, first})
	adapter, err := NewPersistedInventoryFileSetReadAdapter(path, owner)
	if err != nil {
		t.Fatalf("construct file-set read adapter: %v", err)
	}
	states, err := adapter.ReadStates()
	if err != nil {
		t.Fatalf("read initial file-set: %v", err)
	}
	if len(states) != 2 || states[0].Device.DeviceID != "device-a" || states[0].Runner.InstanceID != "runner-a" ||
		states[1].Device.DeviceID != "device-b" || states[1].Runner.InstanceID != "runner-b" {
		t.Fatalf("initial states=%#v, want deterministic device/instance order", states)
	}

	nextRunner := first.Runner
	nextRunner.HeartbeatSequence = 2
	nextRunner.ServerObservedAtMS = 2_000
	nextRunner.CapabilityLeaseExpiresAtMS = 6_000
	nextFirst, err := CommitPersistedInventory(&first, first.Revision, first.Device, nextRunner)
	if err != nil {
		t.Fatalf("commit replacement state: %v", err)
	}
	if err := writePersistedInventoryFileSetPreview(path, owner, []PersistedInventoryState{second, nextFirst}); err != nil {
		t.Fatalf("atomically replace file-set: %v", err)
	}
	updated, err := adapter.ReadStates()
	if err != nil {
		t.Fatalf("read replaced file-set: %v", err)
	}
	if len(updated) != 2 || updated[0].Revision != nextFirst.Revision || updated[0].Runner.HeartbeatSequence != 2 ||
		updated[1].Revision != second.Revision || updated[1].Runner.HeartbeatSequence != second.Runner.HeartbeatSequence {
		t.Fatalf("updated states=%#v, want atomic replacement with both instances", updated)
	}
}

func TestPersistedInventoryFileSetReadAdapterRejectsForeignDuplicateAndOversizedImages(t *testing.T) {
	owner := SnapshotOwner{Issuer: "issuer", Subject: "user", TenantID: "tenant"}
	first := validPersistedInventory(t, 1)
	second := first
	second.Device.DeviceID = "device-b"
	second.Runner.DeviceID = "device-b"
	second.Runner.InstanceID = "runner-b"
	tests := []struct {
		name   string
		owner  SnapshotOwner
		states []PersistedInventoryState
		want   error
	}{
		{name: "foreign envelope owner", owner: SnapshotOwner{Issuer: owner.Issuer, Subject: "foreign", TenantID: owner.TenantID}, states: []PersistedInventoryState{first}, want: ErrInventoryOwnerMismatch},
		{name: "foreign state owner", owner: owner, states: []PersistedInventoryState{func() PersistedInventoryState {
			value := first
			value.Device.Owner.Subject = "foreign"
			return value
		}()}, want: ErrInventoryOwnerMismatch},
		{name: "duplicate device", owner: owner, states: []PersistedInventoryState{first, func() PersistedInventoryState {
			value := second
			value.Device.DeviceID = first.Device.DeviceID
			value.Runner.DeviceID = first.Runner.DeviceID
			return value
		}()}},
		{name: "duplicate instance", owner: owner, states: []PersistedInventoryState{first, func() PersistedInventoryState {
			value := second
			value.Runner.InstanceID = first.Runner.InstanceID
			return value
		}()}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			path := writePersistedInventoryFileSetPreviewFile(t, test.owner, test.states)
			adapter, err := NewPersistedInventoryFileSetReadAdapter(path, owner)
			if err != nil {
				t.Fatalf("construct adapter: %v", err)
			}
			_, err = adapter.ReadStates()
			if test.want == ErrInventoryOwnerMismatch {
				if err != test.want {
					t.Fatalf("error=%v, want %v", err, test.want)
				}
				return
			}
			if err == nil || !strings.Contains(err.Error(), string(ErrPersistedInventoryFileInvalid)) {
				t.Fatalf("error=%v, want invalid file-set image", err)
			}
		})
	}

	oversized := make([]PersistedInventoryState, 0, maxPersistedInventoryFileSetStates+1)
	for index := 0; index < maxPersistedInventoryFileSetStates+1; index++ {
		value := first
		value.Device.DeviceID = fmt.Sprintf("device-%03d", index)
		value.Runner.DeviceID = value.Device.DeviceID
		value.Runner.InstanceID = fmt.Sprintf("runner-%03d", index)
		oversized = append(oversized, value)
	}
	path := writePersistedInventoryFileSetPreviewFile(t, owner, oversized)
	adapter, err := NewPersistedInventoryFileSetReadAdapter(path, owner)
	if err != nil {
		t.Fatalf("construct oversized adapter: %v", err)
	}
	if _, err := adapter.ReadStates(); err == nil || !strings.Contains(err.Error(), string(ErrPersistedInventoryFileInvalid)) {
		t.Fatalf("oversized error=%v, want invalid file-set image", err)
	}
}

func TestPersistedInventoryFileSetReadAdapterRejectsAmbiguousUnknownTrailingAndNonPrivateFiles(t *testing.T) {
	owner := SnapshotOwner{Issuer: "issuer", Subject: "user", TenantID: "tenant"}
	state := validPersistedInventory(t, 1)
	validPath := writePersistedInventoryFileSetPreviewFile(t, owner, []PersistedInventoryState{state})
	validBytes, err := os.ReadFile(validPath)
	if err != nil {
		t.Fatal(err)
	}

	checks := []struct {
		name string
		data []byte
	}{
		{name: "trailing json", data: append(append([]byte(nil), validBytes...), []byte("{}")...)},
		{name: "unknown field", data: append(func() []byte {
			base := bytes.TrimSpace(validBytes)
			return base[:len(base)-1]
		}(), []byte(`,"extra":true}`)...)},
		{name: "duplicate root key", data: []byte(`{"schema_version":"forge.device-inventory-file-set/v1","schema_version":"forge.device-inventory-file-set/v1","owner":{"issuer":"issuer","subject":"user","tenant_id":"tenant"},"states":[]}`)},
	}
	for _, check := range checks {
		t.Run(check.name, func(t *testing.T) {
			path := filepath.Join(t.TempDir(), "inventory-set.json")
			if err := statefs.AtomicWrite(path, check.data, 0o600); err != nil {
				t.Fatal(err)
			}
			adapter, err := NewPersistedInventoryFileSetReadAdapter(path, owner)
			if err != nil {
				t.Fatal(err)
			}
			if _, err := adapter.ReadStates(); err == nil || !strings.Contains(err.Error(), string(ErrPersistedInventoryFileInvalid)) {
				t.Fatalf("error=%v, want invalid file-set image", err)
			}
		})
	}

	permissivePath := writePersistedInventoryFileSetPreviewFile(t, owner, []PersistedInventoryState{state})
	if err := os.Chmod(permissivePath, 0o640); err != nil {
		t.Fatal(err)
	}
	permissiveAdapter, err := NewPersistedInventoryFileSetReadAdapter(permissivePath, owner)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := permissiveAdapter.ReadStates(); err == nil || !strings.Contains(err.Error(), string(ErrPersistedInventoryFileInvalid)) {
		t.Fatalf("permissive file error=%v, want invalid file-set image", err)
	}

	readOnlyPath := writePersistedInventoryFileSetPreviewFile(t, owner, []PersistedInventoryState{state})
	if err := os.Chmod(readOnlyPath, 0o400); err != nil {
		t.Fatal(err)
	}
	readOnlyAdapter, err := NewPersistedInventoryFileSetReadAdapter(readOnlyPath, owner)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := readOnlyAdapter.ReadStates(); err == nil || !strings.Contains(err.Error(), string(ErrPersistedInventoryFileInvalid)) {
		t.Fatalf("non-0600 file error=%v, want invalid file-set image", err)
	}

	aliasPath := filepath.Join(t.TempDir(), "alias.json")
	if err := os.Symlink(validPath, aliasPath); err != nil {
		t.Fatal(err)
	}
	aliasAdapter, err := NewPersistedInventoryFileSetReadAdapter(aliasPath, owner)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := aliasAdapter.ReadStates(); err == nil {
		t.Fatal("symlink file-set image was accepted")
	}
}

func writePersistedInventoryPreviewFile(t *testing.T, state PersistedInventoryState) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "inventory.json")
	data, err := json.Marshal(persistedInventoryFileEnvelope{
		SchemaVersion: persistedInventoryFileSchema,
		State:         state,
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := statefs.AtomicWrite(path, append(data, '\n'), 0o600); err != nil {
		t.Fatalf("write preview inventory file: %v", err)
	}
	return path
}

func writePersistedInventoryFileSetPreviewFile(t *testing.T, owner SnapshotOwner, states []PersistedInventoryState) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "inventory-set.json")
	if err := writePersistedInventoryFileSetPreview(path, owner, states); err != nil {
		t.Fatalf("write preview inventory file-set: %v", err)
	}
	return path
}

func writePersistedInventoryFileSetPreview(path string, owner SnapshotOwner, states []PersistedInventoryState) error {
	data, err := json.Marshal(persistedInventoryFileSetEnvelope{
		SchemaVersion: persistedInventoryFileSetSchema,
		Owner:         owner,
		States:        states,
	})
	if err != nil {
		return err
	}
	return statefs.AtomicWrite(path, append(data, '\n'), 0o600)
}
