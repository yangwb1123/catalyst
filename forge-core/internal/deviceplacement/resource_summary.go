package deviceplacement

import "sort"

// DeviceResourceSummarySchemaVersion identifies a pure, read-only aggregate
// of caller-supplied inventory and session-placement declarations. It does
// not establish inventory authority or select a target.
const DeviceResourceSummarySchemaVersion = "forge.device-resource-summary/v1"

const deviceResourceSummaryNotice = "Every owner, instance, resource, placement, and eligibility value is an unverified caller declaration. This read-only summary aggregates declarations, selects no target, and grants no execution authority."

// DeviceResourceSummaryRequest combines one inventory declaration set with its
// already observed session-placement declaration. Both inputs are values; the
// observer has no clock, storage, transport, or execution capability.
type DeviceResourceSummaryRequest struct {
	Owner     Owner
	Inventory []SessionPlacementCandidate
	Placement SessionPlacementObservation
}

// DeviceResourceSummary is a deterministic aggregate across declared devices
// and Runner instances. Resource totals are sums of available declarations;
// eligible counts come from placement decisions and are not target selection.
type DeviceResourceSummary struct {
	SchemaVersion                   string                    `json:"schema_version"`
	EvaluationMode                  string                    `json:"evaluation_mode"`
	Owner                           Owner                     `json:"owner"`
	ConversationID                  string                    `json:"conversation_id"`
	RunID                           string                    `json:"run_id"`
	EvaluatedAtMS                   int64                     `json:"evaluated_at_ms"`
	OwnerDeclarationUnverified      bool                      `json:"owner_declaration_unverified"`
	InventoryDeclarationsUnverified bool                      `json:"inventory_declarations_unverified"`
	PlacementDeclarationUnverified  bool                      `json:"placement_declaration_unverified"`
	Notice                          string                    `json:"notice"`
	DeviceCount                     int                       `json:"device_count"`
	RunnerInstanceCount             int                       `json:"runner_instance_count"`
	AvailableCPUCores               uint64                    `json:"available_cpu_cores"`
	AvailableMemoryBytes            uint64                    `json:"available_memory_bytes"`
	AvailableStorageBytes           uint64                    `json:"available_storage_bytes"`
	AvailableGPUCount               int                       `json:"available_gpu_count"`
	AvailableGPUMemoryBytes         uint64                    `json:"available_gpu_memory_bytes"`
	EligibleDeviceCount             int                       `json:"eligible_device_count"`
	EligibleInstanceCount           int                       `json:"eligible_instance_count"`
	SelectedDeviceID                *string                   `json:"selected_device_id"`
	SelectedInstanceID              *string                   `json:"selected_instance_id"`
	Authority                       SessionPlacementAuthority `json:"authority"`
}

