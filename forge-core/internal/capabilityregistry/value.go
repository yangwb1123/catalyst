package capabilityregistry

import (
	"bytes"
	"fmt"
	"strings"
)

func requireKeys(object map[string]any, keys ...string) error {
	if len(object) != len(keys) {
		return fmt.Errorf("object has %d fields; expected exactly %d", len(object), len(keys))
	}
	for _, key := range keys {
		if _, exists := object[key]; !exists {
			return fmt.Errorf("missing required field %q", key)
		}
	}
	return nil
}

func objectValue(parent map[string]any, key string) (map[string]any, error) {
	value, exists := parent[key]
	if !exists {
		return nil, fmt.Errorf("missing required field %q", key)
	}
	object, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("field %q must be an object", key)
	}
	return object, nil
}

func arrayValue(parent map[string]any, key string, minimum, maximum int) ([]any, error) {
	value, exists := parent[key]
	if !exists {
		return nil, fmt.Errorf("missing required field %q", key)
	}
	array, ok := value.([]any)
	if !ok || len(array) < minimum || len(array) > maximum {
		return nil, fmt.Errorf("field %q must contain %d..%d items", key, minimum, maximum)
	}
	return array, nil
}

func stringValue(parent map[string]any, key string, minimum, maximum int) (string, error) {
	value, exists := parent[key]
	if !exists {
		return "", fmt.Errorf("missing required field %q", key)
	}
	text, ok := value.(string)
	if !ok || len(text) < minimum || len(text) > maximum || validateWireString(text) != nil {
		return "", fmt.Errorf("field %q must be a %d..%d byte string", key, minimum, maximum)
	}
	return text, nil
}

func integerValue(parent map[string]any, key string, minimum, maximum int64) (int64, error) {
	value, exists := parent[key]
	if !exists {
		return 0, fmt.Errorf("missing required field %q", key)
	}
	integer, ok := value.(int64)
	if !ok || integer < minimum || integer > maximum {
		return 0, fmt.Errorf("field %q must be an integer in %d..%d", key, minimum, maximum)
	}
	return integer, nil
}

func requireString(parent map[string]any, key, expected string) error {
	actual, err := stringValue(parent, key, len(expected), len(expected))
	if err != nil || actual != expected {
		return fmt.Errorf("field %q must equal %q", key, expected)
	}
	return nil
}

func requireBool(parent map[string]any, key string, expected bool) error {
	actual, ok := parent[key].(bool)
	if !ok || actual != expected {
		return fmt.Errorf("field %q must equal %t", key, expected)
	}
	return nil
}

func requireNull(parent map[string]any, key string) error {
	value, exists := parent[key]
	if !exists || value != nil {
		return fmt.Errorf("field %q must be null", key)
	}
	return nil
}

func oneOf(value string, allowed ...string) bool {
	for _, candidate := range allowed {
		if value == candidate {
			return true
		}
	}
	return false
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

func validIdentifier(value string) bool {
	if len(value) == 0 || len(value) > maxIdentifierBytes ||
		(value[0] < 'a' || value[0] > 'z') && (value[0] < '0' || value[0] > '9') {
		return false
	}
	for _, character := range []byte(value) {
		if (character < 'a' || character > 'z') && (character < '0' || character > '9') &&
			!bytes.ContainsRune([]byte("._:/-"), rune(character)) {
			return false
		}
	}
	return true
}

func validOpaqueVersion(value string) bool {
	if len(value) == 0 || len(value) > maxIdentifierBytes || !isASCIIAlphaNumeric(value[0]) {
		return false
	}
	for _, character := range []byte(value) {
		if !isASCIIAlphaNumeric(character) && !bytes.ContainsRune([]byte("._-"), rune(character)) {
			return false
		}
	}
	return true
}

func isASCIIAlphaNumeric(value byte) bool {
	return value >= 'a' && value <= 'z' || value >= 'A' && value <= 'Z' || value >= '0' && value <= '9'
}

func validRepoPath(value string) bool {
	if len(value) == 0 || len(value) > maxRepoPathBytes || strings.HasPrefix(value, "/") || strings.Contains(value, "\\") {
		return false
	}
	for _, segment := range strings.Split(value, "/") {
		if segment == "" || segment == "." || segment == ".." || !validPathSegment(segment) {
			return false
		}
	}
	return true
}

func validPathSegment(value string) bool {
	for _, character := range []byte(value) {
		if !isASCIIAlphaNumeric(character) && character != '.' && character != '_' && character != '-' {
			return false
		}
	}
	return true
}

func validJSONPointer(value string, fragment bool) bool {
	if fragment {
		if value == "#" {
			return true
		}
		if !strings.HasPrefix(value, "#/") {
			return false
		}
		value = value[1:]
	} else if value != "" && !strings.HasPrefix(value, "/") {
		return false
	}
	for index := 0; index < len(value); index++ {
		if value[index] > 0x7e {
			return false
		}
		if value[index] != '~' {
			continue
		}
		if index+1 >= len(value) || value[index+1] != '0' && value[index+1] != '1' {
			return false
		}
		index++
	}
	return true
}

func requireSortedUniqueStrings(values []any, validator func(string) bool) error {
	previous := ""
	for index, item := range values {
		value, ok := item.(string)
		if !ok || !validator(value) {
			return fmt.Errorf("item %d is invalid", index)
		}
		if index > 0 && value <= previous {
			return fmt.Errorf("items must be strictly raw-UTF-8 sorted and unique")
		}
		previous = value
	}
	return nil
}

func requireObjectItems(values []any) ([]map[string]any, error) {
	result := make([]map[string]any, len(values))
	for index, item := range values {
		object, ok := item.(map[string]any)
		if !ok {
			return nil, fmt.Errorf("item %d must be an object", index)
		}
		result[index] = object
	}
	return result, nil
}
