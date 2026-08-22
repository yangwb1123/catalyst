package knowledgeupdateproposalcontract

import (
	"bytes"
	"fmt"
	"regexp"
	"sort"
)

var (
	hashPattern       = regexp.MustCompile(`^[a-f0-9]{64}$`)
	identifierPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9._:/-]*$`)
)

func requireKeys(node map[string]any, keys ...string) error {
	if len(node) != len(keys) {
		return fmt.Errorf("has %d fields; expected exactly %d", len(node), len(keys))
	}
	for _, key := range keys {
		if _, exists := node[key]; !exists {
			return fmt.Errorf("missing required field %q", key)
		}
	}
	return nil
}

func objectValue(node map[string]any, key string) (map[string]any, error) {
	value, exists := node[key]
	if !exists {
		return nil, fmt.Errorf("missing required field %q", key)
	}
	object, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("%s must be an object", key)
	}
	return object, nil
}

func arrayValue(node map[string]any, key string) ([]any, error) {
	value, exists := node[key]
	if !exists {
		return nil, fmt.Errorf("missing required field %q", key)
	}
	array, ok := value.([]any)
	if !ok {
		return nil, fmt.Errorf("%s must be an array", key)
	}
	return array, nil
}

func stringValue(node map[string]any, key string) (string, error) {
	value, exists := node[key]
	if !exists {
		return "", fmt.Errorf("missing required field %q", key)
	}
	text, ok := value.(string)
	if !ok {
		return "", fmt.Errorf("%s must be a string", key)
	}
	return text, nil
}

func nullableStringValue(node map[string]any, key string) (*string, error) {
	value, exists := node[key]
	if !exists {
		return nil, fmt.Errorf("missing required field %q", key)
	}
	if value == nil {
		return nil, nil
	}
	text, ok := value.(string)
	if !ok {
		return nil, fmt.Errorf("%s must be a string or null", key)
	}
	return &text, nil
}

func intValue(node map[string]any, key string) (int64, error) {
	value, exists := node[key]
	if !exists {
		return 0, fmt.Errorf("missing required field %q", key)
	}
	integer, ok := value.(int64)
	if !ok {
		return 0, fmt.Errorf("%s must be a signed int64", key)
	}
	return integer, nil
}

func boolValue(node map[string]any, key string) (bool, error) {
	value, exists := node[key]
	if !exists {
		return false, fmt.Errorf("missing required field %q", key)
	}
	boolean, ok := value.(bool)
	if !ok {
		return false, fmt.Errorf("%s must be a boolean", key)
	}
	return boolean, nil
}

func requireStringLiteral(node map[string]any, key, expected string) error {
	value, err := stringValue(node, key)
	if err != nil || value != expected {
		return fmt.Errorf("%s must equal %q", key, expected)
	}
	return nil
}

func validateHash(value, label string) error {
	if !hashPattern.MatchString(value) {
		return fmt.Errorf("%s must be a lowercase bare SHA-256", label)
	}
	return nil
}

func validateIdentifier(value, label string) error {
	if len(value) == 0 || len(value) > maxShortBytes || !identifierPattern.MatchString(value) {
		return fmt.Errorf("%s is not a valid identifier", label)
	}
	return nil
}

func validateText(value, label string, maximum int) error {
	if len(value) == 0 || len(value) > maximum {
		return fmt.Errorf("%s must contain 1..%d UTF-8 bytes", label, maximum)
	}
	if err := validateWireString(value); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	return nil
}

func validateEnum(value, label string, options ...string) error {
	for _, option := range options {
		if value == option {
			return nil
		}
	}
	return fmt.Errorf("%s has unsupported value %q", label, value)
}

func readStringArray(node map[string]any, key string, minimum, maximum int) ([]string, error) {
	values, err := arrayValue(node, key)
	if err != nil || len(values) < minimum || len(values) > maximum {
		return nil, fmt.Errorf("%s must contain %d..%d items", key, minimum, maximum)
	}
	result := make([]string, len(values))
	for index, value := range values {
		text, ok := value.(string)
		if !ok {
			return nil, fmt.Errorf("%s item %d must be a string", key, index)
		}
		result[index] = text
	}
	if !sort.StringsAreSorted(result) {
		return nil, fmt.Errorf("%s must be UTF-8 sorted", key)
	}
	for index := 1; index < len(result); index++ {
		if result[index] == result[index-1] {
			return nil, fmt.Errorf("%s contains duplicate %q", key, result[index])
		}
	}
	return result, nil
}

func stringsToAny(values []string) []any {
	result := make([]any, len(values))
	for index, value := range values {
		result[index] = value
	}
	return result
}

func canonicalValuesEqual(left, right any) bool {
	leftJSON, leftErr := canonicalJSON(left)
	rightJSON, rightErr := canonicalJSON(right)
	return leftErr == nil && rightErr == nil && string(leftJSON) == string(rightJSON)
}

func validateSortedNodes(values []any, label string) error {
	var previous []byte
	for index, value := range values {
		node, ok := value.(map[string]any)
		if !ok {
			return fmt.Errorf("%s item %d must be an object", label, index)
		}
		current, err := canonicalJSON(node)
		if err != nil {
			return err
		}
		if index > 0 && bytes.Compare(previous, current) >= 0 {
			return fmt.Errorf("%s must be strictly sorted and unique", label)
		}
		previous = append(previous[:0], current...)
	}
	return nil
}
