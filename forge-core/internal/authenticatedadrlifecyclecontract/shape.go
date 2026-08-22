package authenticatedadrlifecyclecontract

import (
	"bytes"
	"encoding/base64"
	"encoding/hex"
	"fmt"
	"regexp"
	"sort"
)

var (
	hashPattern         = regexp.MustCompile(`^[0-9a-f]{64}$`)
	keyIDPattern        = regexp.MustCompile(`^[A-Za-z0-9][A-Za-z0-9._:-]{0,159}$`)
	idempotencyPattern  = regexp.MustCompile(`^[A-Za-z0-9._:@+\-]{16,128}$`)
	proposalNamePattern = regexp.MustCompile(`^ADR-([0-9]{4})-[a-z0-9]+(?:-[a-z0-9]+)*\.md$`)
)

func requireKeys(value any, label string, fields ...string) (map[string]any, error) {
	node, ok := value.(map[string]any)
	if !ok || len(node) != len(fields) {
		return nil, fmt.Errorf("%s fields must be exactly %v", label, sortedStrings(fields))
	}
	for _, field := range fields {
		if _, exists := node[field]; !exists {
			return nil, fmt.Errorf("%s fields must be exactly %v", label, sortedStrings(fields))
		}
	}
	return node, nil
}

func textValue(value any, label string, maximum int) (string, error) {
	result, ok := value.(string)
	if !ok || len(result) == 0 || len(result) > maximum {
		return "", fmt.Errorf("%s must be non-empty text of at most %d bytes", label, maximum)
	}
	if err := validateWireString(result); err != nil {
		return "", fmt.Errorf("%s: %w", label, err)
	}
	return result, nil
}

func enumValue(value any, label string, allowed ...string) (string, error) {
	result, ok := value.(string)
	if !ok || !containsString(allowed, result) {
		return "", fmt.Errorf("%s must be one of %v", label, allowed)
	}
	return result, nil
}

func intValue(value any, label string, minimum, maximum int64) (int64, error) {
	result, ok := value.(int64)
	if !ok || result < minimum || result > maximum {
		return 0, fmt.Errorf("%s must be an integer in %d..%d", label, minimum, maximum)
	}
	return result, nil
}

func shaValue(value any, label string) (string, error) {
	result, ok := value.(string)
	if !ok || !hashPattern.MatchString(result) {
		return "", fmt.Errorf("%s must be lowercase SHA-256 hex", label)
	}
	return result, nil
}

func adrIDValue(value any, label string) (string, error) {
	result, ok := value.(string)
	if !ok || len(result) != 8 || result == "ADR-0000" ||
		result[:4] != "ADR-" || !allDigits(result[4:]) {
		return "", fmt.Errorf("%s must be ADR-NNNN other than ADR-0000", label)
	}
	return result, nil
}

func allDigits(value string) bool {
	for _, character := range value {
		if character < '0' || character > '9' {
			return false
		}
	}
	return true
}

func arrayValue(value any, label string, minimum, maximum int) ([]any, error) {
	result, ok := value.([]any)
	if !ok || len(result) < minimum || len(result) > maximum {
		return nil, fmt.Errorf("%s must contain %d..%d items", label, minimum, maximum)
	}
	return result, nil
}

func sortedUniqueStrings(value any, label string, minimum, maximum int) ([]string, error) {
	items, err := arrayValue(value, label, minimum, maximum)
	if err != nil {
		return nil, err
	}
	result := make([]string, len(items))
	for index, item := range items {
		result[index], err = textValue(item, fmt.Sprintf("%s[%d]", label, index), 160)
		if err != nil {
			return nil, err
		}
		if index > 0 && result[index-1] >= result[index] {
			return nil, fmt.Errorf("%s must be raw-UTF-8 sorted and unique", label)
		}
	}
	return result, nil
}

