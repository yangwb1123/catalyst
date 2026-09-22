package deviceheartbeat

import (
	"reflect"
	"testing"
)

func TestCommitRejectsMalformedPersistedSnapshot(t *testing.T) {
	device := Device{DeviceID: "device-a", TenantID: "tenant-1", ApprovalState: "approved"}
	capabilities := validTestCapabilities()
	base := PersistedInstance{Revision: 1, Instance: Instance{
		DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, HeartbeatSequence: 1,
		ServerObservedAtMS: 100_000, CapabilityLeaseExpiresAtMS: 160_000,
		Capabilities: capabilities,
	}}
	tests := []struct {
		name   string
		mutate func(*Instance)
	}{
		{name: "empty instance id", mutate: func(value *Instance) { value.InstanceID = "" }},
		{name: "zero generation", mutate: func(value *Instance) { value.Generation = 0 }},
		{name: "zero sequence", mutate: func(value *Instance) { value.HeartbeatSequence = 0 }},
		{name: "lease before observation", mutate: func(value *Instance) {
			value.CapabilityLeaseExpiresAtMS = value.ServerObservedAtMS - 1
		}},
		{name: "lease below minimum", mutate: func(value *Instance) {
			value.CapabilityLeaseExpiresAtMS = value.ServerObservedAtMS + MinLeaseTTLMS - 1
		}},
		{name: "lease above maximum", mutate: func(value *Instance) {
			value.CapabilityLeaseExpiresAtMS = value.ServerObservedAtMS + MaxLeaseTTLMS + 1
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			current := base
			test.mutate(&current.Instance)
			result, err := Commit(device, &current, current.Revision, Heartbeat{
				DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, Sequence: 2,
				Capabilities: capabilities,
			}, 110_000, 60_000)
			if err != ErrInvalidPersistedState || !reflect.DeepEqual(result, PersistedInstance{}) {
				t.Fatalf("result=%#v err=%v, want zero result and %v", result, err, ErrInvalidPersistedState)
			}
		})
	}
}

func TestCommitRejectsNonCanonicalPersistedCapabilities(t *testing.T) {
	capabilities, err := NewCapabilitySnapshot(
		"Linux", "X86_64", 8, 8, 16_384, 8_192, 8_192, 4_096,
		[]GPUCapability{
			{ID: "gpu-b", Vendor: " NVIDIA Corporation ", MemoryBytes: 8_192, AvailableMemoryBytes: 4_096},
			{ID: "gpu-a", Vendor: "AMD", MemoryBytes: 4_096, AvailableMemoryBytes: 0},
		},
		[]string{"Python", "Docker"},
	)
	if err != nil {
		t.Fatalf("canonical capabilities rejected: %v", err)
	}
	base := PersistedInstance{Revision: 1, Instance: Instance{
		DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, HeartbeatSequence: 1,
		ServerObservedAtMS: 100_000, CapabilityLeaseExpiresAtMS: 160_000,
		Capabilities: capabilities,
	}}
	heartbeat := Heartbeat{
		DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, Sequence: 2,
		Capabilities: capabilities,
	}
	tests := []struct {
		name   string
		mutate func(*CapabilitySnapshot)
	}{
		{name: "operating system spelling", mutate: func(value *CapabilitySnapshot) {
			value.OperatingSystem = "Linux"
		}},
		{name: "runtime order", mutate: func(value *CapabilitySnapshot) {
			value.Runtimes = []string{"python", "docker"}
		}},
		{name: "gpu order", mutate: func(value *CapabilitySnapshot) {
			value.GPUs = []GPUCapability{value.GPUs[1], value.GPUs[0]}
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			current := base
			current.Instance.Capabilities.GPUs = append([]GPUCapability(nil), base.Instance.Capabilities.GPUs...)
			current.Instance.Capabilities.Runtimes = append([]string(nil), base.Instance.Capabilities.Runtimes...)
			test.mutate(&current.Instance.Capabilities)
			before := current
			before.Instance.Capabilities.GPUs = append([]GPUCapability(nil), current.Instance.Capabilities.GPUs...)
			before.Instance.Capabilities.Runtimes = append([]string(nil), current.Instance.Capabilities.Runtimes...)
			result, err := Commit(Device{DeviceID: "device-a", ApprovalState: "approved"}, &current, current.Revision, heartbeat, 110_000, 60_000)
			if err != ErrInvalidPersistedState || !reflect.DeepEqual(result, PersistedInstance{}) {
				t.Fatalf("result=%#v err=%v, want zero result and %v", result, err, ErrInvalidPersistedState)
			}
			if !reflect.DeepEqual(current, before) {
				t.Fatalf("rejected noncanonical snapshot mutated current: got=%#v want=%#v", current, before)
			}
		})
	}
}

