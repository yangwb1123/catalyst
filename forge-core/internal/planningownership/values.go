package planningownership

import (
	"fmt"
	"regexp"
)

var identifierPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9._:/-]*$`)

func requireKeys(object map[string]any, keys []string) error {
	if len(object) != len(keys) {
		return fmt.Errorf("object has %d fields; expected %d", len(object), len(keys))
	}
	for _, key := range keys {
		if _, exists := object[key]; !exists {
			return fmt.Errorf("missing required field %q", key)
		}
	}
	return nil
}

func requireString(object map[string]any, key, expected string) error {
	actual, ok := object[key].(string)
	if !ok || actual != expected {
		return fmt.Errorf("field %q must equal %q", key, expected)
	}
	return nil
}

func objectValue(object map[string]any, key string) (map[string]any, error) {
	value, ok := object[key].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("field %q must be an object", key)
	}
	return value, nil
}

func arrayValue(object map[string]any, key string, minimum, maximum int) ([]any, error) {
	value, ok := object[key].([]any)
	if !ok || len(value) < minimum || len(value) > maximum {
		return nil, fmt.Errorf("field %q must contain %d..%d items", key, minimum, maximum)
	}
	return value, nil
}

func integerValue(object map[string]any, key string, minimum, maximum int64) (int64, error) {
	value, ok := object[key].(int64)
	if !ok || value < minimum || value > maximum {
		return 0, fmt.Errorf("field %q must be an integer in %d..%d", key, minimum, maximum)
	}
	return value, nil
}

func stringValue(object map[string]any, key string, minimum, maximum int) (string, error) {
	value, ok := object[key].(string)
	if !ok || len(value) < minimum || len(value) > maximum || validateWireString(value) != nil {
		return "", fmt.Errorf("field %q must be a %d..%d byte string", key, minimum, maximum)
	}
	return value, nil
}

func requireBool(object map[string]any, key string, expected bool) error {
	value, ok := object[key].(bool)
	if !ok || value != expected {
		return fmt.Errorf("field %q must equal %t", key, expected)
	}
	return nil
}

func validIdentifier(value string) bool {
	return len(value) <= maxIdentifierBytes && identifierPattern.MatchString(value)
}

func validHash(value string) bool {
	if len(value) != 64 {
		return false
	}
	for _, character := range []byte(value) {
		if (character < '0' || character > '9') && (character < 'a' || character > 'f') {
			return false
		}
	}
	return true
}

func objectItems(values []any) ([]map[string]any, error) {
	result := make([]map[string]any, len(values))
	for index, item := range values {
		value, ok := item.(map[string]any)
		if !ok {
			return nil, fmt.Errorf("item %d must be an object", index)
		}
		result[index] = value
	}
	return result, nil
}

func stringsFromArray(values []any, validator func(string) bool) ([]string, error) {
	result := make([]string, len(values))
	for index, item := range values {
		value, ok := item.(string)
		if !ok || !validator(value) {
			return nil, fmt.Errorf("item %d must be a valid string", index)
		}
		result[index] = value
	}
	return result, nil
}

func hasDuplicate(values []string) bool {
	seen := make(map[string]struct{}, len(values))
	for _, value := range values {
		if _, exists := seen[value]; exists {
			return true
		}
		seen[value] = struct{}{}
	}
	return false
}
