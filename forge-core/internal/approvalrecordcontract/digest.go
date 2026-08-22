package approvalrecordcontract

import (
	"crypto/sha256"
	"encoding/hex"
)

func domainDigest(domain string, payload []byte) string {
	hasher := sha256.New()
	hasher.Write([]byte(domain))
	hasher.Write(payload)
	return hex.EncodeToString(hasher.Sum(nil))
}

func digestNode(domain string, node map[string]any) (string, error) {
	canonical, err := canonicalJSON(node)
	if err != nil {
		return "", err
	}
	return domainDigest(domain, canonical), nil
}

func cloneNode(node map[string]any) map[string]any {
	result := make(map[string]any, len(node))
	for key, value := range node {
		result[key] = cloneValue(value)
	}
	return result
}

func cloneValue(value any) any {
	switch typed := value.(type) {
	case map[string]any:
		return cloneNode(typed)
	case []any:
		result := make([]any, len(typed))
		for index, item := range typed {
			result[index] = cloneValue(item)
		}
		return result
	default:
		return typed
	}
}
