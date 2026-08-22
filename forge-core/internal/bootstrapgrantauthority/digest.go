package bootstrapgrantauthority

import (
	"crypto/sha256"
	"fmt"
)

func digestNode(domain []byte, value any, maximum int, label string) (string, error) {
	canonical, err := canonicalJSON(value)
	if err != nil {
		return "", err
	}
	if len(canonical) > maximum {
		return "", fmt.Errorf("%s canonical bytes exceed %d", label, maximum)
	}
	hash := sha256.New()
	_, _ = hash.Write(domain)
	_, _ = hash.Write(canonical)
	return fmt.Sprintf("%x", hash.Sum(nil)), nil
}

func selfDigest(domain []byte, document map[string]any, digestField string,
	maximum int, label string, signed bool) (string, error) {
	if _, err := digestNode(nil, document, maximum, label); err != nil {
		return "", err
	}
	if _, ok := document[digestField]; !ok {
		return "", fmt.Errorf("%s lacks self-digest field %s", label, digestField)
	}
	preimage, ok := cloneNode(document).(map[string]any)
	if !ok {
		return "", fmt.Errorf("%s cannot be cloned", label)
	}
	preimage[digestField] = ""
	if signed {
		signature, err := objectValue(preimage, "signature")
		if err != nil {
			return "", fmt.Errorf("%s: %w", label, err)
		}
		signature["signature_base64url"] = ""
	}
	return digestNode(domain, preimage, maximum, label)
}

func cloneNode(value any) any {
	switch typed := value.(type) {
	case map[string]any:
		copyValue := make(map[string]any, len(typed))
		for key, child := range typed {
			copyValue[key] = cloneNode(child)
		}
		return copyValue
	case []any:
		copyValue := make([]any, len(typed))
		for index, child := range typed {
			copyValue[index] = cloneNode(child)
		}
		return copyValue
	default:
		return value
	}
}
