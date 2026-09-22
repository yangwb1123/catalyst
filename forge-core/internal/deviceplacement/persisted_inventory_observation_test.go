package deviceplacement

import (
	"reflect"
	"testing"

	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceinventory"
)

func TestBuildPersistedInventoryObservationCanonicalizesAndSorts(t *testing.T) {
	owner := deviceinventory.SnapshotOwner{Issuer: "issuer", Subject: "user", TenantID: "tenant"}
	first := persistedInventoryObservationState(t, owner, "device-b", "runner-b")
	second := persistedInventoryObservationState(t, owner, "device-a", "runner-a")
	original := append([]deviceinventory.PersistedInventoryState(nil), first, second)
	actual, err := BuildPersistedInventoryObservation([]deviceinventory.PersistedInventoryState{first, second}, owner, 200_000)
	if err != nil {
		t.Fatal(err)
	}
	if err := ValidateSessionDeviceObservationInventory(actual); err != nil {
		t.Fatalf("observation validation: %v", err)
	}
	if actual.Owner != (Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) ||
		actual.EvaluatedAtMS != 200_000 || len(actual.Devices) != 2 ||
		actual.Devices[0].Device.DeviceID != "device-a" || actual.Devices[1].Device.DeviceID != "device-b" ||
		actual.Devices[0].Device.OS != "linux" || actual.Devices[0].Device.Architecture != "amd64" ||
		actual.Devices[0].Device.Runtimes[0] != "go" || !actual.OwnerDeclarationUnverified || !actual.InventoryUnverified ||
		actual.ExecutionAuthorized || actual.ReservationCreated || actual.DispatchPerformed {
		t.Fatalf("observation = %#v", actual)
	}
	if !reflect.DeepEqual([]deviceinventory.PersistedInventoryState{first, second}, original) {
		t.Fatal("input inventory was mutated")
	}
}

func TestBuildPersistedInventoryObservationRetainsDeclaredStates(t *testing.T) {
	owner := deviceinventory.SnapshotOwner{Issuer: "issuer", Subject: "user", TenantID: "tenant"}
	cases := []struct {
		name       string
		approval   string
		liveness   string
		observedAt uint64
		expiresAt  uint64
		wantStatus string
	}{
		{name: "pending", approval: "pending", liveness: "online", observedAt: 120_000, expiresAt: 300_000, wantStatus: deviceinventory.StatusPending},
		{name: "offline", approval: "approved", liveness: "offline", observedAt: 120_000, expiresAt: 300_000, wantStatus: deviceinventory.StatusOffline},
		{name: "stale", approval: "approved", liveness: "online", observedAt: 1_000, expiresAt: 200_000, wantStatus: deviceinventory.StatusStale},
		{name: "expired", approval: "approved", liveness: "online", observedAt: 100_000, expiresAt: 199_000, wantStatus: deviceinventory.StatusStale},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			state := persistedInventoryObservationState(t, owner, "device-"+test.name, "runner-"+test.name)
			state.Device.ApprovalState = test.approval
			state.Runner.Liveness = test.liveness
			state.Runner.ServerObservedAtMS = test.observedAt
			state.Runner.CapabilityLeaseExpiresAtMS = test.expiresAt
			actual, err := BuildPersistedInventoryObservation([]deviceinventory.PersistedInventoryState{state}, owner, 200_000)
			if err != nil {
				t.Fatal(err)
			}
			projection, err := deviceinventory.ProjectPersistedInventory(state, owner, 200_000, deviceinventory.DefaultStaleAfterMS)
			if err != nil || projection.Status != test.wantStatus {
				t.Fatalf("projection=%#v err=%v want status=%q", projection, err, test.wantStatus)
			}
			device := actual.Devices[0].Device
			if device.ApprovalState != test.approval || device.Liveness != test.liveness ||
				uint64(device.SnapshotObservedAtMS) != test.observedAt || uint64(device.LeaseExpiresAtMS) != test.expiresAt {
				t.Fatalf("device = %#v", device)
			}
		})
	}
}

