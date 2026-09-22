package deviceplacement

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceinventory"
)

func TestBuildPersistedInventoryObservationV2RetainsReservationAndGPUs(t *testing.T) {
	owner := deviceinventory.SnapshotOwner{Issuer: "issuer", Subject: "user", TenantID: "tenant"}
	value := persistedInventoryObservationState(t, owner, "device-a", "runner-a")
	value.Device.ReservationState = "reserved"
	capabilities, err := deviceheartbeat.NewCapabilitySnapshot(
		"Linux", "AMD64", 8, 7, 16<<30, 8<<30, 100<<30, 50<<30,
		[]deviceheartbeat.GPUCapability{
			{ID: "gpu-b", Vendor: "NVIDIA", MemoryBytes: 8 << 30, AvailableMemoryBytes: 4 << 30},
			{ID: "gpu-a", Vendor: "NVIDIA", MemoryBytes: 16 << 30, AvailableMemoryBytes: 12 << 30},
		}, []string{"Go", "Rust"},
	)
	if err != nil {
		t.Fatal(err)
	}
	value.Runner.Capabilities = capabilities
	actual, err := BuildPersistedInventoryObservationV2([]deviceinventory.PersistedInventoryState{value}, owner, 200_000)
	if err != nil {
		t.Fatal(err)
	}
	if err := ValidateSessionDeviceObservationInventoryV2(actual); err != nil {
		t.Fatalf("v2 observation validation: %v", err)
	}
	device := actual.Devices[0].Device
	if device.ReservationState != "reserved" || len(device.GPUs) != 2 ||
		device.GPUs[0].ID != "gpu-a" || device.GPUs[1].ID != "gpu-b" ||
		device.GPUs[0].AvailableMemoryBytes != 12<<30 ||
		actual.ExecutionAuthorized || actual.ReservationCreated || actual.DispatchPerformed {
		t.Fatalf("lossless v2 observation = %#v", actual)
	}
}

func TestValidatePersistedInventoryObservationV2RejectsUnsortedOrUnknownGPU(t *testing.T) {
	owner := deviceinventory.SnapshotOwner{Issuer: "issuer", Subject: "user", TenantID: "tenant"}
	value := persistedInventoryObservationState(t, owner, "device-a", "runner-a")
	actual, err := BuildPersistedInventoryObservationV2([]deviceinventory.PersistedInventoryState{value}, owner, 200_000)
	if err != nil {
		t.Fatal(err)
	}
	actual.Devices[0].Device.GPUs = []GPUDeclarationV2{{ID: "gpu-b", Vendor: "NVIDIA", MemoryBytes: 2, AvailableMemoryBytes: 1}, {ID: "gpu-a", Vendor: "NVIDIA", MemoryBytes: 2, AvailableMemoryBytes: 1}}
	if err := ValidateSessionDeviceObservationInventoryV2(actual); err == nil {
		t.Fatal("unsorted GPUs were accepted")
	}
	actual = mustV2Observation(t, value, owner)
	actual.Devices[0].Device.ReservationState = "unknown"
	if err := ValidateSessionDeviceObservationInventoryV2(actual); err == nil {
		t.Fatal("unknown reservation state was accepted")
	}
	actual = mustV2Observation(t, value, owner)
	actual.Devices[0].Device.AvailableCPUCores = deviceheartbeat.MaxDeviceCPUCores + 1
	if err := ValidateSessionDeviceObservationInventoryV2(actual); err == nil {
		t.Fatal("unsafe CPU declaration was accepted")
	}
	actual = mustV2Observation(t, value, owner)
	actual.Devices[0].Device.LeaseExpiresAtMS = actual.Devices[0].Device.SnapshotObservedAtMS - 1
	if err := ValidateSessionDeviceObservationInventoryV2(actual); err == nil {
		t.Fatal("backward lease interval was accepted")
	}
	actual = mustV2Observation(t, value, owner)
	actual.Devices[0].Device.GPUs = []GPUDeclarationV2{{ID: "gpu-a", Vendor: " NVIDIA ", MemoryBytes: 2, AvailableMemoryBytes: 1}}
	if err := ValidateSessionDeviceObservationInventoryV2(actual); err == nil {
		t.Fatal("untrimmed GPU vendor was accepted")
	}
	actual = mustV2Observation(t, value, owner)
	actual.Devices[0].Device.Runtimes[0] = "Go"
	if err := ValidateSessionDeviceObservationInventoryV2(actual); err == nil {
		t.Fatal("non-normalized runtime was accepted")
	}
	actual = mustV2Observation(t, value, owner)
	actual.Devices[0].Device.Runtimes[0] = "oci/container"
	if err := ValidateSessionDeviceObservationInventoryV2(actual); err == nil {
		t.Fatal("non-canonical runtime punctuation was accepted")
	}
	actual = mustV2Observation(t, value, owner)
	actual.Devices[0].Device.Runtimes[0] = strings.Repeat("r", deviceheartbeat.MaxDeviceRuntimeNameBytes+1)
	if err := ValidateSessionDeviceObservationInventoryV2(actual); err == nil {
		t.Fatal("overlong canonical runtime was accepted")
	}
}

