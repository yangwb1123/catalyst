package deviceplacement

import (
	"bytes"
	"encoding/json"
)

func exactPersistedInventoryPlacementV2FixtureShape(data []byte) bool {
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil || root == nil || len(root) != 13 ||
		!hasNonNullFields(root,
			"schema_version", "evaluation_mode", "source_schema_version", "evaluation_owner", "evaluated_at_ms", "notice",
			"requirements", "observation", "expected", "eligible_candidate_count", "authority") ||
		!isNull(root["selected_device_id"]) || !isNull(root["selected_instance_id"]) {
		return false
	}
	return exactOwnerShape(root["evaluation_owner"]) && exactRequirementsShape(root["requirements"]) &&
		exactObservationV2Shape(root["observation"]) && exactExpectedV2Shape(root["expected"]) && exactAuthorityShape(root["authority"])
}

func exactOwnerShape(data []byte) bool {
	_, ok := exactJSONObject(data, "issuer", "subject", "tenant_id")
	return ok
}

func exactRequirementsShape(data []byte) bool {
	value, ok := exactJSONObject(data,
		"os", "architecture", "min_cpu_cores", "min_memory_bytes", "min_storage_bytes", "runtime", "gpu",
		"data_residency_zones", "minimum_trust_zone", "sandbox_floor", "concurrency_slots")
	if !ok {
		return false
	}
	_, ok = exactJSONObject(value["gpu"], "required", "min_memory_bytes", "runtime")
	return ok
}

func exactObservationV2Shape(data []byte) bool {
	value, ok := exactJSONObject(data,
		"schema_version", "evaluation_mode", "evaluated_at_ms", "owner_declaration", "owner_declaration_unverified",
		"inventory_declarations_unverified", "notice", "devices", "execution_authorized", "reservation_created", "dispatch_performed")
	if !ok || !exactOwnerShape(value["owner_declaration"]) {
		return false
	}
	var devices []json.RawMessage
	if json.Unmarshal(value["devices"], &devices) != nil || devices == nil {
		return false
	}
	for _, candidate := range devices {
		if !exactObservationV2CandidateShape(candidate) {
			return false
		}
	}
	return true
}

func exactObservationV2CandidateShape(data []byte) bool {
	value, ok := exactJSONObject(data, "instance_id", "revision", "generation", "heartbeat_sequence", "device")
	return ok && exactObservationV2DeviceShape(value["device"])
}

func exactObservationV2DeviceShape(data []byte) bool {
	value, ok := exactJSONObject(data,
		"device_id", "owner", "approval_state", "cordon_state", "reservation_state", "liveness", "snapshot_observed_at_ms",
		"lease_expires_at_ms", "os", "architecture", "available_cpu_cores", "available_memory_bytes", "available_storage_bytes",
		"runtimes", "gpus", "data_residency_zones", "trust_zone", "sandbox_levels", "concurrency_limit", "active_concurrency")
	if !ok || !exactOwnerShape(value["owner"]) {
		return false
	}
	var gpus []json.RawMessage
	if json.Unmarshal(value["gpus"], &gpus) != nil || gpus == nil {
		return false
	}
	for _, gpu := range gpus {
		if _, ok := exactJSONObject(gpu, "id", "vendor", "memory_bytes", "available_memory_bytes"); !ok {
			return false
		}
	}
	return true
}

func exactExpectedV2Shape(data []byte) bool {
	var decisions []json.RawMessage
	if json.Unmarshal(data, &decisions) != nil || decisions == nil {
		return false
	}
	for _, decision := range decisions {
		if _, ok := exactJSONObject(decision,
			"revision", "generation", "heartbeat_sequence", "device_id", "instance_id", "reservation_state", "gpu_count",
			"available_gpu_memory_bytes", "matches_requirements", "exclusion_reasons"); !ok {
			return false
		}
	}
	return true
}

func exactAuthorityShape(data []byte) bool {
	_, ok := exactJSONObject(data,
		"identity_verified", "heartbeat_persisted", "inventory_authoritative", "placement_selected", "reservation_created",
		"execution_authorized", "dispatch_performed")
	return ok
}

func exactJSONObject(data []byte, fields ...string) (map[string]json.RawMessage, bool) {
	var value map[string]json.RawMessage
	if json.Unmarshal(data, &value) != nil || value == nil || len(value) != len(fields) || !hasNonNullFields(value, fields...) {
		return nil, false
	}
	return value, true
}

func hasNonNullFields(value map[string]json.RawMessage, fields ...string) bool {
	for _, field := range fields {
		item, exists := value[field]
		if !exists || len(item) == 0 || isNull(item) {
			return false
		}
	}
	return true
}

func isNull(data []byte) bool {
	return bytes.Equal(bytes.TrimSpace(data), []byte("null"))
}
