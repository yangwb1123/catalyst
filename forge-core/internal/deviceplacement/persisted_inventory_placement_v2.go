package deviceplacement

import (
	"errors"
	"sort"
)

const (
	PersistedInventoryPlacementV2SchemaVersion  = "forge.device-inventory-placement-evaluation/v2"
	PersistedInventoryPlacementV2EvaluationMode = "offline_static_only"
	PersistedInventoryPlacementV2Notice         = "Every owner, state, timestamp, resource, GPU, reservation, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only comparison selects no target and grants no execution authority."
)

// PersistedInventoryPlacementV2Evaluation is a deterministic comparison over
// the lossless v2 inventory observation. It never selects, reserves, or
// dispatches a device.
type PersistedInventoryPlacementV2Evaluation struct {
	SchemaVersion          string                                    `json:"schema_version"`
	EvaluationMode         string                                    `json:"evaluation_mode"`
	SourceSchemaVersion    string                                    `json:"source_schema_version"`
	Owner                  Owner                                     `json:"evaluation_owner"`
	EvaluatedAtMS          int64                                     `json:"evaluated_at_ms"`
	Notice                 string                                    `json:"notice"`
	Decisions              []PersistedInventoryPlacementV2Decision   `json:"decisions"`
	EligibleCandidateCount int                                       `json:"eligible_candidate_count"`
	SelectedDeviceID       *string                                   `json:"selected_device_id"`
	SelectedInstanceID     *string                                   `json:"selected_instance_id"`
	Authority              PersistedInventoryPlacementBatchAuthority `json:"authority"`
}

// PersistedInventoryPlacementV2Decision keeps the source counters,
// reservation declaration, and aggregate GPU capacity beside the comparison
// result so a future authorized scheduler cannot accidentally lose identity.
type PersistedInventoryPlacementV2Decision struct {
	Revision                   uint64   `json:"revision"`
	Generation                 uint64   `json:"generation"`
	HeartbeatSequence          uint64   `json:"heartbeat_sequence"`
	DeviceID                   string   `json:"device_id"`
	InstanceID                 string   `json:"instance_id"`
	ReservationState           string   `json:"reservation_state"`
	GPUCount                   int      `json:"gpu_count"`
	AvailableGPUMemoryBytes    uint64   `json:"available_gpu_memory_bytes"`
	MatchesRequirements        bool     `json:"matches_requirements"`
	ExclusionReasons           []string `json:"exclusion_reasons"`
	OwnerDeclarationUnverified bool     `json:"owner_declaration_unverified"`
	DeviceAttributesUnverified bool     `json:"device_attributes_unverified"`
}

// EvaluatePersistedInventoryObservationV2 compares every v2 candidate at the
// caller-supplied fixed time. The v2 observation remains unverified input;
// reservation is an exclusion declaration, not a mutation or authority.
func EvaluatePersistedInventoryObservationV2(
	observation SessionDeviceObservationInventoryV2,
	owner Owner,
	requirements Requirements,
	evaluatedAtMS int64,
) (PersistedInventoryPlacementV2Evaluation, error) {
	if err := ValidateSessionDeviceObservationInventoryV2(observation); err != nil {
		return PersistedInventoryPlacementV2Evaluation{}, err
	}
	if observation.Owner != owner || !validOwner(owner) || evaluatedAtMS <= 0 || evaluatedAtMS > MaxSafeIntegerMS {
		return PersistedInventoryPlacementV2Evaluation{}, errInvalidRequest
	}
	if validateRequirements(requirements) != nil {
		return PersistedInventoryPlacementV2Evaluation{}, errInvalidRequest
	}
	// v2 preserves GPU identity and memory, but deliberately does not claim a
	// GPU runtime. A runtime-specific accelerator request therefore fails closed.
	if requirements.GPU.Runtime != "" {
		return PersistedInventoryPlacementV2Evaluation{}, errors.New("unsupported_persisted_placement_gpu_runtime")
	}

	decisions := make([]PersistedInventoryPlacementV2Decision, 0, len(observation.Devices))
	for _, candidate := range observation.Devices {
		decision := evaluatePersistedInventoryV2Candidate(candidate, requirements, evaluatedAtMS)
		decisions = append(decisions, decision)
	}
	sort.Slice(decisions, func(left, right int) bool {
		if decisions[left].DeviceID == decisions[right].DeviceID {
			return decisions[left].InstanceID < decisions[right].InstanceID
		}
		return decisions[left].DeviceID < decisions[right].DeviceID
	})
	eligible := 0
	for _, decision := range decisions {
		if decision.MatchesRequirements {
			eligible++
		}
	}
	return PersistedInventoryPlacementV2Evaluation{
		SchemaVersion:          PersistedInventoryPlacementV2SchemaVersion,
		EvaluationMode:         PersistedInventoryPlacementV2EvaluationMode,
		SourceSchemaVersion:    SessionDeviceObservationInventoryV2SchemaVersion,
		Owner:                  owner,
		EvaluatedAtMS:          evaluatedAtMS,
		Notice:                 PersistedInventoryPlacementV2Notice,
		Decisions:              decisions,
		EligibleCandidateCount: eligible,
		SelectedDeviceID:       nil,
		SelectedInstanceID:     nil,
		Authority:              PersistedInventoryPlacementBatchAuthority{},
	}, nil
}

