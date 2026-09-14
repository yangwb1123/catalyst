package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"unicode/utf8"
)

// Decode reads one bounded UTF-8 request, rejecting duplicate and unknown keys.
func Decode(reader io.Reader) (Request, error) {
	data, err := io.ReadAll(io.LimitReader(reader, MaxRequestBytes+1))
	if err != nil || len(data) == 0 || len(data) > MaxRequestBytes || !utf8.Valid(data) {
		return Request{}, errInvalidRequest
	}
	if rejectDuplicateFields(data) != nil || rejectNullValues(data) != nil || !requiredRequestShape(data) {
		return Request{}, errInvalidRequest
	}
	var request Request
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if decoder.Decode(&request) != nil {
		return Request{}, errInvalidRequest
	}
	var trailing any
	if decoder.Decode(&trailing) != io.EOF || validateRequest(request) != nil {
		return Request{}, errInvalidRequest
	}
	return request, nil
}

func requiredRequestShape(data []byte) bool {
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil || !hasFields(root,
		"schema_version", "evaluated_at_ms", "owner", "max_snapshot_age_ms", "requirements", "devices") {
		return false
	}
	owner, okOwner := objectField(root, "owner")
	requirements, okRequirements := objectField(root, "requirements")
	devices, okDevices := arrayField(root, "devices")
	return okOwner && hasFields(owner, "issuer", "subject", "tenant_id") && okRequirements &&
		requiredRequirementsShape(requirements) && okDevices && requiredDeviceShapes(devices)
}

func requiredRequirementsShape(raw map[string]json.RawMessage) bool {
	if !hasFields(raw, "os", "architecture", "min_cpu_cores", "min_memory_bytes", "min_storage_bytes",
		"runtime", "gpu", "data_residency_zones", "minimum_trust_zone", "sandbox_floor", "concurrency_slots") {
		return false
	}
	gpu, ok := objectField(raw, "gpu")
	return ok && hasFields(gpu, "required", "min_memory_bytes", "runtime")
}

func requiredDeviceShapes(devices []json.RawMessage) bool {
	for _, encoded := range devices {
		var device map[string]json.RawMessage
		if json.Unmarshal(encoded, &device) != nil || !hasFields(device,
			"device_id", "owner", "approval_state", "cordon_state", "liveness", "snapshot_observed_at_ms",
			"lease_expires_at_ms", "os", "architecture", "available_cpu_cores", "available_memory_bytes",
			"available_storage_bytes", "runtimes", "gpu", "data_residency_zones", "trust_zone", "sandbox_levels",
			"concurrency_limit", "active_concurrency") {
			return false
		}
		owner, okOwner := objectField(device, "owner")
		gpu, okGPU := objectField(device, "gpu")
		if !okOwner || !hasFields(owner, "issuer", "subject", "tenant_id") ||
			!okGPU || !hasFields(gpu, "present", "memory_bytes", "runtime") {
			return false
		}
	}
	return true
}

func hasFields(object map[string]json.RawMessage, names ...string) bool {
	for _, name := range names {
		if _, exists := object[name]; !exists {
			return false
		}
	}
	return true
}

func objectField(object map[string]json.RawMessage, name string) (map[string]json.RawMessage, bool) {
	var value map[string]json.RawMessage
	err := json.Unmarshal(object[name], &value)
	return value, err == nil && value != nil
}

func arrayField(object map[string]json.RawMessage, name string) ([]json.RawMessage, bool) {
	var value []json.RawMessage
	err := json.Unmarshal(object[name], &value)
	return value, err == nil && value != nil
}

func rejectDuplicateFields(data []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	if err := scanValue(decoder, 1); err != nil {
		return err
	}
	if _, err := decoder.Token(); err != io.EOF {
		return errInvalidRequest
	}
	return nil
}

func scanValue(decoder *json.Decoder, depth int) error {
	token, err := decoder.Token()
	if err != nil {
		return err
	}
	delimiter, compound := token.(json.Delim)
	if !compound {
		return nil
	}
	if depth > MaxJSONDepth {
		return errInvalidRequest
	}
	if delimiter == '{' {
		return scanObject(decoder, depth)
	}
	if delimiter == '[' {
		return scanArray(decoder, depth)
	}
	return errInvalidRequest
}

func rejectNullValues(data []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	if err := scanNonNullValue(decoder, 1); err != nil {
		return err
	}
	if _, err := decoder.Token(); err != io.EOF {
		return errInvalidRequest
	}
	return nil
}

func scanNonNullValue(decoder *json.Decoder, depth int) error {
	token, err := decoder.Token()
	if err != nil || token == nil {
		return errInvalidRequest
	}
	delimiter, compound := token.(json.Delim)
	if !compound {
		return nil
	}
	if depth > MaxJSONDepth {
		return errInvalidRequest
	}
	switch delimiter {
	case '{':
		for decoder.More() {
			if _, err := decoder.Token(); err != nil {
				return errInvalidRequest
			}
			if err := scanNonNullValue(decoder, depth+1); err != nil {
				return err
			}
		}
		return consumeClosingDelimiter(decoder, '}')
	case '[':
		for decoder.More() {
			if err := scanNonNullValue(decoder, depth+1); err != nil {
				return err
			}
		}
		return consumeClosingDelimiter(decoder, ']')
	default:
		return errInvalidRequest
	}
}

func scanObject(decoder *json.Decoder, depth int) error {
	seen := make(map[string]struct{})
	for decoder.More() {
		token, err := decoder.Token()
		key, valid := token.(string)
		if err != nil || !valid {
			return errInvalidRequest
		}
		if _, exists := seen[key]; exists {
			return errInvalidRequest
		}
		seen[key] = struct{}{}
		if err := scanValue(decoder, depth+1); err != nil {
			return err
		}
	}
	return consumeClosingDelimiter(decoder, '}')
}

func scanArray(decoder *json.Decoder, depth int) error {
	for decoder.More() {
		if err := scanValue(decoder, depth+1); err != nil {
			return err
		}
	}
	return consumeClosingDelimiter(decoder, ']')
}

func consumeClosingDelimiter(decoder *json.Decoder, expected json.Delim) error {
	token, err := decoder.Token()
	if err != nil || token != expected {
		return errInvalidRequest
	}
	return nil
}
