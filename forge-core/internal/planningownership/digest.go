package planningownership

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
)

func rawSHA256(value []byte) string {
	digest := sha256.Sum256(value)
	return hex.EncodeToString(digest[:])
}

func documentDigest(domain string, document map[string]any, field string) (string, error) {
	copy := cloneObject(document)
	if _, exists := copy[field]; !exists {
		return "", fmt.Errorf("digest field %q is absent", field)
	}
	copy[field] = ""
	encoded, err := canonicalJSON(copy)
	if err != nil {
		return "", err
	}
	preimage := append([]byte(domain), encoded...)
	return rawSHA256(preimage), nil
}

func requireDigest(domain string, document map[string]any, field string) error {
	actual, ok := document[field].(string)
	if !ok || !validHash(actual) {
		return fmt.Errorf("field %q must be lowercase SHA-256", field)
	}
	expected, err := documentDigest(domain, document, field)
	if err != nil || actual != expected {
		return fmt.Errorf("field %q digest mismatch", field)
	}
	return nil
}
