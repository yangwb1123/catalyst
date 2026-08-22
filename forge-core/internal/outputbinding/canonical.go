package outputbinding

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
)

// SHA256 returns the lowercase, ordinary SHA-256 of exact bytes. Self-digests
// use separate, private domain-separated helpers.
func SHA256(value []byte) string {
	digest := sha256.Sum256(value)
	return hex.EncodeToString(digest[:])
}

func domainDigest(domain string, value []byte) string {
	hasher := sha256.New()
	_, _ = hasher.Write([]byte(domain))
	_, _ = hasher.Write(value)
	return hex.EncodeToString(hasher.Sum(nil))
}

func canonicalJSON(value any, maximum int) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, fmt.Errorf("output binding: encode canonical JSON: %w", err)
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, fmt.Errorf("output binding: canonical encoder terminated unexpectedly")
	}
	encoded = encoded[:len(encoded)-1]
	if len(encoded) == 0 || len(encoded) > maximum {
		return nil, fmt.Errorf("output binding: canonical JSON exceeds %d bytes", maximum)
	}
	return append([]byte(nil), encoded...), nil
}

func decodeExact(data []byte, maximum int, target any,
	validate func() error, encode func() ([]byte, error)) error {
	if _, err := parseStrictJSONObject(data, maximum); err != nil {
		return fmt.Errorf("output binding: decode canonical JSON: %w", err)
	}
	if err := json.Unmarshal(data, target); err != nil {
		return fmt.Errorf("output binding: decode typed JSON: %w", err)
	}
	if err := validate(); err != nil {
		return err
	}
	canonical, err := encode()
	if err != nil {
		return err
	}
	if !bytes.Equal(data, canonical) {
		return fmt.Errorf("output binding: input is not exact compact canonical JSON")
	}
	return nil
}
