package contextpackagecontract

import (
	"crypto/sha256"
	"encoding/hex"
)

const (
	requestDigestDomain    = "forgeos.context-package-build-request.v1\x00"
	cacheDigestDomain      = "forgeos.context-package-cache-key.v1\x00"
	contextDigestDomain    = "forgeos.context-package.v1\x00"
	snippetDigestDomain    = "forgeos.context-snippet.v1\x00"
	contentDigestDomain    = "forgeos.context-content.v1\x00"
	projectionDigestDomain = "forgeos.context-package-projection.v1\x00"
)

func domainDigest(domain string, payload []byte) string {
	hasher := sha256.New()
	hasher.Write([]byte(domain))
	hasher.Write(payload)
	return hex.EncodeToString(hasher.Sum(nil))
}

// RequestSHA256 binds the complete exact canonical request, including the
// tokenizer identity.
func RequestSHA256(request *BuildRequest) (string, error) {
	canonical, err := CanonicalRequestJSON(request)
	if err != nil {
		return "", err
	}
	return domainDigest(requestDigestDomain, canonical), nil
}

// CacheKeySHA256 is an independently domain-separated identity for the same
// complete exact request. There are no ambient cache-key inputs.
func CacheKeySHA256(request *BuildRequest) (string, error) {
	canonical, err := CanonicalRequestJSON(request)
	if err != nil {
		return "", err
	}
	return domainDigest(cacheDigestDomain, canonical), nil
}
