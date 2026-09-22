package deviceplacement

import (
	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceinventory"
)

// BuildPersistedInventoryClientInstanceResourceView joins already restored,
// owner-bound inventory rows with caller-declared client instances. The
// result is a deterministic display observation; this function performs no
// storage, clock, discovery, reservation, scheduling, dispatch, or Runner
// operation.
func BuildPersistedInventoryClientInstanceResourceView(
	values []deviceinventory.PersistedInventoryState,
	evaluationOwner deviceinventory.SnapshotOwner,
	instances []ClientInstanceSessionViewInstance,
) (ClientInstanceResourceViewObservation, error) {
	owner := ownerFromSnapshot(evaluationOwner)
	if !validOwner(owner) || len(values) > MaxDevices {
		return ClientInstanceResourceViewObservation{}, ErrInvalidPersistedInventoryObservation
	}

	devices := make([]ClientInstanceResourceViewDevice, 0, len(values))
	seenDevices := make(map[string]struct{}, len(values))
	seenRunners := make(map[string]struct{}, len(values))
	for _, value := range values {
		canonical, err := deviceinventory.RestorePersistedInventory(value.Revision, value.Device, value.Runner)
		if err != nil {
			return ClientInstanceResourceViewObservation{}, err
		}
		if canonical.Device.Owner != evaluationOwner {
			return ClientInstanceResourceViewObservation{}, deviceinventory.ErrInventoryOwnerMismatch
		}
		if canonical.Revision > uint64(MaxSafeIntegerMS) ||
			canonical.Runner.Generation > uint64(MaxSafeIntegerMS) ||
			canonical.Runner.HeartbeatSequence > uint64(MaxSafeIntegerMS) ||
			canonical.Runner.ServerObservedAtMS == 0 ||
			canonical.Runner.ServerObservedAtMS > uint64(MaxSafeIntegerMS) ||
			canonical.Runner.CapabilityLeaseExpiresAtMS > uint64(MaxSafeIntegerMS) ||
			canonical.Runner.Capabilities.MemoryBytes > uint64(MaxSafeIntegerMS) ||
			canonical.Runner.Capabilities.AvailableMemoryBytes > uint64(MaxSafeIntegerMS) ||
			canonical.Runner.Capabilities.StorageBytes > uint64(MaxSafeIntegerMS) ||
			canonical.Runner.Capabilities.AvailableStorageBytes > uint64(MaxSafeIntegerMS) {
			return ClientInstanceResourceViewObservation{}, ErrInvalidPersistedInventoryObservation
		}

		availableGPUMemory, ok := persistedInventoryClientResourceGPUMemory(canonical.Runner.Capabilities.GPUs)
		if !ok {
			return ClientInstanceResourceViewObservation{}, ErrInvalidPersistedInventoryObservation
		}
		deviceID := canonical.Device.DeviceID
		runnerID := canonical.Runner.InstanceID
		if _, exists := seenDevices[deviceID]; exists {
			return ClientInstanceResourceViewObservation{}, ErrDuplicatePersistedInventoryDevice
		}
		if _, exists := seenRunners[runnerID]; exists {
			return ClientInstanceResourceViewObservation{}, ErrDuplicatePersistedInventoryInstance
		}
		seenDevices[deviceID] = struct{}{}
		seenRunners[runnerID] = struct{}{}

		capabilities := canonical.Runner.Capabilities
		devices = append(devices, ClientInstanceResourceViewDevice{
			DeviceID:                deviceID,
			RunnerInstanceID:        runnerID,
			Owner:                   owner,
			Revision:                canonical.Revision,
			Generation:              canonical.Runner.Generation,
			HeartbeatSequence:       canonical.Runner.HeartbeatSequence,
			ObservedAtMS:            int64(canonical.Runner.ServerObservedAtMS),
			ApprovalState:           canonical.Device.ApprovalState,
			CordonState:             canonical.Device.CordonState,
			ReservationState:        canonical.Device.ReservationState,
			Liveness:                canonical.Runner.Liveness,
			OS:                      capabilities.OperatingSystem,
			Architecture:            capabilities.Architecture,
			CPUCores:                capabilities.CPUCores,
			AvailableCPUCores:       capabilities.AvailableCPUCores,
			MemoryBytes:             capabilities.MemoryBytes,
			AvailableMemoryBytes:    capabilities.AvailableMemoryBytes,
			StorageBytes:            capabilities.StorageBytes,
			AvailableStorageBytes:   capabilities.AvailableStorageBytes,
			GPUCount:                uint32(len(capabilities.GPUs)),
			AvailableGPUMemoryBytes: availableGPUMemory,
		})
	}

	return ObserveClientInstanceResourceView(ClientInstanceResourceViewRequest{
		Owner: owner, Instances: instances, Devices: devices,
	})
}

func persistedInventoryClientResourceGPUMemory(gpus []deviceheartbeat.GPUCapability) (uint64, bool) {
	availableTotal := uint64(0)
	max := uint64(MaxSafeIntegerMS)
	for _, gpu := range gpus {
		if gpu.MemoryBytes > max || gpu.AvailableMemoryBytes > max {
			return 0, false
		}
		var ok bool
		availableTotal, ok = addSafeGPUBytes(availableTotal, gpu.AvailableMemoryBytes)
		if !ok {
			return 0, false
		}
	}
	return availableTotal, true
}
