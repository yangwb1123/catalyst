package approvalrecordcontract

import (
	"bytes"
	"encoding/hex"
	"fmt"
	"sort"
)

func requireKeys(object map[string]any, keys ...string) error {
	if len(object) != len(keys) {
		return fmt.Errorf("fields mismatch")
	}
	for _, key := range keys {
		if _, ok := object[key]; !ok {
			return fmt.Errorf("missing field %q", key)
		}
	}
	return nil
}

func objectValue(parent map[string]any, key string) (map[string]any, error) {
	value, ok := parent[key].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("%s must be an object", key)
	}
	return value, nil
}

func arrayValue(parent map[string]any, key string) ([]any, error) {
	value, ok := parent[key].([]any)
	if !ok {
		return nil, fmt.Errorf("%s must be an array", key)
	}
	return value, nil
}

func stringValue(parent map[string]any, key string) (string, error) {
	value, ok := parent[key].(string)
	if !ok {
		return "", fmt.Errorf("%s must be a string", key)
	}
	return value, nil
}

func intValue(parent map[string]any, key string) (int64, error) {
	value, ok := parent[key].(int64)
	if !ok {
		return 0, fmt.Errorf("%s must be an integer", key)
	}
	return value, nil
}

func boolValue(parent map[string]any, key string) (bool, error) {
	value, ok := parent[key].(bool)
	if !ok {
		return false, fmt.Errorf("%s must be a boolean", key)
	}
	return value, nil
}

func nullableStringValue(parent map[string]any, key string) (*string, error) {
	if parent[key] == nil {
		return nil, nil
	}
	value, err := stringValue(parent, key)
	if err != nil {
		return nil, err
	}
	return &value, nil
}

func validateText(value, label string, maximum int) error {
	if value == "" || len(value) > maximum {
		return fmt.Errorf("%s byte length must be 1..%d", label, maximum)
	}
	return validateWireString(value)
}

func validateHash(value, label string) error {
	if len(value) != 64 {
		return fmt.Errorf("%s must be 64 lowercase hex characters", label)
	}
	decoded, err := hex.DecodeString(value)
	if err != nil || hex.EncodeToString(decoded) != value {
		return fmt.Errorf("%s must be 64 lowercase hex characters", label)
	}
	return nil
}

func validateEnum(value, label string, allowed ...string) error {
	for _, candidate := range allowed {
		if value == candidate {
			return nil
		}
	}
	return fmt.Errorf("%s has unsupported value %q", label, value)
}

func validateBoundedInt(value int64, label string, minimum, maximum int64) error {
	if value < minimum || value > maximum {
		return fmt.Errorf("%s must be in %d..%d", label, minimum, maximum)
	}
	return nil
}

func validateSortedNodes(values []any, key func(map[string]any) ([]byte, error)) error {
	var previous []byte
	for index, value := range values {
		node, ok := value.(map[string]any)
		if !ok {
			return fmt.Errorf("item %d must be an object", index)
		}
		current, err := key(node)
		if err != nil {
			return err
		}
		if index > 0 && bytes.Compare(previous, current) >= 0 {
			return fmt.Errorf("items must be strictly sorted and unique")
		}
		previous = append(previous[:0], current...)
	}
	return nil
}

func canonicalNodeKey(node map[string]any) ([]byte, error) {
	return canonicalJSON(node)
}

func readStringArray(parent map[string]any, key string, minimum, maximum int) ([]string, error) {
	values, err := arrayValue(parent, key)
	if err != nil || len(values) < minimum || len(values) > maximum {
		return nil, fmt.Errorf("%s must contain %d..%d items", key, minimum, maximum)
	}
	result := make([]string, len(values))
	for index, value := range values {
		text, ok := value.(string)
		if !ok || validateText(text, key, maxStringBytes) != nil {
			return nil, fmt.Errorf("%s item %d is invalid", key, index)
		}
		result[index] = text
	}
	if !sort.StringsAreSorted(result) || hasAdjacentDuplicate(result) {
		return nil, fmt.Errorf("%s must be strictly sorted and unique", key)
	}
	return result, nil
}

func hasAdjacentDuplicate(values []string) bool {
	for index := 1; index < len(values); index++ {
		if values[index-1] == values[index] {
			return true
		}
	}
	return false
}
