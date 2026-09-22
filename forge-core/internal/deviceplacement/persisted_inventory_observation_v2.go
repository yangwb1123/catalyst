package deviceplacement

import (
	"sort"
	"strings"
	"unicode"
	"unicode/utf8"

	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceinventory"
)

// SessionDeviceObservationInventoryV2SchemaVersion is the lossless successor
// to the v1 display envelope. It keeps reservation declarations and every
// self-reported GPU instead of collapsing either value.
const SessionDeviceObservationInventoryV2SchemaVersion = "forge.device-inventory-observation/v2"

const SessionDeviceObservationInventoryV2Notice = "Every owner, instance, state, timestamp, resource, GPU, reservation, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only observation selects no target and grants no execution authority."

type SessionDeviceObservationInventoryV2 struct {
	SchemaVersion              string                        `json:"schema_version"`
	EvaluationMode             string                        `json:"evaluation_mode"`
	EvaluatedAtMS              int64                         `json:"evaluated_at_ms"`
	Owner                      Owner                         `json:"owner_declaration"`
	OwnerDeclarationUnverified bool                          `json:"owner_declaration_unverified"`
	InventoryUnverified        bool                          `json:"inventory_declarations_unverified"`
	Notice                     string                        `json:"notice"`
	Devices                    []SessionPlacementCandidateV2 `json:"devices"`
	ExecutionAuthorized        bool                          `json:"execution_authorized"`
	ReservationCreated         bool                          `json:"reservation_created"`
	DispatchPerformed          bool                          `json:"dispatch_performed"`
}

type SessionPlacementCandidateV2 struct {
	InstanceID        string   `json:"instance_id"`
	Revision          uint64   `json:"revision"`
	Generation        uint64   `json:"generation"`
	HeartbeatSequence uint64   `json:"heartbeat_sequence"`
	Device            DeviceV2 `json:"device"`
}

type DeviceV2 struct {
	DeviceID             string             `json:"device_id"`
	Owner                Owner              `json:"owner"`
	ApprovalState        string             `json:"approval_state"`
	CordonState          string             `json:"cordon_state"`
	ReservationState     string             `json:"reservation_state"`
	Liveness             string             `json:"liveness"`
	SnapshotObservedAtMS int64              `json:"snapshot_observed_at_ms"`
	LeaseExpiresAtMS     int64              `json:"lease_expires_at_ms"`
	OS                   string             `json:"os"`
	Architecture         string             `json:"architecture"`
	AvailableCPUCores    uint32             `json:"available_cpu_cores"`
	AvailableMemoryBytes uint64             `json:"available_memory_bytes"`
	AvailableStorage     uint64             `json:"available_storage_bytes"`
	Runtimes             []string           `json:"runtimes"`
	GPUs                 []GPUDeclarationV2 `json:"gpus"`
	DataResidencyZones   []string           `json:"data_residency_zones"`
	TrustZone            string             `json:"trust_zone"`
	SandboxLevels        []string           `json:"sandbox_levels"`
	ConcurrencyLimit     uint16             `json:"concurrency_limit"`
	ActiveConcurrency    uint16             `json:"active_concurrency"`
}

type GPUDeclarationV2 struct {
	ID                   string `json:"id"`
	Vendor               string `json:"vendor"`
	MemoryBytes          uint64 `json:"memory_bytes"`
	AvailableMemoryBytes uint64 `json:"available_memory_bytes"`
}

