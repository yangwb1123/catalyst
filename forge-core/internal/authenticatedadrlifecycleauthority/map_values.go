package authenticatedadrlifecycleauthority

import (
	"fmt"
	"sort"
)

func objectValue(value any, label string) (map[string]any, error) {
	result, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("%s must be an object", label)
	}
	return result, nil
}

func arrayValue(value any, label string) ([]any, error) {
	result, ok := value.([]any)
	if !ok {
		return nil, fmt.Errorf("%s must be an array", label)
	}
	return result, nil
}

func stringValue(value any, label string) (string, error) {
	result, ok := value.(string)
	if !ok || result == "" {
		return "", fmt.Errorf("%s must be a nonempty string", label)
	}
	return result, nil
}

func intValue(value any, label string) (int64, error) {
	result, ok := value.(int64)
	if !ok || result < 0 {
		return 0, fmt.Errorf("%s must be a nonnegative signed int64", label)
	}
	return result, nil
}

func stringField(node map[string]any, field string) (string, error) {
	return stringValue(node[field], field)
}

func intField(node map[string]any, field string) (int64, error) {
	return intValue(node[field], field)
}

func objectField(node map[string]any, field string) (map[string]any, error) {
	return objectValue(node[field], field)
}

func arrayField(node map[string]any, field string) ([]any, error) {
	return arrayValue(node[field], field)
}

func requireFields(node map[string]any, label string, fields ...string) error {
	if len(node) != len(fields) {
		return fmt.Errorf("%s has %d fields; want %d", label, len(node), len(fields))
	}
	for _, field := range fields {
		if _, exists := node[field]; !exists {
			return fmt.Errorf("%s misses %s", label, field)
		}
	}
	return nil
}

func sortByADRID(values []any) {
	sort.Slice(values, func(left, right int) bool {
		leftNode, leftOK := values[left].(map[string]any)
		rightNode, rightOK := values[right].(map[string]any)
		if !leftOK || !rightOK {
			return left < right
		}
		leftID, _ := leftNode["adr_id"].(string)
		rightID, _ := rightNode["adr_id"].(string)
		return leftID < rightID
	})
}

func validIdempotency(value string) bool {
	if len(value) < 16 || len(value) > 128 {
		return false
	}
	for index := range value {
		current := value[index]
		if (current >= 'A' && current <= 'Z') || (current >= 'a' && current <= 'z') ||
			(current >= '0' && current <= '9') || current == '.' || current == '_' ||
			current == ':' || current == '@' || current == '+' || current == '-' {
			continue
		}
		return false
	}
	return true
}