func TestBuildPersistedInventoryObservationRejectsUnsafeAndUnsupportedValues(t *testing.T) {
	owner := deviceinventory.SnapshotOwner{Issuer: "issuer", Subject: "user", TenantID: "tenant"}
	base := persistedInventoryObservationState(t, owner, "device-a", "runner-a")
	tests := []struct {
		name   string
		mutate func(*deviceinventory.PersistedInventoryState)
		want   error
	}{
		{name: "foreign owner", mutate: func(value *deviceinventory.PersistedInventoryState) { value.Device.Owner.Subject = "foreign" }, want: deviceinventory.ErrInventoryOwnerMismatch},
		{name: "future snapshot", mutate: func(value *deviceinventory.PersistedInventoryState) {
			value.Runner.ServerObservedAtMS, value.Runner.CapabilityLeaseExpiresAtMS = 300_000, 400_000
		}, want: deviceinventory.ErrSnapshotFromFuture},
		{name: "unsafe observation", mutate: func(value *deviceinventory.PersistedInventoryState) {
			value.Runner.ServerObservedAtMS, value.Runner.CapabilityLeaseExpiresAtMS = uint64(MaxSafeIntegerMS)+1, uint64(MaxSafeIntegerMS)+1+deviceheartbeat.MinLeaseTTLMS
		}, want: ErrInvalidPersistedInventoryObservation},
		{name: "reservation declaration", mutate: func(value *deviceinventory.PersistedInventoryState) { value.Device.ReservationState = "reserved" }, want: ErrUnsupportedPersistedInventoryObservation},
		{name: "duplicate device", mutate: func(value *deviceinventory.PersistedInventoryState) { value.Runner.InstanceID = "runner-b" }, want: ErrDuplicatePersistedInventoryDevice},
		{name: "duplicate instance", mutate: func(value *deviceinventory.PersistedInventoryState) {
			value.Device.DeviceID, value.Runner.DeviceID = "device-b", "device-b"
		}, want: ErrDuplicatePersistedInventoryInstance},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			left := base
			right := base
			test.mutate(&right)
			values := []deviceinventory.PersistedInventoryState{left, right}
			if test.name == "foreign owner" || test.name == "future snapshot" || test.name == "unsafe observation" || test.name == "reservation declaration" {
				values = []deviceinventory.PersistedInventoryState{right}
			}
			_, err := BuildPersistedInventoryObservation(values, owner, 200_000)
			if err == nil || err.Error() != test.want.Error() {
				t.Fatalf("error = %v, want %v", err, test.want)
			}
		})
	}

	if _, err := BuildPersistedInventoryObservation([]deviceinventory.PersistedInventoryState{base}, owner, uint64(MaxSafeIntegerMS)+1); err != ErrInvalidPersistedInventoryObservation {
		t.Fatalf("unsafe evaluation time error = %v", err)
	}
	withGPU := base
	withGPU.Runner.Capabilities.GPUs = []deviceheartbeat.GPUCapability{{ID: "gpu-a", Vendor: "vendor", MemoryBytes: 1, AvailableMemoryBytes: 1}}
	if _, err := BuildPersistedInventoryObservation([]deviceinventory.PersistedInventoryState{withGPU}, owner, 200_000); err != ErrUnsupportedPersistedInventoryObservation {
		t.Fatalf("GPU error = %v", err)
	}
	for _, test := range []struct {
		name   string
		mutate func(*deviceinventory.PersistedInventoryState)
	}{
		{name: "unsafe available memory", mutate: func(value *deviceinventory.PersistedInventoryState) {
			value.Runner.Capabilities.MemoryBytes = uint64(MaxSafeIntegerMS) + 1
			value.Runner.Capabilities.AvailableMemoryBytes = uint64(MaxSafeIntegerMS) + 1
		}},
		{name: "unsafe available storage", mutate: func(value *deviceinventory.PersistedInventoryState) {
			value.Runner.Capabilities.StorageBytes = uint64(MaxSafeIntegerMS) + 1
			value.Runner.Capabilities.AvailableStorageBytes = uint64(MaxSafeIntegerMS) + 1
		}},
	} {
		t.Run(test.name, func(t *testing.T) {
			value := base
			test.mutate(&value)
			if _, err := BuildPersistedInventoryObservation([]deviceinventory.PersistedInventoryState{value}, owner, 200_000); err != ErrInvalidPersistedInventoryObservation {
				t.Fatalf("unsafe capability error = %v", err)
			}
		})
	}
}

func persistedInventoryObservationState(t *testing.T, owner deviceinventory.SnapshotOwner, deviceID, instanceID string) deviceinventory.PersistedInventoryState {
	t.Helper()
	capabilities, err := deviceheartbeat.NewCapabilitySnapshot(
		"Linux", "AMD64", 8, 7, 16<<30, 8<<30, 100<<30, 50<<30,
		nil, []string{"Go", "Rust"},
	)
	if err != nil {
		t.Fatal(err)
	}
	return deviceinventory.PersistedInventoryState{
		Revision: 1,
		Device: deviceinventory.DeviceRecord{
			DeviceID: deviceID, Owner: owner, ApprovalState: "approved", CordonState: "clear", ReservationState: "none",
		},
		Runner: deviceinventory.RunnerInstanceRecord{
			DeviceID: deviceID, InstanceID: instanceID, Generation: 1, HeartbeatSequence: 1,
			ServerObservedAtMS: 100_000, CapabilityLeaseExpiresAtMS: 200_000, Liveness: "online", Capabilities: capabilities,
		},
	}
}
