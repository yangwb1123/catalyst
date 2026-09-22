package deviceplacement

import (
	"testing"

	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceinventory"
)

func TestBuildPersistedInventoryClientInstanceResourceViewJoinsAndSorts(t *testing.T) {
	owner := deviceinventory.SnapshotOwner{Issuer: "issuer", Subject: "user", TenantID: "tenant"}
	first := clientInstanceResourcePersistedState(t, owner, "device-a", "runner-a")
	first.Device.ReservationState = "reserved"
	first.Runner.Capabilities.GPUs = []deviceheartbeat.GPUCapability{
		{ID: "gpu-a", Vendor: "NVIDIA", MemoryBytes: 16 << 30, AvailableMemoryBytes: 12 << 30},
		{ID: "gpu-b", Vendor: "NVIDIA", MemoryBytes: 8 << 30, AvailableMemoryBytes: 4 << 30},
	}
	second := clientInstanceResourcePersistedState(t, owner, "device-b", "runner-b")
	second.Revision = 2
	second.Runner.Generation = 2
	second.Runner.HeartbeatSequence = 4
	second.Runner.Liveness = "offline"
	second.Device.ApprovalState = "pending"
	second.Device.CordonState = "cordoned"
	instances := []ClientInstanceSessionViewInstance{
		{InstanceID: "client-web", ClientKind: ClientKindWeb, SessionIDs: []string{"conversation-b", "conversation-a"}, ObservedAtMS: 200_500, Status: "idle"},
		{InstanceID: "client-cli", ClientKind: ClientKindCLI, SessionIDs: []string{"conversation-a"}, ObservedAtMS: 200_500, Status: "active"},
	}

	view, err := BuildPersistedInventoryClientInstanceResourceView(
		[]deviceinventory.PersistedInventoryState{second, first}, owner, instances,
	)
	if err != nil {
		t.Fatalf("build resource view: %v", err)
	}
	if err := view.Validate(); err != nil {
		t.Fatalf("validate resource view: %v", err)
	}
	if view.Owner != (Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) ||
		len(view.Instances) != 2 || view.Instances[0].InstanceID != "client-cli" ||
		view.Instances[1].InstanceID != "client-web" ||
		len(view.Instances[1].SessionIDs) != 2 || view.Instances[1].SessionIDs[0] != "conversation-a" ||
		len(view.Devices) != 2 || view.Devices[0].DeviceID != "device-a" ||
		view.Devices[0].RunnerInstanceID != "runner-a" || view.Devices[0].ReservationState != "reserved" ||
		view.Devices[0].GPUCount != 2 || view.Devices[0].AvailableGPUMemoryBytes != 16<<30 ||
		view.Devices[1].ApprovalState != "pending" || view.Devices[1].Liveness != "offline" ||
		view.Authority != (ClientInstanceResourceViewAuthority{}) || !view.ReadOnly {
		t.Fatalf("unexpected resource view: %#v", view)
	}
}

func TestBuildPersistedInventoryClientInstanceResourceViewRejectsForeignAndConfusedRows(t *testing.T) {
	owner := deviceinventory.SnapshotOwner{Issuer: "issuer", Subject: "user", TenantID: "tenant"}
	base := clientInstanceResourcePersistedState(t, owner, "device-a", "runner-a")
	foreign := base
	foreign.Device.Owner.Subject = "other"
	if _, err := BuildPersistedInventoryClientInstanceResourceView([]deviceinventory.PersistedInventoryState{foreign}, owner, nil); err != deviceinventory.ErrInventoryOwnerMismatch {
		t.Fatalf("foreign owner error=%v", err)
	}
	duplicateDevice := base
	duplicateRunner := base
	duplicateRunner.Runner.InstanceID = "runner-b"
	if _, err := BuildPersistedInventoryClientInstanceResourceView([]deviceinventory.PersistedInventoryState{duplicateDevice, duplicateRunner}, owner, nil); err != ErrDuplicatePersistedInventoryDevice {
		t.Fatalf("duplicate device error=%v", err)
	}
	duplicateInstance := base
	duplicateInstance.Device.DeviceID = "device-b"
	duplicateInstance.Runner.DeviceID = "device-b"
	if _, err := BuildPersistedInventoryClientInstanceResourceView([]deviceinventory.PersistedInventoryState{base, duplicateInstance}, owner, nil); err != ErrDuplicatePersistedInventoryInstance {
		t.Fatalf("duplicate runner error=%v", err)
	}
	unsafe := base
	unsafe.Runner.Capabilities.MemoryBytes = uint64(MaxSafeIntegerMS) + 1
	unsafe.Runner.Capabilities.AvailableMemoryBytes = 1
	if _, err := BuildPersistedInventoryClientInstanceResourceView([]deviceinventory.PersistedInventoryState{unsafe}, owner, nil); err != ErrInvalidPersistedInventoryObservation {
		t.Fatalf("unsafe capability error=%v", err)
	}
}

func clientInstanceResourcePersistedState(t *testing.T, owner deviceinventory.SnapshotOwner, deviceID, runnerID string) deviceinventory.PersistedInventoryState {
	t.Helper()
	capabilities, err := deviceheartbeat.NewCapabilitySnapshot(
		"linux", "amd64", 8, 7, 16<<30, 8<<30, 100<<30, 50<<30, nil, []string{"oci"},
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
			DeviceID: deviceID, InstanceID: runnerID, Generation: 1, HeartbeatSequence: 1,
			ServerObservedAtMS: 100_000, CapabilityLeaseExpiresAtMS: 200_000, Liveness: "online", Capabilities: capabilities,
		},
	}
}
