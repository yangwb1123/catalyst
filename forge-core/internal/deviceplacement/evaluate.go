package deviceplacement

import "sort"

const resultNotice = "All owner, approval, liveness, resource, residency, trust, and sandbox attributes are unverified caller declarations. This offline comparison selects no target and grants no execution authority."

// Evaluate compares each declaration independently at the fixed request time.
func Evaluate(request Request) (Result, error) {
	if validateRequest(request) != nil {
		return Result{}, errInvalidRequest
	}
	results := make([]DeviceResult, 0, len(request.Devices))
	for _, device := range request.Devices {
		results = append(results, evaluateDevice(request, device))
	}
	sort.Slice(results, func(left, right int) bool {
		return results[left].DeviceID < results[right].DeviceID
	})
	return Result{
		SchemaVersion: ResultSchemaVersion, EvaluationMode: EvaluationMode,
		EvaluatedAtMS: request.EvaluatedAtMS, Owner: request.Owner,
		OwnerDeclarationUnverified: true, DeviceAttributesUnverified: true,
		Notice: resultNotice, DeviceResults: results,
		ExecutionAuthorized: false, ReservationCreated: false, DispatchPerformed: false,
	}, nil
}

func evaluateDevice(request Request, device Device) DeviceResult {
	reasons := make([]string, 0, 12)
	appendStateReasons(&reasons, request, device)
	appendResourceReasons(&reasons, request.Requirements, device)
	appendPolicyReasons(&reasons, request.Requirements, device)
	sort.Strings(reasons)
	return DeviceResult{
		DeviceID: device.DeviceID, AttributesUnverified: true,
		MatchesRequirements: len(reasons) == 0, ExclusionReasons: reasons,
	}
}

func appendStateReasons(reasons *[]string, request Request, device Device) {
	if device.Owner != request.Owner {
		*reasons = append(*reasons, "owner_mismatch")
	}
	switch device.ApprovalState {
	case "approved":
	case "pending":
		*reasons = append(*reasons, "approval_pending")
	case "revoked":
		*reasons = append(*reasons, "device_revoked")
	default:
		*reasons = append(*reasons, "approval_unconfirmed")
	}
	switch device.CordonState {
	case "clear":
	case "cordoned":
		*reasons = append(*reasons, "device_cordoned")
	default:
		*reasons = append(*reasons, "cordon_unconfirmed")
	}
	appendLivenessReason(reasons, device.Liveness)
	appendFreshnessReason(reasons, request, device)
	if device.LeaseExpiresAtMS <= request.EvaluatedAtMS {
		*reasons = append(*reasons, "declared_lease_expired")
	}
}

func appendLivenessReason(reasons *[]string, liveness string) {
	if liveness == "offline" {
		*reasons = append(*reasons, "declared_offline")
	} else if liveness != "online" {
		*reasons = append(*reasons, "liveness_unconfirmed")
	}
}

func appendFreshnessReason(reasons *[]string, request Request, device Device) {
	if device.SnapshotObservedAtMS > request.EvaluatedAtMS {
		*reasons = append(*reasons, "snapshot_declared_from_future")
		return
	}
	if request.EvaluatedAtMS-device.SnapshotObservedAtMS > request.MaxSnapshotAgeMS {
		*reasons = append(*reasons, "snapshot_stale")
	}
}

func appendResourceReasons(reasons *[]string, requirement Requirements, device Device) {
	if device.OS != requirement.OS {
		*reasons = append(*reasons, "os_mismatch")
	}
	if device.Architecture != requirement.Architecture {
		*reasons = append(*reasons, "architecture_mismatch")
	}
	if device.AvailableCPUCores < requirement.MinCPUCores {
		*reasons = append(*reasons, "cpu_cores_insufficient")
	}
	if device.AvailableMemoryBytes < requirement.MinMemoryBytes {
		*reasons = append(*reasons, "memory_insufficient")
	}
	if device.AvailableStorage < requirement.MinStorageBytes {
		*reasons = append(*reasons, "storage_insufficient")
	}
	if !contains(device.Runtimes, requirement.Runtime) {
		*reasons = append(*reasons, "runtime_missing")
	}
	appendGPUReasons(reasons, requirement.GPU, device.GPU)
	if device.ActiveConcurrency > device.ConcurrencyLimit ||
		requirement.ConcurrencySlots > device.ConcurrencyLimit-device.ActiveConcurrency {
		*reasons = append(*reasons, "concurrency_capacity_insufficient")
	}
}

func appendGPUReasons(reasons *[]string, requirement GPURequirement, declared GPUDeclaration) {
	if !requirement.Required {
		return
	}
	if !declared.Present {
		*reasons = append(*reasons, "gpu_missing")
		return
	}
	if declared.MemoryBytes < requirement.MinMemoryBytes {
		*reasons = append(*reasons, "gpu_memory_insufficient")
	}
	if requirement.Runtime != "" && declared.Runtime != requirement.Runtime {
		*reasons = append(*reasons, "gpu_runtime_mismatch")
	}
}

func appendPolicyReasons(reasons *[]string, requirement Requirements, device Device) {
	if !intersects(requirement.DataResidencyZones, device.DataResidencyZones) {
		*reasons = append(*reasons, "data_residency_zone_mismatch")
	}
	appendTrustReason(reasons, requirement.MinimumTrustZone, device.TrustZone)
	if !sandboxFloorMet(requirement.SandboxFloor, device.SandboxLevels) {
		*reasons = append(*reasons, "sandbox_floor_unmet")
	}
}

func appendTrustReason(reasons *[]string, minimum, declared string) {
	declaredRank, known := trustRank(declared)
	minimumRank, _ := trustRank(minimum)
	if !known {
		*reasons = append(*reasons, "trust_zone_unconfirmed")
	} else if declaredRank < minimumRank {
		*reasons = append(*reasons, "trust_zone_below_minimum")
	}
}

func trustRank(value string) (int, bool) {
	switch value {
	case "untrusted":
		return 0, true
	case "low":
		return 1, true
	case "standard":
		return 2, true
	case "high":
		return 3, true
	case "restricted":
		return 4, true
	default:
		return 0, false
	}
}

func sandboxFloorMet(floor string, declared []string) bool {
	minimum, _ := sandboxRank(floor)
	for _, level := range declared {
		actual, _ := sandboxRank(level)
		if actual >= minimum {
			return true
		}
	}
	return false
}

func sandboxRank(value string) (int, bool) {
	switch value {
	case "process":
		return 1, true
	case "container":
		return 2, true
	case "microvm":
		return 3, true
	default:
		return 0, false
	}
}

func contains(values []string, expected string) bool {
	for _, value := range values {
		if value == expected {
			return true
		}
	}
	return false
}

func intersects(left, right []string) bool {
	for _, value := range left {
		if contains(right, value) {
			return true
		}
	}
	return false
}