func sortedUniqueNodes(value any, label string, count int) ([]any, error) {
	items, err := arrayValue(value, label, count, count)
	if err != nil {
		return nil, err
	}
	var prior []byte
	for index, item := range items {
		encoded, encodeErr := canonicalJSON(item)
		if encodeErr != nil {
			return nil, fmt.Errorf("%s[%d]: %w", label, index, encodeErr)
		}
		if index > 0 && bytes.Compare(prior, encoded) >= 0 {
			return nil, fmt.Errorf("%s must be canonical-byte sorted and unique", label)
		}
		prior = encoded
	}
	return items, nil
}

func decodeBase64URL(value any, label string, maximum int) ([]byte, error) {
	result, ok := value.(string)
	if !ok || result == "" || bytes.ContainsRune([]byte(result), '=') {
		return nil, fmt.Errorf("%s must be nonempty unpadded base64url", label)
	}
	decoded, err := base64.RawURLEncoding.Strict().DecodeString(result)
	if err != nil || len(decoded) == 0 || len(decoded) > maximum ||
		base64.RawURLEncoding.EncodeToString(decoded) != result {
		return nil, fmt.Errorf("%s is noncanonical or exceeds %d bytes", label, maximum)
	}
	return decoded, nil
}

func fixedBase64URL(value any, label string, size int) ([]byte, error) {
	decoded, err := decodeBase64URL(value, label, size)
	if err != nil || len(decoded) != size {
		return nil, fmt.Errorf("%s must encode exactly %d bytes", label, size)
	}
	return decoded, nil
}

func validatePrincipal(value any, label string, allowed ...string) (map[string]any, error) {
	node, err := requireKeys(value, label, "authority_domain", "principal_id", "principal_type")
	if err != nil {
		return nil, err
	}
	if _, err = textValue(node["authority_domain"], label+".authority_domain", 160); err != nil {
		return nil, err
	}
	if _, err = textValue(node["principal_id"], label+".principal_id", 160); err != nil {
		return nil, err
	}
	if _, err = enumValue(node["principal_type"], label+".principal_type", allowed...); err != nil {
		return nil, err
	}
	return node, nil
}

func validateSignature(value any, label, profileHash, expectedKey string) (map[string]any, error) {
	node, err := requireKeys(value, label, "key_id", "profile_id", "profile_sha256", "signature_base64url")
	if err != nil {
		return nil, err
	}
	keyID, err := textValue(node["key_id"], label+".key_id", 160)
	if err != nil || !keyIDPattern.MatchString(keyID) || keyID != expectedKey {
		return nil, fmt.Errorf("%s uses the wrong trust-root key", label)
	}
	if node["profile_id"] != signatureProfileID || node["profile_sha256"] != profileHash {
		return nil, fmt.Errorf("%s does not bind the signature profile", label)
	}
	if _, err = shaValue(node["profile_sha256"], label+".profile_sha256"); err != nil {
		return nil, err
	}
	if _, err = fixedBase64URL(node["signature_base64url"], label+".signature_base64url", 64); err != nil {
		return nil, err
	}
	return node, nil
}

func canonicalEqual(left, right any) bool {
	leftBytes, leftErr := canonicalJSON(left)
	rightBytes, rightErr := canonicalJSON(right)
	return leftErr == nil && rightErr == nil && bytes.Equal(leftBytes, rightBytes)
}

func cloneValue(value any) any {
	switch typed := value.(type) {
	case map[string]any:
		result := make(map[string]any, len(typed))
		for key, child := range typed {
			result[key] = cloneValue(child)
		}
		return result
	case []any:
		result := make([]any, len(typed))
		for index, child := range typed {
			result[index] = cloneValue(child)
		}
		return result
	default:
		return typed
	}
}

func sortedStrings(values []string) []string {
	result := append([]string(nil), values...)
	sort.Strings(result)
	return result
}

func containsString(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}

func decodeHexSHA(value string) ([]byte, error) {
	decoded, err := hex.DecodeString(value)
	if err != nil || len(decoded) != 32 || hex.EncodeToString(decoded) != value {
		return nil, fmt.Errorf("digest must be lowercase SHA-256 hex")
	}
	return decoded, nil
}