// BuildPersistedInventoryObservationV2 converts already restored values into
// a lossless, owner-bound, display-only envelope. It performs no storage,
// clock, transport, reservation, target-selection, or execution operation.
func BuildPersistedInventoryObservationV2(
	values []deviceinventory.PersistedInventoryState,
	evaluationOwner deviceinventory.SnapshotOwner,
	evaluatedAtMS uint64,
) (SessionDeviceObservationInventoryV2, error) {
	owner := Owner{Issuer: evaluationOwner.Issuer, Subject: evaluationOwner.Subject, TenantID: evaluationOwner.TenantID}
	if !validOwner(owner) || evaluatedAtMS == 0 || evaluatedAtMS > uint64(MaxSafeIntegerMS) || len(values) > MaxDevices {
		return SessionDeviceObservationInventoryV2{}, ErrInvalidPersistedInventoryObservation
	}
	devices := make([]SessionPlacementCandidateV2, 0, len(values))
	seenDevices := make(map[string]struct{}, len(values))
	seenInstances := make(map[string]struct{}, len(values))
	for _, value := range values {
		canonical, err := deviceinventory.RestorePersistedInventory(value.Revision, value.Device, value.Runner)
		if err != nil {
			return SessionDeviceObservationInventoryV2{}, err
		}
		if canonical.Device.Owner != evaluationOwner {
			return SessionDeviceObservationInventoryV2{}, deviceinventory.ErrInventoryOwnerMismatch
		}
		if canonical.Runner.ServerObservedAtMS > uint64(MaxSafeIntegerMS) ||
			canonical.Runner.CapabilityLeaseExpiresAtMS > uint64(MaxSafeIntegerMS) ||
			canonical.Revision > uint64(MaxSafeIntegerMS) ||
			canonical.Runner.Generation > uint64(MaxSafeIntegerMS) ||
			canonical.Runner.HeartbeatSequence > uint64(MaxSafeIntegerMS) ||
			canonical.Runner.Capabilities.AvailableMemoryBytes > uint64(MaxSafeIntegerMS) ||
			canonical.Runner.Capabilities.AvailableStorageBytes > uint64(MaxSafeIntegerMS) ||
			len(canonical.Runner.Capabilities.Runtimes) > MaxArrayItems ||
			!safeGPUMemory(canonical.Runner.Capabilities.GPUs) {
			return SessionDeviceObservationInventoryV2{}, ErrInvalidPersistedInventoryObservation
		}
		if _, err := deviceinventory.ProjectPersistedInventory(canonical, evaluationOwner, evaluatedAtMS, deviceinventory.DefaultStaleAfterMS); err != nil {
			return SessionDeviceObservationInventoryV2{}, err
		}
		deviceID := canonical.Device.DeviceID
		instanceID := canonical.Runner.InstanceID
		if _, exists := seenDevices[deviceID]; exists {
			return SessionDeviceObservationInventoryV2{}, ErrDuplicatePersistedInventoryDevice
		}
		if _, exists := seenInstances[instanceID]; exists {
			return SessionDeviceObservationInventoryV2{}, ErrDuplicatePersistedInventoryInstance
		}
		seenDevices[deviceID] = struct{}{}
		seenInstances[instanceID] = struct{}{}
		capabilities := canonical.Runner.Capabilities
		gpus := make([]GPUDeclarationV2, len(capabilities.GPUs))
		for index, gpu := range capabilities.GPUs {
			gpus[index] = GPUDeclarationV2{ID: gpu.ID, Vendor: gpu.Vendor, MemoryBytes: gpu.MemoryBytes, AvailableMemoryBytes: gpu.AvailableMemoryBytes}
		}
		devices = append(devices, SessionPlacementCandidateV2{
			InstanceID: instanceID, Revision: canonical.Revision,
			Generation: canonical.Runner.Generation, HeartbeatSequence: canonical.Runner.HeartbeatSequence,
			Device: DeviceV2{
				DeviceID: deviceID, Owner: owner, ApprovalState: canonical.Device.ApprovalState,
				CordonState: canonical.Device.CordonState, ReservationState: canonical.Device.ReservationState,
				Liveness: canonical.Runner.Liveness, SnapshotObservedAtMS: int64(canonical.Runner.ServerObservedAtMS),
				LeaseExpiresAtMS: int64(canonical.Runner.CapabilityLeaseExpiresAtMS), OS: capabilities.OperatingSystem,
				Architecture: capabilities.Architecture, AvailableCPUCores: capabilities.AvailableCPUCores,
				AvailableMemoryBytes: capabilities.AvailableMemoryBytes, AvailableStorage: capabilities.AvailableStorageBytes,
				Runtimes: append([]string(nil), capabilities.Runtimes...), GPUs: gpus,
				DataResidencyZones: []string{}, TrustZone: "unknown", SandboxLevels: []string{},
				ConcurrencyLimit: 0, ActiveConcurrency: 0,
			},
		})
	}
	sort.Slice(devices, func(left, right int) bool {
		if devices[left].Device.DeviceID == devices[right].Device.DeviceID {
			return devices[left].InstanceID < devices[right].InstanceID
		}
		return devices[left].Device.DeviceID < devices[right].Device.DeviceID
	})
	return SessionDeviceObservationInventoryV2{
		SchemaVersion:  SessionDeviceObservationInventoryV2SchemaVersion,
		EvaluationMode: EvaluationMode, EvaluatedAtMS: int64(evaluatedAtMS), Owner: owner,
		OwnerDeclarationUnverified: true, InventoryUnverified: true,
		Notice: SessionDeviceObservationInventoryV2Notice, Devices: devices,
		ExecutionAuthorized: false, ReservationCreated: false, DispatchPerformed: false,
	}, nil
}

