package capabilitygrantcontract

import (
	"bytes"
	"encoding/hex"
	"fmt"
	"net"
	"path"
	"sort"
	"strings"
)

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

func validateCanonicalPath(value, label string, allowRoot bool) error {
	if err := validateText(value, label, 4096); err != nil {
		return err
	}
	if strings.Contains(value, "\\") || strings.HasPrefix(value, "/") ||
		strings.HasSuffix(value, "/") || strings.ContainsAny(value, "*?[]{}") {
		return fmt.Errorf("%s is not a canonical repo-relative path", label)
	}
	if value == "." {
		if allowRoot {
			return nil
		}
		return fmt.Errorf("%s cannot name the repository root", label)
	}
	if path.Clean(value) != value {
		return fmt.Errorf("%s is not a canonical repo-relative path", label)
	}
	for _, segment := range strings.Split(value, "/") {
		if segment == "" || segment == "." || segment == ".." {
			return fmt.Errorf("%s has an unsafe path segment", label)
		}
	}
	return nil
}

func validateHost(value, kind string) error {
	if err := validateText(value, "network host", 253); err != nil {
		return err
	}
	switch kind {
	case "ipv4":
		parsed := net.ParseIP(value)
		if parsed == nil || parsed.To4() == nil || parsed.String() != value {
			return fmt.Errorf("network host is not canonical IPv4")
		}
	case "ipv6":
		parsed := net.ParseIP(value)
		if parsed == nil || parsed.To4() != nil || parsed.String() != value {
			return fmt.Errorf("network host is not canonical lowercase IPv6")
		}
	case "dns":
		parsed := net.ParseIP(value)
		if parsed != nil && parsed.To4() != nil && parsed.String() == value {
			return fmt.Errorf("network DNS host cannot be a canonical IPv4 literal")
		}
		if value != strings.ToLower(value) || strings.HasSuffix(value, ".") ||
			strings.Contains(value, "*") {
			return fmt.Errorf("network host is not canonical DNS")
		}
		for _, label := range strings.Split(value, ".") {
			if label == "" || len(label) > 63 || strings.HasPrefix(label, "-") ||
				strings.HasSuffix(label, "-") {
				return fmt.Errorf("network host is not canonical DNS")
			}
			for _, character := range []byte(label) {
				if (character < 'a' || character > 'z') &&
					(character < '0' || character > '9') && character != '-' {
					return fmt.Errorf("network host is not canonical DNS")
				}
			}
		}
	default:
		return fmt.Errorf("network host_kind is unsupported")
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

func sortedUniqueStrings(values []string) bool {
	return sort.StringsAreSorted(values) && !hasAdjacentDuplicate(values)
}

func hasAdjacentDuplicate(values []string) bool {
	for index := 1; index < len(values); index++ {
		if values[index-1] == values[index] {
			return true
		}
	}
	return false
}