func TestCommitAcceptsCanonicalPersistedCapabilities(t *testing.T) {
	capabilities, err := NewCapabilitySnapshot(
		"Linux", "X86_64", 8, 8, 16_384, 8_192, 8_192, 4_096,
		[]GPUCapability{
			{ID: "gpu-b", Vendor: " NVIDIA Corporation ", MemoryBytes: 8_192, AvailableMemoryBytes: 4_096},
			{ID: "gpu-a", Vendor: "AMD", MemoryBytes: 4_096, AvailableMemoryBytes: 0},
		},
		[]string{"Python", "Docker"},
	)
	if err != nil {
		t.Fatalf("canonical capabilities rejected: %v", err)
	}
	current := &PersistedInstance{Revision: 1, Instance: Instance{
		DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, HeartbeatSequence: 1,
		ServerObservedAtMS: 100_000, CapabilityLeaseExpiresAtMS: 160_000,
		Capabilities: capabilities,
	}}
	result, err := Commit(Device{DeviceID: "device-a", ApprovalState: "approved"}, current, 1, Heartbeat{
		DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, Sequence: 2,
		Capabilities: capabilities,
	}, 110_000, 60_000)
	if err != nil {
		t.Fatalf("canonical persisted capabilities rejected: %v", err)
	}
	if result.Revision != 2 || !capabilitySnapshotsEqual(result.Instance.Capabilities, capabilities) {
		t.Fatalf("canonical replacement=%#v", result)
	}
}

func TestCommitTreatsNilAndEmptyCapabilityCollectionsAsEquivalent(t *testing.T) {
	capabilities, err := NewCapabilitySnapshot(
		"linux", "amd64", 8, 8, 16_384, 16_384, 0, 0, nil, nil,
	)
	if err != nil {
		t.Fatalf("empty collections rejected: %v", err)
	}
	capabilities.GPUs = nil
	capabilities.Runtimes = nil
	current := &PersistedInstance{Revision: 1, Instance: Instance{
		DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, HeartbeatSequence: 1,
		ServerObservedAtMS: 100_000, CapabilityLeaseExpiresAtMS: 160_000,
		Capabilities: capabilities,
	}}
	result, err := Commit(Device{DeviceID: "device-a", ApprovalState: "approved"}, current, 1, Heartbeat{
		DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, Sequence: 2,
		Capabilities: capabilities,
	}, 110_000, 60_000)
	if err != nil {
		t.Fatalf("empty collections rejected at restore boundary: %v", err)
	}
	if result.Revision != 2 || len(result.Instance.Capabilities.GPUs) != 0 || len(result.Instance.Capabilities.Runtimes) != 0 {
		t.Fatalf("empty collection replacement=%#v", result)
	}
}

func TestCommitLeavesCurrentSnapshotUntouchedOnSuccessAndRejection(t *testing.T) {
	device := Device{DeviceID: "device-a", TenantID: "tenant-1", ApprovalState: "approved"}
	capabilities := validTestCapabilities()
	current := &PersistedInstance{Revision: 1, Instance: Instance{
		DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, HeartbeatSequence: 1,
		ServerObservedAtMS: 100_000, CapabilityLeaseExpiresAtMS: 160_000,
		Capabilities: capabilities,
	}}
	original := *current
	result, err := Commit(device, current, 1, Heartbeat{
		DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, Sequence: 2,
		Capabilities: capabilities,
	}, 110_000, 60_000)
	if err != nil || result.Revision != 2 || result.Instance.HeartbeatSequence != 2 {
		t.Fatalf("success result=%#v err=%v", result, err)
	}
	if !reflect.DeepEqual(*current, original) {
		t.Fatalf("successful commit mutated current: got=%#v want=%#v", *current, original)
	}
	if _, err := Commit(device, current, 0, Heartbeat{
		DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, Sequence: 2,
		Capabilities: capabilities,
	}, 110_000, 60_000); err != ErrRevisionConflict {
		t.Fatalf("stale revision error=%v, want %v", err, ErrRevisionConflict)
	}
	if !reflect.DeepEqual(*current, original) {
		t.Fatalf("rejected commit mutated current: got=%#v want=%#v", *current, original)
	}
}
