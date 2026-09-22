package deviceplacement

import (
	"sort"

	"forgeos/forge-core/internal/deviceinventory"
)

// PersistedInventoryObservationError identifies a value-level conversion
// failure. The conversion has no storage, clock, transport, or execution
// authority; callers must treat the returned inventory as an unverified
// observation even when conversion succeeds.
type PersistedInventoryObservationError string

const (
	ErrInvalidPersistedInventoryObservation     PersistedInventoryObservationError = "invalid_persisted_inventory_observation"
	ErrUnsupportedPersistedInventoryObservation PersistedInventoryObservationError = "unsupported_persisted_inventory_observation"
	ErrDuplicatePersistedInventoryDevice        PersistedInventoryObservationError = "duplicate_persisted_inventory_device"
	ErrDuplicatePersistedInventoryInstance      PersistedInventoryObservationError = "duplicate_persisted_inventory_instance"
)

func (e PersistedInventoryObservationError) Error() string { return string(e) }

// BuildPersistedInventoryObservation converts already restored inventory
// values into the owner-bound, display-only inventory envelope used by the
// private read candidate. It evaluates every value at the explicit supplied
// time and never reads a clock or writes storage.
//
// The current observation envelope cannot represent multiple GPUs or a
// reservation declaration without losing information. Such values are
// rejected until a lossless versioned envelope exists; silently dropping
// either declaration would make a later client decision unsafe.
func BuildPersistedInventoryObservation(
	values []deviceinventory.PersistedInventoryState,
	evaluationOwner deviceinventory.SnapshotOwner,
	evaluatedAtMS uint64,
) (SessionDeviceObservationInventory, error) {
	owner := Owner{
		Issuer:   evaluationOwner.Issuer,
		Subject:  evaluationOwner.Subject,
		TenantID: evaluationOwner.TenantID,
	}
	if !validOwner(owner) || evaluatedAtMS == 0 || evaluatedAtMS > uint64(MaxSafeIntegerMS) || len(values) > MaxDevices {
		return SessionDeviceObservationInventory{}, ErrInvalidPersistedInventoryObservation
	}

	devices := make([]SessionPlacementCandidate, 0, len(values))
	seenDevices := make(map[string]struct{}, len(values))
	seenInstances := make(map[string]struct{}, len(values))
	for _, value := range values {
		canonical, err := deviceinventory.RestorePersistedInventory(value.Revision, value.Device, value.Runner)
		if err != nil {
			return SessionDeviceObservationInventory{}, err
		}
		if canonical.Device.Owner != evaluationOwner {
			return SessionDeviceObservationInventory{}, deviceinventory.ErrInventoryOwnerMismatch
		}
		if canonical.Device.ReservationState != "none" || len(canonical.Runner.Capabilities.GPUs) != 0 {
			return SessionDeviceObservationInventory{}, ErrUnsupportedPersistedInventoryObservation
		}
		if canonical.Runner.ServerObservedAtMS > uint64(MaxSafeIntegerMS) ||
			canonical.Runner.CapabilityLeaseExpiresAtMS > uint64(MaxSafeIntegerMS) ||
			canonical.Runner.Capabilities.AvailableMemoryBytes > uint64(MaxSafeIntegerMS) ||
			canonical.Runner.Capabilities.AvailableStorageBytes > uint64(MaxSafeIntegerMS) {
			return SessionDeviceObservationInventory{}, ErrInvalidPersistedInventoryObservation
		}
		if _, err := deviceinventory.ProjectPersistedInventory(
			canonical,
			evaluationOwner,
			evaluatedAtMS,
			deviceinventory.DefaultStaleAfterMS,
		); err != nil {
			return SessionDeviceObservationInventory{}, err
		}

		deviceID := canonical.Device.DeviceID
		instanceID := canonical.Runner.InstanceID
		if _, exists := seenDevices[deviceID]; exists {
			return SessionDeviceObservationInventory{}, ErrDuplicatePersistedInventoryDevice
		}
		if _, exists := seenInstances[instanceID]; exists {
			return SessionDeviceObservationInventory{}, ErrDuplicatePersistedInventoryInstance
		}
		seenDevices[deviceID] = struct{}{}
		seenInstances[instanceID] = struct{}{}

		capabilities := canonical.Runner.Capabilities
		devices = append(devices, SessionPlacementCandidate{
			InstanceID: instanceID,
			Device: Device{
				DeviceID:             deviceID,
				Owner:                owner,
				ApprovalState:        canonical.Device.ApprovalState,
				CordonState:          canonical.Device.CordonState,
				Liveness:             canonical.Runner.Liveness,
				SnapshotObservedAtMS: int64(canonical.Runner.ServerObservedAtMS),
				LeaseExpiresAtMS:     int64(canonical.Runner.CapabilityLeaseExpiresAtMS),
				OS:                   capabilities.OperatingSystem,
				Architecture:         capabilities.Architecture,
				AvailableCPUCores:    capabilities.AvailableCPUCores,
				AvailableMemoryBytes: capabilities.AvailableMemoryBytes,
				AvailableStorage:     capabilities.AvailableStorageBytes,
				Runtimes:             append([]string(nil), capabilities.Runtimes...),
				GPU:                  GPUDeclaration{},
				DataResidencyZones:   []string{},
				TrustZone:            "unknown",
				SandboxLevels:        []string{},
				ConcurrencyLimit:     0,
				ActiveConcurrency:    0,
			},
		})
	}
	sort.Slice(devices, func(left, right int) bool {
		return devices[left].Device.DeviceID < devices[right].Device.DeviceID
	})
	return SessionDeviceObservationInventory{
		SchemaVersion:              "forge.device-inventory-observation/v1",
		EvaluationMode:             EvaluationMode,
		EvaluatedAtMS:              int64(evaluatedAtMS),
		Owner:                      owner,
		OwnerDeclarationUnverified: true,
		InventoryUnverified:        true,
		Notice:                     SessionDeviceObservationInventoryNotice,
		Devices:                    devices,
		ExecutionAuthorized:        false,
		ReservationCreated:         false,
		DispatchPerformed:          false,
	}, nil
}
