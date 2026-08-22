package bootstrapgrantauthority

import (
	"encoding/base64"
	"encoding/hex"
	"fmt"
	"regexp"
)

var (
	hashPattern        = regexp.MustCompile(`^[0-9a-f]{64}$`)
	idempotencyPattern = regexp.MustCompile(`^[A-Za-z0-9._:@+\-]{16,128}$`)
)

func objectValue(node map[string]any, key string) (map[string]any, error) {
	value, ok := node[key].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("%s must be an object", key)
	}
	return value, nil
}

func arrayValue(node map[string]any, key string) ([]any, error) {
	value, ok := node[key].([]any)
	if !ok {
		return nil, fmt.Errorf("%s must be an array", key)
	}
	return value, nil
}

func stringValue(node map[string]any, key string) (string, error) {
	value, ok := node[key].(string)
	if !ok {
		return "", fmt.Errorf("%s must be a string", key)
	}
	return value, nil
}

func intValue(node map[string]any, key string) (int64, error) {
	value, ok := node[key].(int64)
	if !ok {
		return 0, fmt.Errorf("%s must be a signed int64", key)
	}
	return value, nil
}

func requireLiteral(node map[string]any, key, expected string) error {
	value, err := stringValue(node, key)
	if err != nil || value != expected {
		return fmt.Errorf("%s must be %q", key, expected)
	}
	return nil
}

func validateText(value, label string, maximum int) error {
	if value == "" || len(value) > maximum {
		return fmt.Errorf("%s must contain 1..%d UTF-8 bytes", label, maximum)
	}
	if err := validateWireString(value); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	return nil
}

func validateHash(value, label string) error {
	if !hashPattern.MatchString(value) {
		return fmt.Errorf("%s must be lowercase SHA-256 hex", label)
	}
	return nil
}

func decodeHash(value, label string) ([]byte, error) {
	if err := validateHash(value, label); err != nil {
		return nil, err
	}
	decoded, err := hex.DecodeString(value)
	if err != nil || len(decoded) != 32 {
		return nil, fmt.Errorf("%s must encode 32 bytes", label)
	}
	return decoded, nil
}

func decodeBase64URL(value, label string, size int) ([]byte, error) {
	decoded, err := base64.RawURLEncoding.Strict().DecodeString(value)
	if err != nil || len(decoded) != size || base64.RawURLEncoding.EncodeToString(decoded) != value {
		return nil, fmt.Errorf("%s must be canonical unpadded base64url for %d bytes", label, size)
	}
	return decoded, nil
}

func validateRange(value int64, label string, minimum, maximum int64) error {
	if value < minimum || value > maximum {
		return fmt.Errorf("%s must be in %d..%d", label, minimum, maximum)
	}
	return nil
}

func sameCanonical(left, right any) (bool, error) {
	leftBytes, err := canonicalJSON(left)
	if err != nil {
		return false, err
	}
	rightBytes, err := canonicalJSON(right)
	if err != nil {
		return false, err
	}
	return string(leftBytes) == string(rightBytes), nil
}
