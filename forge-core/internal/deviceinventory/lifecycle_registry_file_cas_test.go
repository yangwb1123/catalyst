package deviceinventory

import (
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"sync"
	"testing"

	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/statefs"
)

func TestPersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapterCreatesAndRestoresCanonicalImage(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	path := filepath.Join(t.TempDir(), "lifecycle-registry.json")
	if err := statefs.EnsurePrivateDir(filepath.Dir(path)); err != nil {
		t.Fatal(err)
	}
	adapter, err := NewPersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter(path, owner)
	if err != nil {
		t.Fatal(err)
	}
	initial, err := adapter.ReadSnapshot()
	if err != nil {
		t.Fatalf("read initial snapshot: %v", err)
	}
	if initial.Present() || len(initial.States()) != 0 {
		t.Fatalf("initial snapshot=%#v", initial)
	}

	second := lifecycleRegistryState(t, "device-z", "runner-z")
	first := lifecycleRegistryState(t, "device-a", "runner-a")
	published, err := adapter.ReplaceStatesIfUnchanged(initial, []PersistedEnrollmentHeartbeatLifecycleState{second, first})
	if err != nil {
		t.Fatalf("create registry image: %v", err)
	}
	if !published.Present() || len(published.States()) != 2 {
		t.Fatalf("published snapshot=%#v", published)
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode().Perm() != 0o600 {
		t.Fatalf("file mode=%#o, want 0600", info.Mode().Perm())
	}

	reader, err := NewPersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter(path, owner)
	if err != nil {
		t.Fatal(err)
	}
	states, err := reader.ReadStates()
	if err != nil {
		t.Fatalf("restore published image: %v", err)
	}
	if len(states) != 2 || states[0].Device.DeviceID != "device-a" || states[1].Device.DeviceID != "device-z" {
		t.Fatalf("states=%#v, want deterministic device order", states)
	}

	statesCopy := published.States()
	statesCopy[0].Heartbeat.Instance.Capabilities.Runtimes[0] = "mutated"
	if published.States()[0].Heartbeat.Instance.Capabilities.Runtimes[0] == "mutated" {
		t.Fatal("snapshot exposed capability aliases")
	}
}

func TestPersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapterRejectsStaleTokenAndPreservesCurrentImage(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	path := filepath.Join(t.TempDir(), "lifecycle-registry.json")
	if err := statefs.EnsurePrivateDir(filepath.Dir(path)); err != nil {
		t.Fatal(err)
	}
	adapter, err := NewPersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter(path, owner)
	if err != nil {
		t.Fatal(err)
	}
	initial, err := adapter.ReadSnapshot()
	if err != nil {
		t.Fatal(err)
	}
	first := lifecycleRegistryState(t, "device-a", "runner-a")
	published, err := adapter.ReplaceStatesIfUnchanged(initial, []PersistedEnrollmentHeartbeatLifecycleState{first})
	if err != nil {
		t.Fatal(err)
	}
	second := lifecycleRegistryState(t, "device-b", "runner-b")
	if _, err := adapter.ReplaceStatesIfUnchanged(initial, []PersistedEnrollmentHeartbeatLifecycleState{second}); !errors.Is(err, ErrPersistedLifecycleRegistryFileCASConflict) {
		t.Fatalf("stale create error=%v, want %v", err, ErrPersistedLifecycleRegistryFileCASConflict)
	}

	reader, err := NewPersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter(path, owner)
	if err != nil {
		t.Fatal(err)
	}
	got, err := reader.ReadStates()
	if err != nil {
		t.Fatal(err)
	}
	if want := published.States(); !reflect.DeepEqual(got, want) {
		t.Fatalf("stale CAS changed image: got=%#v want=%#v", got, want)
	}

	current, err := adapter.ReadSnapshot()
	if err != nil {
		t.Fatal(err)
	}
	updated := lifecycleRegistryState(t, "device-a", "runner-a")
	updated.Revision = 2
	updated.Heartbeat.Revision = 2
	updated.Inventory.Revision = 2
	updated.Heartbeat.Instance.HeartbeatSequence = 2
	updated.Inventory.Runner.HeartbeatSequence = 2
	updated.Heartbeat.Instance.ServerObservedAtMS = 112_000
	updated.Inventory.Runner.ServerObservedAtMS = 112_000
	updated.Heartbeat.Instance.CapabilityLeaseExpiresAtMS = 172_000
	updated.Inventory.Runner.CapabilityLeaseExpiresAtMS = 172_000
	updatedAgain, err := adapter.ReplaceStatesIfUnchanged(current, []PersistedEnrollmentHeartbeatLifecycleState{updated})
	if err != nil {
		t.Fatalf("replace current token: %v", err)
	}
	if updatedAgain.States()[0].Revision != 2 {
		t.Fatalf("updated revision=%d, want 2", updatedAgain.States()[0].Revision)
	}
}

func TestPersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapterRejectsOwnerInvalidAndFilesystemDrift(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	foreign := owner
	foreign.Subject = "user-foreign"
	path := filepath.Join(t.TempDir(), "lifecycle-registry.json")
	if err := statefs.EnsurePrivateDir(filepath.Dir(path)); err != nil {
		t.Fatal(err)
	}
	adapter, err := NewPersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter(path, owner)
	if err != nil {
		t.Fatal(err)
	}
	initial, err := adapter.ReadSnapshot()
	if err != nil {
		t.Fatal(err)
	}
	state := lifecycleRegistryState(t, "device-a", "runner-a")
	state.Owner = foreign
	if _, err := adapter.ReplaceStatesIfUnchanged(initial, []PersistedEnrollmentHeartbeatLifecycleState{state}); !errors.Is(err, ErrLifecycleRegistryOwnerMismatch) {
		t.Fatalf("foreign next image error=%v, want %v", err, ErrLifecycleRegistryOwnerMismatch)
	}
	if _, err := adapter.ReplaceStatesIfUnchanged(initial, []PersistedEnrollmentHeartbeatLifecycleState{lifecycleRegistryState(t, "device-a", "runner-a"), lifecycleRegistryState(t, "device-a", "runner-b")}); !errors.Is(err, ErrLifecycleRegistryDuplicateDevice) {
		t.Fatalf("duplicate device error=%v, want %v", err, ErrLifecycleRegistryDuplicateDevice)
	}

	if err := statefs.AtomicWrite(path, lifecycleRegistryJSON(t, owner, []PersistedEnrollmentHeartbeatLifecycleState{stateForOwner(t, owner, "device-a", "runner-a")}), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := adapter.ReplaceStatesIfUnchanged(initial, []PersistedEnrollmentHeartbeatLifecycleState{stateForOwner(t, owner, "device-a", "runner-a")}); !errors.Is(err, ErrPersistedLifecycleRegistryFileCASConflict) {
		t.Fatalf("filesystem drift error=%v, want %v", err, ErrPersistedLifecycleRegistryFileCASConflict)
	}

	if err := os.Chmod(path, 0o640); err != nil {
		t.Fatal(err)
	}
	if _, err := adapter.ReadSnapshot(); !errors.Is(err, ErrPersistedLifecycleRegistryFileCASInvalid) {
		t.Fatalf("broad mode error=%v, want %v", err, ErrPersistedLifecycleRegistryFileCASInvalid)
	}
}

func TestPersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapterSerializesConcurrentCASWriters(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	path := filepath.Join(t.TempDir(), "lifecycle-registry.json")
	if err := statefs.EnsurePrivateDir(filepath.Dir(path)); err != nil {
		t.Fatal(err)
	}
	adapter, err := NewPersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter(path, owner)
	if err != nil {
		t.Fatal(err)
	}
	initial, err := adapter.ReadSnapshot()
	if err != nil {
		t.Fatal(err)
	}
	base := lifecycleRegistryState(t, "device-a", "runner-a")
	if _, err := adapter.ReplaceStatesIfUnchanged(initial, []PersistedEnrollmentHeartbeatLifecycleState{base}); err != nil {
		t.Fatalf("create base image: %v", err)
	}
	current, err := adapter.ReadSnapshot()
	if err != nil {
		t.Fatal(err)
	}

	const writers = 8
	results := make(chan error, writers)
	var group sync.WaitGroup
	group.Add(writers)
	for index := 0; index < writers; index++ {
		go func() {
			defer group.Done()
			next := current.States()
			next[0].Revision = 2
			next[0].Heartbeat.Revision = 2
			next[0].Inventory.Revision = 2
			next[0].Heartbeat.Instance.HeartbeatSequence = 2
			next[0].Inventory.Runner.HeartbeatSequence = 2
			next[0].Heartbeat.Instance.ServerObservedAtMS = 112_000
			next[0].Inventory.Runner.ServerObservedAtMS = 112_000
			next[0].Heartbeat.Instance.CapabilityLeaseExpiresAtMS = 172_000
			next[0].Inventory.Runner.CapabilityLeaseExpiresAtMS = 172_000
			_, replaceErr := adapter.ReplaceStatesIfUnchanged(current, next)
			results <- replaceErr
		}()
	}
	group.Wait()
	close(results)

	successes := 0
	for replaceErr := range results {
		if replaceErr == nil {
			successes++
			continue
		}
		if !errors.Is(replaceErr, ErrPersistedLifecycleRegistryFileCASConflict) {
			t.Fatalf("concurrent CAS error=%v, want conflict after winner", replaceErr)
		}
	}
	if successes != 1 {
		t.Fatalf("concurrent CAS successes=%d, want exactly one", successes)
	}

	restarted, err := NewPersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter(path, owner)
	if err != nil {
		t.Fatal(err)
	}
	states, err := restarted.ReadStates()
	if err != nil {
		t.Fatalf("read post-concurrency image: %v", err)
	}
	if len(states) != 1 || states[0].Revision != 2 || states[0].Heartbeat.Instance.HeartbeatSequence != 2 {
		t.Fatalf("post-concurrency image=%#v, want the single committed revision", states)
	}
	lockInfo, err := os.Stat(path + ".lock")
	if err != nil {
		t.Fatalf("stat CAS lock: %v", err)
	}
	if lockInfo.Mode().Perm() != 0o600 {
		t.Fatalf("CAS lock mode=%#o, want 0600", lockInfo.Mode().Perm())
	}
}

func TestPersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapterRejectsRollbackAndDeletion(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	path := filepath.Join(t.TempDir(), "lifecycle-registry.json")
	if err := statefs.EnsurePrivateDir(filepath.Dir(path)); err != nil {
		t.Fatal(err)
	}
	adapter, err := NewPersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter(path, owner)
	if err != nil {
		t.Fatal(err)
	}
	initial, err := adapter.ReadSnapshot()
	if err != nil {
		t.Fatal(err)
	}
	base := lifecycleRegistryState(t, "device-a", "runner-a")
	first, err := adapter.ReplaceStatesIfUnchanged(initial, []PersistedEnrollmentHeartbeatLifecycleState{base})
	if err != nil {
		t.Fatal(err)
	}
	advanced := first.States()
	advanced[0].Revision = 2
	advanced[0].Heartbeat.Revision = 2
	advanced[0].Inventory.Revision = 2
	advanced[0].Heartbeat.Instance.HeartbeatSequence = 2
	advanced[0].Inventory.Runner.HeartbeatSequence = 2
	advanced[0].Heartbeat.Instance.ServerObservedAtMS = 112_000
	advanced[0].Inventory.Runner.ServerObservedAtMS = 112_000
	advanced[0].Heartbeat.Instance.CapabilityLeaseExpiresAtMS = 172_000
	advanced[0].Inventory.Runner.CapabilityLeaseExpiresAtMS = 172_000
	second, err := adapter.ReplaceStatesIfUnchanged(first, advanced)
	if err != nil {
		t.Fatal(err)
	}

	rollback := base
	if _, err := adapter.ReplaceStatesIfUnchanged(second, []PersistedEnrollmentHeartbeatLifecycleState{rollback}); !errors.Is(err, ErrPersistedLifecycleRegistryFileCASRollback) || !errors.Is(err, ErrPersistedLifecycleRegistryFileCASConflict) {
		t.Fatalf("rollback error=%v, want rollback and conflict classifications", err)
	}
	if _, err := adapter.ReplaceStatesIfUnchanged(second, nil); !errors.Is(err, ErrPersistedLifecycleRegistryFileCASRollback) {
		t.Fatalf("deletion error=%v, want rollback classification", err)
	}

	restarted, err := NewPersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter(path, owner)
	if err != nil {
		t.Fatal(err)
	}
	states, err := restarted.ReadStates()
	if err != nil {
		t.Fatal(err)
	}
	if len(states) != 1 || states[0].Revision != 2 || states[0].Heartbeat.Instance.HeartbeatSequence != 2 {
		t.Fatalf("rollback changed current image=%#v", states)
	}
}

func stateForOwner(t *testing.T, owner deviceidentity.Owner, deviceID, instanceID string) PersistedEnrollmentHeartbeatLifecycleState {
	t.Helper()
	state := lifecycleRegistryState(t, deviceID, instanceID)
	state.Owner = owner
	state.Device.Owner = owner
	state.Inventory.Device.Owner = lifecycleSnapshotOwner(owner)
	return state
}
