package deviceheartbeat

import (
	"reflect"
	"strconv"
	"testing"
)

func validTestCapabilities() CapabilitySnapshot {
	value, err := NewCapabilitySnapshot(
		"Linux", "X86_64", 8, 4,
		32*1024*1024*1024, 16*1024*1024*1024,
		100*1024*1024*1024, 50*1024*1024*1024,
		[]GPUCapability{{
			ID: "gpu-0", Vendor: "NVIDIA Corporation",
			MemoryBytes: 16 * 1024 * 1024 * 1024, AvailableMemoryBytes: 14 * 1024 * 1024 * 1024,
		}},
		[]string{"docker", "python"},
	)
	if err != nil {
		panic(err)
	}
	return value
}

func TestNewCapabilitySnapshotCanonicalizesEquivalentDeclarations(t *testing.T) {
	value, err := NewCapabilitySnapshot(
		"Linux", "X86_64", 8, 8, 16_384, 8_192, 8_192, 4_096,
		[]GPUCapability{
			{ID: "gpu-b", Vendor: " NVIDIA Corporation ", MemoryBytes: 8_192, AvailableMemoryBytes: 4_096},
			{ID: "gpu-a", Vendor: "AMD", MemoryBytes: 4_096, AvailableMemoryBytes: 0},
		},
		[]string{"Python", "Docker"},
	)
	if err != nil {
		t.Fatalf("canonical snapshot rejected: %v", err)
	}
	if value.OperatingSystem != "linux" || value.Architecture != "x86_64" ||
		value.GPUs[0].ID != "gpu-a" || value.GPUs[1].ID != "gpu-b" ||
		value.GPUs[1].Vendor != "NVIDIA Corporation" ||
		!reflect.DeepEqual(value.Runtimes, []string{"docker", "python"}) {
		t.Fatalf("snapshot was not canonicalized: %#v", value)
	}
}

func TestCapabilitySnapshotRejectsCapacityAndCollectionBoundaryViolations(t *testing.T) {
	base := validTestCapabilities()
	tests := []struct {
		name   string
		mutate func(*CapabilitySnapshot)
		want   CapabilityError
	}{
		{name: "zero cpu", mutate: func(value *CapabilitySnapshot) { value.CPUCores = 0 }, want: ErrInvalidCapabilityValue},
		{name: "available cpu exceeds total", mutate: func(value *CapabilitySnapshot) { value.AvailableCPUCores = value.CPUCores + 1 }, want: ErrInvalidCapabilityValue},
		{name: "cpu limit", mutate: func(value *CapabilitySnapshot) { value.CPUCores = MaxDeviceCPUCores + 1 }, want: ErrInvalidCapabilityValue},
		{name: "zero memory", mutate: func(value *CapabilitySnapshot) { value.MemoryBytes = 0; value.AvailableMemoryBytes = 0 }, want: ErrInvalidCapabilityValue},
		{name: "memory available exceeds total", mutate: func(value *CapabilitySnapshot) { value.AvailableMemoryBytes = value.MemoryBytes + 1 }, want: ErrInvalidCapabilityValue},
		{name: "memory limit", mutate: func(value *CapabilitySnapshot) { value.MemoryBytes = MaxDeviceCapabilityBytes + 1 }, want: ErrInvalidCapabilityValue},
		{name: "storage available exceeds total", mutate: func(value *CapabilitySnapshot) { value.AvailableStorageBytes = value.StorageBytes + 1 }, want: ErrInvalidCapabilityValue},
		{name: "storage limit", mutate: func(value *CapabilitySnapshot) { value.StorageBytes = MaxDeviceCapabilityBytes + 1 }, want: ErrInvalidCapabilityValue},
		{name: "gpu available exceeds total", mutate: func(value *CapabilitySnapshot) { value.GPUs[0].AvailableMemoryBytes = value.GPUs[0].MemoryBytes + 1 }, want: ErrInvalidCapabilityValue},
		{name: "gpu zero memory", mutate: func(value *CapabilitySnapshot) { value.GPUs[0].MemoryBytes = 0; value.GPUs[0].AvailableMemoryBytes = 0 }, want: ErrInvalidCapabilityValue},
		{name: "too many gpus", mutate: func(value *CapabilitySnapshot) {
			value.GPUs = make([]GPUCapability, MaxDeviceGPUCount+1)
			for index := range value.GPUs {
				value.GPUs[index] = GPUCapability{ID: "gpu-" + strconv.Itoa(index), Vendor: "vendor", MemoryBytes: 1}
			}
		}, want: ErrCapabilityLimitExceeded},
		{name: "too many runtimes", mutate: func(value *CapabilitySnapshot) {
			value.Runtimes = make([]string, MaxDeviceRuntimeCount+1)
			for index := range value.Runtimes {
				value.Runtimes[index] = "runtime-" + strconv.Itoa(index)
			}
		}, want: ErrCapabilityLimitExceeded},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			value := base
			value.GPUs = append([]GPUCapability(nil), base.GPUs...)
			value.Runtimes = append([]string(nil), base.Runtimes...)
			test.mutate(&value)
			if err := value.Validate(); err != test.want {
				t.Fatalf("validation error=%v, want %v", err, test.want)
			}
		})
	}
}

