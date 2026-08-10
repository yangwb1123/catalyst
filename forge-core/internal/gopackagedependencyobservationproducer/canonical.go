package gopackagedependencyobservationproducer

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
)

func canonicalJSON(value any, maxBytes int) ([]byte, error) {
	buffer := bytes.NewBuffer(nil)
	encoder := json.NewEncoder(buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, fmt.Errorf("encode canonical JSON: %w", err)
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, fmt.Errorf("canonical JSON encoder did not terminate predictably")
	}
	encoded = encoded[:len(encoded)-1]
	if len(encoded) > maxBytes {
		return nil, fmt.Errorf("canonical JSON exceeds %d bytes", maxBytes)
	}
	return append([]byte(nil), encoded...), nil
}

func domainDigest(domain string, encoded []byte) string {
	hasher := sha256.New()
	_, _ = hasher.Write([]byte(domain))
	_, _ = hasher.Write([]byte{0})
	_, _ = hasher.Write(encoded)
	return hex.EncodeToString(hasher.Sum(nil))
}