func TestValidatePersistedInventoryObservationV2RejectsInvalidCapabilityLeaseTTL(t *testing.T) {
	owner := deviceinventory.SnapshotOwner{Issuer: "issuer", Subject: "user", TenantID: "tenant"}
	value := persistedInventoryObservationState(t, owner, "device-a", "runner-a")
	actual := mustV2Observation(t, value, owner)
	actual.Devices[0].Device.LeaseExpiresAtMS = actual.Devices[0].Device.SnapshotObservedAtMS + int64(deviceheartbeat.MinLeaseTTLMS) - 1
	if err := ValidateSessionDeviceObservationInventoryV2(actual); err == nil {
		t.Fatal("sub-minimum capability lease was accepted")
	}

	actual = mustV2Observation(t, value, owner)
	actual.Devices[0].Device.LeaseExpiresAtMS = actual.Devices[0].Device.SnapshotObservedAtMS + int64(deviceheartbeat.MaxLeaseTTLMS) + 1
	if err := ValidateSessionDeviceObservationInventoryV2(actual); err == nil {
		t.Fatal("overlong capability lease was accepted")
	}

	actual = mustV2Observation(t, value, owner)
	actual.Devices[0].Device.LeaseExpiresAtMS = actual.Devices[0].Device.SnapshotObservedAtMS + int64(deviceheartbeat.MinLeaseTTLMS)
	if err := ValidateSessionDeviceObservationInventoryV2(actual); err != nil {
		t.Fatalf("minimum capability lease was rejected: %v", err)
	}

	actual = mustV2Observation(t, value, owner)
	actual.Devices[0].Device.LeaseExpiresAtMS = actual.Devices[0].Device.SnapshotObservedAtMS + int64(deviceheartbeat.MaxLeaseTTLMS)
	if err := ValidateSessionDeviceObservationInventoryV2(actual); err != nil {
		t.Fatalf("maximum capability lease was rejected: %v", err)
	}
}

func TestBuildPersistedInventoryObservationV2RejectsUnsafeCountersAndGPUBytes(t *testing.T) {
	owner := deviceinventory.SnapshotOwner{Issuer: "issuer", Subject: "user", TenantID: "tenant"}
	value := persistedInventoryObservationState(t, owner, "device-a", "runner-a")
	value.Revision = uint64(MaxSafeIntegerMS) + 1
	if _, err := BuildPersistedInventoryObservationV2([]deviceinventory.PersistedInventoryState{value}, owner, 200_000); err == nil {
		t.Fatal("unsafe revision was accepted")
	}
	value = persistedInventoryObservationState(t, owner, "device-a", "runner-a")
	value.Runner.Generation = uint64(MaxSafeIntegerMS) + 1
	if _, err := BuildPersistedInventoryObservationV2([]deviceinventory.PersistedInventoryState{value}, owner, 200_000); err == nil {
		t.Fatal("unsafe generation was accepted")
	}
	value = persistedInventoryObservationState(t, owner, "device-a", "runner-a")
	value.Runner.HeartbeatSequence = uint64(MaxSafeIntegerMS) + 1
	if _, err := BuildPersistedInventoryObservationV2([]deviceinventory.PersistedInventoryState{value}, owner, 200_000); err == nil {
		t.Fatal("unsafe heartbeat sequence was accepted")
	}
	capabilities, err := deviceheartbeat.NewCapabilitySnapshot(
		"Linux", "AMD64", 8, 7, 16<<30, 8<<30, 100<<30, 50<<30,
		[]deviceheartbeat.GPUCapability{{ID: "gpu-a", Vendor: "NVIDIA", MemoryBytes: uint64(MaxSafeIntegerMS) + 1, AvailableMemoryBytes: 1}},
		[]string{"Go"},
	)
	if err != nil {
		t.Fatal(err)
	}
	value = persistedInventoryObservationState(t, owner, "device-a", "runner-a")
	value.Runner.Capabilities = capabilities
	if _, err := BuildPersistedInventoryObservationV2([]deviceinventory.PersistedInventoryState{value}, owner, 200_000); err == nil {
		t.Fatal("unsafe GPU memory was accepted")
	}
}