func evaluatePersistedInventoryV2Candidate(
	candidate SessionPlacementCandidateV2,
	requirements Requirements,
	evaluatedAtMS int64,
) PersistedInventoryPlacementV2Decision {
	device := candidate.Device
	reasons := make([]string, 0, 12)
	if device.ApprovalState == "pending" {
		reasons = append(reasons, "approval_pending")
	} else if device.ApprovalState == "revoked" {
		reasons = append(reasons, "device_revoked")
	}
	if device.CordonState == "cordoned" {
		reasons = append(reasons, "device_cordoned")
	}
	if device.Liveness == "offline" {
		reasons = append(reasons, "declared_offline")
	}
	if device.SnapshotObservedAtMS > evaluatedAtMS {
		reasons = append(reasons, "snapshot_declared_from_future")
	} else if evaluatedAtMS-device.SnapshotObservedAtMS > PersistedInventoryPlacementStaleAfterMS {
		reasons = append(reasons, "snapshot_stale")
	}
	if device.LeaseExpiresAtMS <= evaluatedAtMS {
		reasons = append(reasons, "declared_lease_expired")
	}
	if device.ReservationState == "reserved" {
		reasons = append(reasons, "device_reserved")
	}
	if device.OS != requirements.OS {
		reasons = append(reasons, "os_mismatch")
	}
	if device.Architecture != requirements.Architecture {
		reasons = append(reasons, "architecture_mismatch")
	}
	if device.AvailableCPUCores < requirements.MinCPUCores {
		reasons = append(reasons, "cpu_cores_insufficient")
	}
	if device.AvailableMemoryBytes < requirements.MinMemoryBytes {
		reasons = append(reasons, "memory_insufficient")
	}
	if device.AvailableStorage < requirements.MinStorageBytes {
		reasons = append(reasons, "storage_insufficient")
	}
	if !contains(device.Runtimes, requirements.Runtime) {
		reasons = append(reasons, "runtime_missing")
	}
	if requirements.GPU.Required {
		if len(device.GPUs) == 0 {
			reasons = append(reasons, "gpu_missing")
		} else {
			sufficient := false
			for _, gpu := range device.GPUs {
				if gpu.AvailableMemoryBytes >= requirements.GPU.MinMemoryBytes {
					sufficient = true
					break
				}
			}
			if !sufficient {
				reasons = append(reasons, "gpu_memory_insufficient")
			}
		}
	}
	if !intersects(requirements.DataResidencyZones, device.DataResidencyZones) {
		reasons = append(reasons, "data_residency_zone_mismatch")
	}
	if device.TrustZone == "unknown" {
		reasons = append(reasons, "trust_zone_unconfirmed")
	} else if trustRankValue(device.TrustZone) < trustRankValue(requirements.MinimumTrustZone) {
		reasons = append(reasons, "trust_zone_below_minimum")
	}
	if !sandboxFloorMet(requirements.SandboxFloor, device.SandboxLevels) {
		reasons = append(reasons, "sandbox_floor_unmet")
	}
	if device.ActiveConcurrency > device.ConcurrencyLimit ||
		requirements.ConcurrencySlots > device.ConcurrencyLimit-device.ActiveConcurrency {
		reasons = append(reasons, "concurrency_capacity_insufficient")
	}
	sort.Strings(reasons)
	availableGPUMemoryBytes := uint64(0)
	for _, gpu := range device.GPUs {
		// ValidateSessionDeviceObservationInventoryV2 already guarantees this
		// checked sum. Keep the evaluator defensive so a future caller cannot
		// silently wrap the cross-client JSON-safe aggregate.
		availableGPUMemoryBytes, _ = addSafeGPUBytes(availableGPUMemoryBytes, gpu.AvailableMemoryBytes)
	}
	return PersistedInventoryPlacementV2Decision{
		Revision:                   candidate.Revision,
		Generation:                 candidate.Generation,
		HeartbeatSequence:          candidate.HeartbeatSequence,
		DeviceID:                   device.DeviceID,
		InstanceID:                 candidate.InstanceID,
		ReservationState:           device.ReservationState,
		GPUCount:                   len(device.GPUs),
		AvailableGPUMemoryBytes:    availableGPUMemoryBytes,
		MatchesRequirements:        len(reasons) == 0,
		ExclusionReasons:           reasons,
		OwnerDeclarationUnverified: true,
		DeviceAttributesUnverified: true,
	}
}

func trustRankValue(value string) int {
	switch value {
	case "untrusted":
		return 0
	case "low":
		return 1
	case "standard":
		return 2
	case "high":
		return 3
	case "restricted":
		return 4
	default:
		return -1
	}
}
