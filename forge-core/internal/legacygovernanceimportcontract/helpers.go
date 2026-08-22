package legacygovernanceimportcontract

import (
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"fmt"
	"regexp"
)

var identifierPattern = regexp.MustCompile(`^[A-Za-z0-9][A-Za-z0-9._:/@+-]{0,159}$`)

func shaBytes(raw []byte) string {
	sum := sha256.Sum256(raw)
	return hex.EncodeToString(sum[:])
}

func digestValue(domain []byte, value any, maximum int, label string) (string, error) {
	encoded, err := canonicalJSON(value, maximum, label)
	if err != nil {
		return "", err
	}
	hash := sha256.New()
	_, _ = hash.Write(domain)
	_, _ = hash.Write(encoded)
	return hex.EncodeToString(hash.Sum(nil)), nil
}

func selfDigest(domain []byte, value map[string]any, field string,
	maximum int, label string) (string, error) {
	clone := make(map[string]any, len(value))
	for key, child := range value {
		clone[key] = child
	}
	clone[field] = ""
	return digestValue(domain, clone, maximum, label+" digest preimage")
}

func encodeBase64URL(raw []byte) string {
	return base64.RawURLEncoding.EncodeToString(raw)
}

func decodeBase64URL(value any, label string) ([]byte, error) {
	text, ok := value.(string)
	if !ok {
		return nil, fmt.Errorf("%s must be base64url text", label)
	}
	raw, err := base64.RawURLEncoding.Strict().DecodeString(text)
	if err != nil || encodeBase64URL(raw) != text {
		return nil, fmt.Errorf("%s is not canonical unpadded base64url", label)
	}
	return raw, nil
}

func requireDigest(value any, label string) (string, error) {
	text, ok := value.(string)
	if !ok || len(text) != 64 {
		return "", fmt.Errorf("%s must be lowercase SHA-256 hex", label)
	}
	raw, err := hex.DecodeString(text)
	if err != nil || len(raw) != 32 || hex.EncodeToString(raw) != text {
		return "", fmt.Errorf("%s must be lowercase SHA-256 hex", label)
	}
	return text, nil
}

func exactFields(value any, fields []string, label string) (map[string]any, error) {
	object, ok := value.(map[string]any)
	if !ok || len(object) != len(fields) {
		return nil, fmt.Errorf("%s does not have its exact field set", label)
	}
	for _, field := range fields {
		if _, exists := object[field]; !exists {
			return nil, fmt.Errorf("%s lacks field %q", label, field)
		}
	}
	return object, nil
}

func stringValue(object map[string]any, field, label string, maximum int) (string, error) {
	value, ok := object[field].(string)
	if !ok || validateWireString(value, maximum, true) != nil {
		return "", fmt.Errorf("%s.%s is not bounded nonempty text", label, field)
	}
	return value, nil
}
