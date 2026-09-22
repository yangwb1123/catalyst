package deviceheartbeat

import (
	"reflect"
	"testing"
)

func TestApplyPreservesBoundaryErrorPrecedenceBeforeCapabilityValidation(t *testing.T) {
	capabilities := validTestCapabilities()
	capabilities.CPUCores = 0
	heartbeat := Heartbeat{
		DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, Sequence: 1,
		Capabilities: capabilities,
	}
	tests := []struct {
		name   string
		device Device
		lease  uint64
		want   error
	}{
		{
			name:   "device mismatch",
			device: Device{DeviceID: "device-foreign", ApprovalState: "approved"},
			lease:  60_000,
			want:   ErrDeviceMismatch,
		},
		{
			name:   "revoked device",
			device: Device{DeviceID: "device-a", ApprovalState: "revoked"},
			lease:  60_000,
			want:   ErrDeviceRevoked,
		},
		{
			name:   "invalid lease",
			device: Device{DeviceID: "device-a", ApprovalState: "approved"},
			lease:  MinLeaseTTLMS - 1,
			want:   ErrInvalidLeaseDuration,
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if _, err := Apply(test.device, nil, heartbeat, 100_000, test.lease); err != test.want {
				t.Fatalf("error=%v, want %v", err, test.want)
			}
		})
	}
}

func TestApplyRejectsInvalidIdentityAndApprovalBoundariesBeforeCapabilityValidation(t *testing.T) {
	capabilities := validTestCapabilities()
	capabilities.CPUCores = 0
	tests := []struct {
		name      string
		device    Device
		heartbeat Heartbeat
		want      error
	}{
		{
			name:      "empty instance id",
			device:    Device{DeviceID: "device-a", ApprovalState: "approved"},
			heartbeat: Heartbeat{DeviceID: "device-a", Generation: 1, Sequence: 1, Capabilities: capabilities},
			want:      ErrInvalidInstanceID,
		},
		{
			name:   "invalid instance id",
			device: Device{DeviceID: "device-a", ApprovalState: "approved"},
			heartbeat: Heartbeat{
				DeviceID: "device-a", InstanceID: "runner/a", Generation: 1, Sequence: 1,
				Capabilities: capabilities,
			},
			want: ErrInvalidInstanceID,
		},
		{
			name:      "invalid device id",
			device:    Device{DeviceID: "", ApprovalState: "approved"},
			heartbeat: Heartbeat{DeviceID: "", InstanceID: "runner-a", Generation: 1, Sequence: 1, Capabilities: capabilities},
			want:      ErrInvalidDeviceID,
		},
		{
			name:   "unknown approval state",
			device: Device{DeviceID: "device-a", ApprovalState: "quarantined"},
			heartbeat: Heartbeat{
				DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, Sequence: 1,
				Capabilities: capabilities,
			},
			want: ErrUnknownApproval,
		},
		{
			name:   "zero generation with prior observation",
			device: Device{DeviceID: "device-a", ApprovalState: "approved"},
			heartbeat: Heartbeat{
				DeviceID: "device-a", InstanceID: "runner-a", Generation: 0, Sequence: 2,
				Capabilities: capabilities,
			},
			want: ErrGenerationMustStartAtOne,
		},
		{
			name:   "zero sequence with prior observation",
			device: Device{DeviceID: "device-a", ApprovalState: "approved"},
			heartbeat: Heartbeat{
				DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, Sequence: 0,
				Capabilities: capabilities,
			},
			want: ErrSequenceMustStartAtOne,
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			current := &Instance{
				DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, HeartbeatSequence: 1,
				ServerObservedAtMS: 100_000, CapabilityLeaseExpiresAtMS: 160_000,
				Capabilities: validTestCapabilities(),
			}
			if _, err := Apply(test.device, current, test.heartbeat, 110_000, 60_000); err != test.want {
				t.Fatalf("error=%v, want %v", err, test.want)
			}
		})
	}
}

func TestApplyAcceptsKnownPendingApprovalAsPureHeartbeatInput(t *testing.T) {
	for _, approval := range []string{"pending", "approved"} {
		t.Run(approval, func(t *testing.T) {
			instance, err := Apply(Device{DeviceID: "device-a", ApprovalState: approval}, nil, Heartbeat{
				DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, Sequence: 1,
				Capabilities: validTestCapabilities(),
			}, 100_000, 60_000)
			if err != nil {
				t.Fatalf("known approval %q rejected: %v", approval, err)
			}
			if instance.DeviceID != "device-a" || instance.InstanceID != "runner-a" {
				t.Fatalf("accepted instance=%#v", instance)
			}
		})
	}
}

func TestCommitInheritsHeartbeatBoundaryValidationWithoutReplacingCurrent(t *testing.T) {
	current := &PersistedInstance{Revision: 1, Instance: Instance{
		DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, HeartbeatSequence: 1,
		ServerObservedAtMS: 100_000, CapabilityLeaseExpiresAtMS: 160_000,
		Capabilities: validTestCapabilities(),
	}}
	original := *current
	result, err := Commit(Device{DeviceID: "device-a", ApprovalState: "approved"}, current, 1, Heartbeat{
		DeviceID: "device-a", InstanceID: "", Generation: 1, Sequence: 2,
		Capabilities: validTestCapabilities(),
	}, 110_000, 60_000)
	if err != ErrInvalidInstanceID || result.Revision != 0 || result.Instance.DeviceID != "" {
		t.Fatalf("result=%#v err=%v, want zero result and %v", result, err, ErrInvalidInstanceID)
	}
	if !reflect.DeepEqual(*current, original) {
		t.Fatalf("boundary rejection replaced current: got=%#v want=%#v", *current, original)
	}

	result, err = Commit(Device{DeviceID: "device-a", ApprovalState: "unknown"}, current, 1, Heartbeat{
		DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, Sequence: 2,
		Capabilities: validTestCapabilities(),
	}, 110_000, 60_000)
	if err != ErrUnknownApproval || result.Revision != 0 || result.Instance.DeviceID != "" {
		t.Fatalf("unknown approval result=%#v err=%v, want zero result and %v", result, err, ErrUnknownApproval)
	}
	if !reflect.DeepEqual(*current, original) {
		t.Fatalf("unknown approval rejection replaced current: got=%#v want=%#v", *current, original)
	}
}
