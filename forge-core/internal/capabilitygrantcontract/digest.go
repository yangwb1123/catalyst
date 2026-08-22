package capabilitygrantcontract

import (
	"crypto/sha256"
	"encoding/hex"
)

const (
	vocabularyDigestDomain = "forgeos.governance.effect-vocabulary.v1\x00"
	grantDigestDomain      = "forgeos.capability-grant.v1\x00"
	actionDigestDomain     = "forgeos.capability-requested-action.v1\x00"
	requestDigestDomain    = "forgeos.capability-grant-declared-assessment-request.v1\x00"
	assessmentDigestDomain = "forgeos.capability-grant-declared-assessment.v1\x00"
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
