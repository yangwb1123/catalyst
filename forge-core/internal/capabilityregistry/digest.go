package capabilityregistry

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
)

func digestDocument(domain string, document map[string]any, emptyFields ...string) (string, error) {
	preimage, ok := cloneJSON(document).(map[string]any)
	if !ok {
		return "", fmt.Errorf("digest document is not an object")
	}
	for _, field := range emptyFields {
		if _, exists := preimage[field]; !exists {
			return "", fmt.Errorf("digest field %q is absent", field)
		}
		preimage[field] = ""
	}
	encoded, err := canonicalJSON(preimage)
	if err != nil {
		return "", err
	}
	hasher := sha256.New()
	_, _ = hasher.Write([]byte(domain))
	_, _ = hasher.Write(encoded)
	return hex.EncodeToString(hasher.Sum(nil)), nil
}

func cloneJSON(value any) any {
	switch typed := value.(type) {
	case map[string]any:
		copyValue := make(map[string]any, len(typed))
		for key, child := range typed {
			copyValue[key] = cloneJSON(child)
		}
		return copyValue
	case []any:
		copyValue := make([]any, len(typed))
		for index, child := range typed {
			copyValue[index] = cloneJSON(child)
		}
		return copyValue
	default:
		return typed
	}
}

func requireDigest(document map[string]any, domain string, fields ...string) error {
	digest, err := digestDocument(domain, document, fields...)
	if err != nil {
		return err
	}
	for _, field := range fields {
		stored, ok := document[field].(string)
		if !ok {
			return fmt.Errorf("digest field %q must be a string", field)
		}
		if field[len(field)-3:] == "_id" {
			continue
		}
		if stored != digest {
			return fmt.Errorf("field %q does not match its domain-separated digest", field)
		}
	}
	return nil
}

func requirePrefixedIdentity(document map[string]any, domain, idField, digestField, prefix string) error {
	digest, err := digestDocument(domain, document, idField, digestField)
	if err != nil {
		return err
	}
	id, idOK := document[idField].(string)
	stored, digestOK := document[digestField].(string)
	if !idOK || !digestOK || id != prefix+digest || stored != digest {
		return fmt.Errorf("%s/%s identity does not match its domain-separated digest", idField, digestField)
	}
	return nil
}