func TestCapabilitySnapshotRejectsDuplicateAndMalformedDeclarations(t *testing.T) {
	base := validTestCapabilities()
	tests := []struct {
		name   string
		mutate func(*CapabilitySnapshot)
		want   CapabilityError
	}{
		{name: "duplicate gpu id", mutate: func(value *CapabilitySnapshot) {
			value.GPUs = append(value.GPUs, value.GPUs[0])
		}, want: ErrDuplicateGPUIdentifier},
		{name: "duplicate runtime after case folding", mutate: func(value *CapabilitySnapshot) {
			value.Runtimes = []string{"Docker", "docker"}
		}, want: ErrDuplicateRuntime},
		{name: "invalid runtime punctuation", mutate: func(value *CapabilitySnapshot) {
			value.Runtimes = []string{"oci/container"}
		}, want: ErrInvalidCapabilityValue},
		{name: "invalid gpu id", mutate: func(value *CapabilitySnapshot) {
			value.GPUs[0].ID = "gpu/id"
		}, want: ErrInvalidCapabilityValue},
		{name: "empty vendor", mutate: func(value *CapabilitySnapshot) {
			value.GPUs[0].Vendor = "  "
		}, want: ErrInvalidCapabilityValue},
		{name: "oversized label", mutate: func(value *CapabilitySnapshot) {
			value.OperatingSystem = "x"
			for len(value.OperatingSystem) <= MaxDeviceRuntimeNameBytes {
				value.OperatingSystem += "x"
			}
		}, want: ErrInvalidCapabilityValue},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			value := base
			value.GPUs = append([]GPUCapability(nil), base.GPUs...)
			value.Runtimes = append([]string(nil), base.Runtimes...)
			test.mutate(&value)
			if err := value.Validate(); err != test.want {
				t.Fatalf("validation error=%v, want %v", err, test.want)
			}
		})
	}
}

func TestApplyCarriesCanonicalCapabilitiesIntoInstance(t *testing.T) {
	capabilities := validTestCapabilities()
	heartbeat := Heartbeat{
		DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, Sequence: 1,
		Capabilities: capabilities,
	}
	instance, err := Apply(Device{DeviceID: "device-a", ApprovalState: "approved"}, nil, heartbeat, 100_000, 60_000)
	if err != nil {
		t.Fatalf("heartbeat rejected: %v", err)
	}
	if !reflect.DeepEqual(instance.Capabilities, capabilities) {
		t.Fatalf("instance capabilities=%#v, want %#v", instance.Capabilities, capabilities)
	}
	heartbeat.Capabilities.Runtimes[0] = "mutated"
	if instance.Capabilities.Runtimes[0] == "mutated" {
		t.Fatal("instance shares mutable runtime storage with heartbeat")
	}
}

func TestCommitReplacesAndPreservesCapabilities(t *testing.T) {
	firstCapabilities := validTestCapabilities()
	first, err := Commit(Device{DeviceID: "device-a", ApprovalState: "approved"}, nil, 0, Heartbeat{
		DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, Sequence: 1,
		Capabilities: firstCapabilities,
	}, 100_000, 60_000)
	if err != nil {
		t.Fatalf("initial commit rejected: %v", err)
	}
	secondCapabilities := firstCapabilities
	secondCapabilities.AvailableCPUCores = 2
	second, err := Commit(Device{DeviceID: "device-a", ApprovalState: "approved"}, &first, 1, Heartbeat{
		DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, Sequence: 2,
		Capabilities: secondCapabilities,
	}, 110_000, 60_000)
	if err != nil {
		t.Fatalf("second commit rejected: %v", err)
	}
	if second.Revision != 2 || second.Instance.HeartbeatSequence != 2 ||
		second.Instance.Capabilities.AvailableCPUCores != 2 ||
		first.Instance.Capabilities.AvailableCPUCores != firstCapabilities.AvailableCPUCores {
		t.Fatalf("CAS capability replacement mismatch: first=%#v second=%#v", first, second)
	}
}