// ObserveDeviceResourceSummary aggregates one complete inventory declaration
// set with the matching session-placement observation. It sorts declarations
// before folding them and never selects, reserves, authorizes, or dispatches.
func ObserveDeviceResourceSummary(input DeviceResourceSummaryRequest) (DeviceResourceSummary, error) {
	if !validOwner(input.Owner) || len(input.Inventory) > MaxDevices ||
		!validSummaryPlacement(input.Placement, input.Owner) ||
		input.Placement.EvaluatedAtMS > MaxSafeIntegerMS {
		return DeviceResourceSummary{}, errInvalidRequest
	}

	byDevice := make(map[string]SessionPlacementCandidate, len(input.Inventory))
	byInstance := make(map[string]struct{}, len(input.Inventory))
	for _, candidate := range input.Inventory {
		if !validSessionIdentifier(candidate.InstanceID) || !validDeviceDeclaration(candidate.Device) ||
			candidate.Device.Owner != input.Owner {
			return DeviceResourceSummary{}, errInvalidRequest
		}
		if _, exists := byDevice[candidate.Device.DeviceID]; exists {
			return DeviceResourceSummary{}, errInvalidRequest
		}
		if _, exists := byInstance[candidate.InstanceID]; exists {
			return DeviceResourceSummary{}, errInvalidRequest
		}
		byDevice[candidate.Device.DeviceID] = candidate
		byInstance[candidate.InstanceID] = struct{}{}
	}
	if len(input.Placement.Decisions) != len(input.Inventory) {
		return DeviceResourceSummary{}, errInvalidRequest
	}

	decisions := append([]SessionPlacementDecision(nil), input.Placement.Decisions...)
	sort.Slice(decisions, func(left, right int) bool {
		if decisions[left].DeviceID != decisions[right].DeviceID {
			return decisions[left].DeviceID < decisions[right].DeviceID
		}
		return decisions[left].InstanceID < decisions[right].InstanceID
	})
	seenDevices := make(map[string]struct{}, len(decisions))
	seenInstances := make(map[string]struct{}, len(decisions))
	eligibleDevices := 0
	eligibleInstances := 0
	for _, decision := range decisions {
		if !validSessionIdentifier(decision.DeviceID) || !validSessionIdentifier(decision.InstanceID) {
			return DeviceResourceSummary{}, errInvalidRequest
		}
		if _, exists := seenDevices[decision.DeviceID]; exists {
			return DeviceResourceSummary{}, errInvalidRequest
		}
		if _, exists := seenInstances[decision.InstanceID]; exists {
			return DeviceResourceSummary{}, errInvalidRequest
		}
		candidate, exists := byDevice[decision.DeviceID]
		if !exists || candidate.InstanceID != decision.InstanceID {
			return DeviceResourceSummary{}, errInvalidRequest
		}
		seenDevices[decision.DeviceID] = struct{}{}
		seenInstances[decision.InstanceID] = struct{}{}
		if decision.MatchesRequirements {
			eligibleDevices++
			eligibleInstances++
		}
	}

	availableCPU := uint64(0)
	availableMemory := uint64(0)
	availableStorage := uint64(0)
	availableGPUCount := 0
	availableGPUMemory := uint64(0)
	for _, candidate := range input.Inventory {
		device := candidate.Device
		var ok bool
		if availableCPU, ok = addUint64(availableCPU, uint64(device.AvailableCPUCores)); !ok {
			return DeviceResourceSummary{}, errInvalidRequest
		}
		if availableMemory, ok = addUint64(availableMemory, device.AvailableMemoryBytes); !ok {
			return DeviceResourceSummary{}, errInvalidRequest
		}
		if availableStorage, ok = addUint64(availableStorage, device.AvailableStorage); !ok {
			return DeviceResourceSummary{}, errInvalidRequest
		}
		if device.GPU.Present {
			availableGPUCount++
			if availableGPUMemory, ok = addUint64(availableGPUMemory, device.GPU.MemoryBytes); !ok {
				return DeviceResourceSummary{}, errInvalidRequest
			}
		}
	}
	count := len(input.Inventory)
	return DeviceResourceSummary{
		SchemaVersion:                   DeviceResourceSummarySchemaVersion,
		EvaluationMode:                  EvaluationMode,
		Owner:                           input.Owner,
		ConversationID:                  input.Placement.ConversationID,
		RunID:                           input.Placement.RunID,
		EvaluatedAtMS:                   input.Placement.EvaluatedAtMS,
		OwnerDeclarationUnverified:      true,
		InventoryDeclarationsUnverified: true,
		PlacementDeclarationUnverified:  true,
		Notice:                          deviceResourceSummaryNotice,
		DeviceCount:                     count,
		RunnerInstanceCount:             count,
		AvailableCPUCores:               availableCPU,
		AvailableMemoryBytes:            availableMemory,
		AvailableStorageBytes:           availableStorage,
		AvailableGPUCount:               availableGPUCount,
		AvailableGPUMemoryBytes:         availableGPUMemory,
		EligibleDeviceCount:             eligibleDevices,
		EligibleInstanceCount:           eligibleInstances,
		SelectedDeviceID:                nil,
		SelectedInstanceID:              nil,
		Authority:                       SessionPlacementAuthority{},
	}, nil
}

func validSummaryPlacement(observation SessionPlacementObservation, owner Owner) bool {
	return observation.SchemaVersion == SessionPlacementObservationSchemaVersion &&
		observation.EvaluationMode == EvaluationMode && observation.Owner == owner &&
		validSessionIdentifier(observation.ConversationID) && validSessionIdentifier(observation.RunID) &&
		observation.EvaluatedAtMS > 0 && observation.EvaluatedAtMS <= MaxSafeIntegerMS &&
		observation.OwnerDeclarationUnverified && observation.DeviceAttributesUnverified &&
		observation.SelectedDeviceID == nil && observation.SelectedInstanceID == nil &&
		observation.Authority == (SessionPlacementAuthority{})
}

func addUint64(left, right uint64) (uint64, bool) {
	max := uint64(MaxSafeIntegerMS)
	if left > max || right > max-left {
		return 0, false
	}
	return left + right, true
}