func safeGPUMemory(gpus []deviceheartbeat.GPUCapability) bool {
	availableTotal := uint64(0)
	for _, gpu := range gpus {
		if gpu.MemoryBytes > uint64(MaxSafeIntegerMS) || gpu.AvailableMemoryBytes > uint64(MaxSafeIntegerMS) {
			return false
		}
		var ok bool
		availableTotal, ok = addSafeGPUBytes(availableTotal, gpu.AvailableMemoryBytes)
		if !ok {
			return false
		}
	}
	return true
}

func addSafeGPUBytes(total, available uint64) (uint64, bool) {
	max := uint64(MaxSafeIntegerMS)
	if available > max || total > max-available {
		return 0, false
	}
	return total + available, true
}

// ValidateSessionDeviceObservationInventoryV2 validates the lossless envelope
// without authenticating any declaration or granting any authority.
func ValidateSessionDeviceObservationInventoryV2(value SessionDeviceObservationInventoryV2) error {
	if value.SchemaVersion != SessionDeviceObservationInventoryV2SchemaVersion || value.EvaluationMode != EvaluationMode ||
		value.EvaluatedAtMS <= 0 || value.EvaluatedAtMS > MaxSafeIntegerMS || !validOwner(value.Owner) ||
		!value.OwnerDeclarationUnverified || !value.InventoryUnverified || value.Notice != SessionDeviceObservationInventoryV2Notice ||
		value.ExecutionAuthorized || value.ReservationCreated || value.DispatchPerformed || len(value.Devices) > MaxDevices {
		return errInvalidRequest
	}
	seenDevices := make(map[string]struct{}, len(value.Devices))
	seenInstances := make(map[string]struct{}, len(value.Devices))
	for index, candidate := range value.Devices {
		device := candidate.Device
		if !validSessionIdentifier(candidate.InstanceID) || candidate.Revision == 0 || candidate.Revision > uint64(MaxSafeIntegerMS) ||
			candidate.Generation == 0 || candidate.Generation > uint64(MaxSafeIntegerMS) ||
			candidate.HeartbeatSequence == 0 || candidate.HeartbeatSequence > uint64(MaxSafeIntegerMS) ||
			!validOwner(device.Owner) || device.Owner != value.Owner ||
			!validDeviceID(device.DeviceID) || device.ApprovalState == "unknown" || !validApproval(device.ApprovalState) ||
			device.CordonState == "unknown" || !validCordonState(device.CordonState) ||
			(device.ReservationState != "none" && device.ReservationState != "reserved") || device.Liveness == "unknown" || !validLiveness(device.Liveness) ||
			device.SnapshotObservedAtMS < 0 || device.SnapshotObservedAtMS > MaxSafeIntegerMS ||
			!validV2CapabilityLease(device.SnapshotObservedAtMS, device.LeaseExpiresAtMS) ||
			!validV2CanonicalTag(device.OS) || !validV2CanonicalTag(device.Architecture) || device.AvailableCPUCores > deviceheartbeat.MaxDeviceCPUCores ||
			device.AvailableMemoryBytes > uint64(MaxSafeIntegerMS) ||
			device.AvailableStorage > uint64(MaxSafeIntegerMS) || !validUniqueTokens(device.Runtimes, MaxArrayItems, validV2CanonicalTag) ||
			!validUniqueTokens(device.DataResidencyZones, MaxArrayItems, validZone) || device.TrustZone != "unknown" ||
			!validUniqueTokens(device.SandboxLevels, MaxArrayItems, validSandboxLevel) || len(device.DataResidencyZones) != 0 ||
			len(device.SandboxLevels) != 0 || device.ConcurrencyLimit != 0 || device.ActiveConcurrency != 0 {
			return errInvalidRequest
		}
		if index > 0 && value.Devices[index-1].Device.DeviceID >= device.DeviceID {
			return errInvalidRequest
		}
		if _, exists := seenDevices[device.DeviceID]; exists {
			return errInvalidRequest
		}
		if _, exists := seenInstances[candidate.InstanceID]; exists {
			return errInvalidRequest
		}
		seenDevices[device.DeviceID] = struct{}{}
		seenInstances[candidate.InstanceID] = struct{}{}
		if len(device.GPUs) > deviceheartbeat.MaxDeviceGPUCount {
			return errInvalidRequest
		}
		seenGPUs := make(map[string]struct{}, len(device.GPUs))
		availableGPUMemoryBytes := uint64(0)
		for gpuIndex, gpu := range device.GPUs {
			if !validDeviceID(gpu.ID) || !validGPUVendor(gpu.Vendor) ||
				gpu.MemoryBytes == 0 || gpu.MemoryBytes > uint64(MaxSafeIntegerMS) ||
				gpu.MemoryBytes > uint64(deviceheartbeat.MaxDeviceCapabilityBytes) ||
				gpu.AvailableMemoryBytes > gpu.MemoryBytes || gpu.AvailableMemoryBytes > uint64(MaxSafeIntegerMS) ||
				(gpuIndex > 0 && device.GPUs[gpuIndex-1].ID >= gpu.ID) {
				return errInvalidRequest
			}
			var ok bool
			availableGPUMemoryBytes, ok = addSafeGPUBytes(availableGPUMemoryBytes, gpu.AvailableMemoryBytes)
			if !ok {
				return errInvalidRequest
			}
			if _, exists := seenGPUs[gpu.ID]; exists {
				return errInvalidRequest
			}
			seenGPUs[gpu.ID] = struct{}{}
		}
	}
	return nil
}

