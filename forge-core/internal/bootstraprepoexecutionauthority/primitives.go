package bootstraprepoexecutionauthority

import (
	"encoding/base64"
	"encoding/hex"
	"fmt"
	"regexp"
	"strings"
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

func validateHashField(document map[string]any, field, label string) error {
	value, err := stringValue(document, field)
	if err != nil {
		return err
	}
	return validateHash(value, label)
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
	if err != nil || (size >= 0 && len(decoded) != size) ||
		base64.RawURLEncoding.EncodeToString(decoded) != value {
		return nil, fmt.Errorf("%s is not canonical unpadded base64url", label)
	}
	return decoded, nil
}

func validateRepoPath(value string) error {
	if value == "" || value == "." || len(value) > 4096 || strings.HasPrefix(value, "/") ||
		strings.HasSuffix(value, "/") || strings.Contains(value, "//") ||
		strings.Contains(value, "\\") || strings.ContainsAny(value, "*?[]{}") {
		return fmt.Errorf("repository path is not canonical")
	}
	segments := strings.Split(value, "/")
	if len(segments) > 256 {
		return fmt.Errorf("repository path exceeds 256 components")
	}
	if strings.EqualFold(segments[0], ".git") || strings.EqualFold(segments[0], ".forge") {
		return fmt.Errorf("repository control directory is forbidden")
	}
	for _, segment := range segments {
		if segment == "." || segment == ".." {
			return fmt.Errorf("repository path segment is unsafe")
		}
	}
	return validateWireString(value)
}

func sameCanonical(left, right any) bool {
	leftBytes, leftErr := canonicalJSON(left)
	rightBytes, rightErr := canonicalJSON(right)
	return leftErr == nil && rightErr == nil && string(leftBytes) == string(rightBytes)
}

func oneOf(value string, allowed ...string) bool {
	for _, candidate := range allowed {
		if value == candidate {
			return true
		}
	}
	return false
}