func TestBuildPersistedInventoryObservationV2RejectsAggregateGPUBytesAboveJSONSafeInteger(t *testing.T) {
	owner := deviceinventory.SnapshotOwner{Issuer: "issuer", Subject: "user", TenantID: "tenant"}
	value := persistedInventoryObservationState(t, owner, "device-a", "runner-a")
	available := uint64(5_000_000_000_000_000)
	capabilities, err := deviceheartbeat.NewCapabilitySnapshot(
		"Linux", "AMD64", 8, 7, 16<<30, 8<<30, 100<<30, 50<<30,
		[]deviceheartbeat.GPUCapability{
			{ID: "gpu-a", Vendor: "NVIDIA", MemoryBytes: available, AvailableMemoryBytes: available},
			{ID: "gpu-b", Vendor: "NVIDIA", MemoryBytes: available, AvailableMemoryBytes: available},
		}, []string{"Go"},
	)
	if err != nil {
		t.Fatal(err)
	}
	value.Runner.Capabilities = capabilities
	if _, err := BuildPersistedInventoryObservationV2([]deviceinventory.PersistedInventoryState{value}, owner, 200_000); err == nil {
		t.Fatal("aggregate GPU memory above JSON-safe integer was accepted")
	}
}

func TestBuildPersistedInventoryObservationV2MatchesSharedFixture(t *testing.T) {
	fixturePath := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "forge-device-inventory-observation-v2.json")
	bytes, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatal(err)
	}
	var expected SessionDeviceObservationInventoryV2
	if err := json.Unmarshal(bytes, &expected); err != nil {
		t.Fatal(err)
	}
	owner := deviceinventory.SnapshotOwner{Issuer: "issuer", Subject: "user", TenantID: "tenant"}
	first := persistedInventoryObservationState(t, owner, "device-a", "runner-a")
	first.Device.ReservationState = "reserved"
	capabilities, err := deviceheartbeat.NewCapabilitySnapshot(
		"Linux", "AMD64", 8, 7, 16<<30, 8<<30, 100<<30, 50<<30,
		[]deviceheartbeat.GPUCapability{
			{ID: "gpu-b", Vendor: "NVIDIA", MemoryBytes: 8 << 30, AvailableMemoryBytes: 4 << 30},
			{ID: "gpu-a", Vendor: "NVIDIA", MemoryBytes: 16 << 30, AvailableMemoryBytes: 12 << 30},
		}, []string{"Go", "Rust"},
	)
	if err != nil {
		t.Fatal(err)
	}
	first.Runner.Capabilities = capabilities
	second := persistedInventoryObservationState(t, owner, "device-b", "runner-b")
	second.Revision = 2
	second.Device.ApprovalState = "pending"
	second.Device.CordonState = "cordoned"
	second.Runner.Generation = 2
	second.Runner.HeartbeatSequence = 4
	second.Runner.Liveness = "offline"
	actual, err := BuildPersistedInventoryObservationV2([]deviceinventory.PersistedInventoryState{second, first}, owner, 200_000)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(actual, expected) {
		t.Fatalf("v2 fixture mismatch\nactual=%#v\nexpected=%#v", actual, expected)
	}
}

func mustV2Observation(t *testing.T, value deviceinventory.PersistedInventoryState, owner deviceinventory.SnapshotOwner) SessionDeviceObservationInventoryV2 {
	t.Helper()
	actual, err := BuildPersistedInventoryObservationV2([]deviceinventory.PersistedInventoryState{value}, owner, 200_000)
	if err != nil {
		t.Fatal(err)
	}
	return actual
}
