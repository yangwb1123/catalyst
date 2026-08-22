package adrv2

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
)

func domainDigest(domain string, parts ...[]byte) string {
	hasher := sha256.New()
	_, _ = hasher.Write([]byte(domain))
	for _, part := range parts {
		_, _ = hasher.Write([]byte{0})
		_, _ = hasher.Write(part)
	}
	return hex.EncodeToString(hasher.Sum(nil))
}

func validateDigests(root map[string]any, body []byte, frontmatter *Frontmatter) error {
	wantBody := domainDigest(bodyDomain, body)
	if frontmatter.BodySHA256 != wantBody {
		return fmt.Errorf("body_sha256 mismatch: got %q want %q", frontmatter.BodySHA256, wantBody)
	}
	blanked := cloneJSON(root).(map[string]any)
	blanked["self_sha256"] = ""
	canonical, err := canonicalJSON(blanked)
	if err != nil {
		return err
	}
	wantSelf := domainDigest(selfDomain, canonical, body)
	if frontmatter.SelfSHA256 != wantSelf {
		return fmt.Errorf("self_sha256 mismatch: got %q want %q", frontmatter.SelfSHA256, wantSelf)
	}
	return nil
}

func cloneJSON(value any) any {
	switch typed := value.(type) {
	case map[string]any:
		result := make(map[string]any, len(typed))
		for key, child := range typed {
			result[key] = cloneJSON(child)
		}
		return result
	case []any:
		result := make([]any, len(typed))
		for index, child := range typed {
			result[index] = cloneJSON(child)
		}
		return result
	default:
		return typed
	}
}
