// Package deviceheartbeat contains the effect-free Runner heartbeat reference
// model. It does not authenticate a device, persist a row, read a clock, or
// open a listener. Forge Core may use these rules only after a separately
// accepted enrollment decision.
package deviceheartbeat

const (
	MinLeaseTTLMS uint64 = 1_000
	MaxLeaseTTLMS uint64 = 600_000
)

// Device is the already-bound identity and approval state supplied to the
// pure transition. The model never derives it from a caller-provided owner.
type Device struct {
	DeviceID      string
	TenantID      string
	ApprovalState string
}

// Heartbeat is a device-declared Runner incarnation and sequence.
type Heartbeat struct {
	DeviceID     string             `json:"device_id"`
	InstanceID   string             `json:"instance_id"`
	Generation   uint64             `json:"generation"`
	Sequence     uint64             `json:"sequence"`
	Capabilities CapabilitySnapshot `json:"capabilities"`
}

// Instance is the server-observed state produced by an accepted heartbeat.
type Instance struct {
	DeviceID                   string             `json:"device_id"`
	InstanceID                 string             `json:"instance_id"`
	Generation                 uint64             `json:"generation"`
	HeartbeatSequence          uint64             `json:"heartbeat_sequence"`
	ServerObservedAtMS         uint64             `json:"server_observed_at_ms"`
	CapabilityLeaseExpiresAtMS uint64             `json:"capability_lease_expires_at_ms"`
	Capabilities               CapabilitySnapshot `json:"capabilities"`
}

// ErrorCode is a stable, authority-neutral rejection reason for this model.
type ErrorCode string

const (
	ErrDeviceMismatch                  ErrorCode = "device_mismatch"
	ErrDeviceRevoked                   ErrorCode = "device_revoked"
	ErrUnknownApproval                 ErrorCode = "unknown_approval_state"
	ErrInvalidDeviceID                 ErrorCode = "invalid_device_id"
	ErrInvalidInstanceID               ErrorCode = "invalid_instance_id"
	ErrGenerationMustStartAtOne        ErrorCode = "generation_must_start_at_one"
	ErrOldGeneration                   ErrorCode = "old_generation"
	ErrGenerationSkipped               ErrorCode = "generation_skipped"
	ErrInstanceChangedWithinGeneration ErrorCode = "instance_changed_within_generation"
	ErrSequenceMustStartAtOne          ErrorCode = "sequence_must_start_at_one"
	ErrSequenceNotIncreasing           ErrorCode = "sequence_not_increasing"
	ErrServerTimeWentBackwards         ErrorCode = "server_time_went_backwards"
	ErrInvalidLeaseDuration            ErrorCode = "invalid_lease_duration"
	ErrLeaseExpiryOverflow             ErrorCode = "lease_expiry_overflow"
)

func (e ErrorCode) Error() string { return string(e) }

// ErrUnknownApprovalState is the descriptive alias used by callers that
// prefer the full state-oriented name. Both names carry the same stable wire
// value so a transition cannot accidentally grow two rejection contracts.
const ErrUnknownApprovalState ErrorCode = ErrUnknownApproval

// Apply validates one heartbeat against an optional prior observation. The
// supplied serverObservedAtMS and leaseTTLMS are explicit inputs so this
// transition cannot acquire authority from a local clock or storage layer.
func Apply(device Device, current *Instance, heartbeat Heartbeat, serverObservedAtMS, leaseTTLMS uint64) (Instance, error) {
	if heartbeat.DeviceID != device.DeviceID {
		return Instance{}, ErrDeviceMismatch
	}
	if !validHeartbeatIdentifier(device.DeviceID) {
		return Instance{}, ErrInvalidDeviceID
	}
	switch device.ApprovalState {
	case "approved", "pending":
	case "revoked":
		return Instance{}, ErrDeviceRevoked
	default:
		return Instance{}, ErrUnknownApproval
	}
	if !validHeartbeatIdentifier(heartbeat.InstanceID) {
		return Instance{}, ErrInvalidInstanceID
	}
	if heartbeat.Generation == 0 {
		return Instance{}, ErrGenerationMustStartAtOne
	}
	if heartbeat.Sequence == 0 {
		return Instance{}, ErrSequenceMustStartAtOne
	}
	if leaseTTLMS < MinLeaseTTLMS || leaseTTLMS > MaxLeaseTTLMS {
		return Instance{}, ErrInvalidLeaseDuration
	}
	capabilities, err := heartbeat.Capabilities.canonicalize()
	if err != nil {
		return Instance{}, err
	}
	if current != nil {
		if err := current.Capabilities.Validate(); err != nil {
			return Instance{}, ErrInvalidCapabilityValue
		}
	}
	if err := validateIncarnation(current, heartbeat, device, serverObservedAtMS); err != nil {
		return Instance{}, err
	}
	expiresAtMS := serverObservedAtMS + leaseTTLMS
	if expiresAtMS < serverObservedAtMS {
		return Instance{}, ErrLeaseExpiryOverflow
	}
	return Instance{
		DeviceID:                   heartbeat.DeviceID,
		InstanceID:                 heartbeat.InstanceID,
		Generation:                 heartbeat.Generation,
		HeartbeatSequence:          heartbeat.Sequence,
		ServerObservedAtMS:         serverObservedAtMS,
		CapabilityLeaseExpiresAtMS: expiresAtMS,
		Capabilities:               capabilities,
	}, nil
}

func validHeartbeatIdentifier(value string) bool {
	_, err := normalizeIdentifier(value)
	return err == nil
}

func validateIncarnation(current *Instance, heartbeat Heartbeat, device Device, serverNowMS uint64) error {
	if current == nil {
		if heartbeat.Generation != 1 {
			return ErrGenerationMustStartAtOne
		}
		if heartbeat.Sequence != 1 {
			return ErrSequenceMustStartAtOne
		}
		return nil
	}
	if current.DeviceID != device.DeviceID {
		return ErrDeviceMismatch
	}
	if serverNowMS < current.ServerObservedAtMS {
		return ErrServerTimeWentBackwards
	}
	if heartbeat.Generation < current.Generation {
		return ErrOldGeneration
	}
	if heartbeat.Generation == current.Generation {
		if heartbeat.InstanceID != current.InstanceID {
			return ErrInstanceChangedWithinGeneration
		}
		if heartbeat.Sequence <= current.HeartbeatSequence {
			return ErrSequenceNotIncreasing
		}
		return nil
	}
	if current.Generation == ^uint64(0) || heartbeat.Generation != current.Generation+1 {
		return ErrGenerationSkipped
	}
	if heartbeat.Sequence != 1 {
		return ErrSequenceMustStartAtOne
	}
	return nil
}