// validV2CapabilityLease keeps caller-supplied observations aligned with the
// persisted heartbeat boundary used by Go, Rust, and Flutter. A zero-length
// or sub-minimum lease is not a valid capability observation, even when its
// expiry is otherwise ordered and still representable by the wire format.
func validV2CapabilityLease(observedAtMS, expiresAtMS int64) bool {
	if observedAtMS < 0 || expiresAtMS < observedAtMS || expiresAtMS > MaxSafeIntegerMS {
		return false
	}
	ttl := expiresAtMS - observedAtMS
	return ttl >= int64(deviceheartbeat.MinLeaseTTLMS) && ttl <= int64(deviceheartbeat.MaxLeaseTTLMS)
}

func validLowerToken(value string) bool {
	return validToken(value) && value == strings.ToLower(value)
}

// validV2CanonicalTag mirrors the heartbeat capability tag grammar used by
// Rust and Flutter. The legacy placement contract accepts URL-like token
// punctuation and a wider length; v2 observations carry canonical heartbeat
// tags, which are lowercase ASCII and at most 64 bytes.
func validV2CanonicalTag(value string) bool {
	if value == "" || len(value) > deviceheartbeat.MaxDeviceRuntimeNameBytes {
		return false
	}
	for _, character := range value {
		if character >= 'A' && character <= 'Z' {
			return false
		}
		if character >= 'a' && character <= 'z' || character >= '0' && character <= '9' ||
			strings.ContainsRune("._-+", character) {
			continue
		}
		return false
	}
	return true
}

func validGPUVendor(value string) bool {
	return utf8.ValidString(value) && value != "" && len(value) <= deviceheartbeat.MaxDeviceRuntimeNameBytes &&
		strings.TrimSpace(value) == value && !strings.ContainsFunc(value, unicode.IsControl)
}
