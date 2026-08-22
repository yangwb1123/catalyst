package approvalcontext

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"unicode"
	"unicode/utf8"
)

func CanonicalContextJSON(value Context) ([]byte, error) {
	if err := ValidateContext(value); err != nil {
		return nil, err
	}
	return marshalBounded(value)
}

func ContextSHA256(value Context) (string, error) {
	data, err := CanonicalContextJSON(value)
	if err != nil {
		return "", err
	}
	digest := sha256.Sum256(append([]byte(ContextDomain), data...))
	return hex.EncodeToString(digest[:]), nil
}

func DecodeCanonicalContext(data []byte) (Context, error) {
	var value Context
	if err := decodeCanonical(data, &value); err != nil {
		return Context{}, fmt.Errorf("approval context: %w", err)
	}
	canonical, err := CanonicalContextJSON(value)
	if err != nil {
		return Context{}, err
	}
	if !bytes.Equal(data, canonical) {
		return Context{}, fmt.Errorf("approval context is not exact canonical JSON")
	}
	return value, nil
}

func CanonicalMarkerJSON(value PositiveMarker) ([]byte, error) {
	if err := ValidatePositiveMarker(value); err != nil {
		return nil, err
	}
	return marshalBounded(value)
}

func DecodeCanonicalMarker(data []byte) (PositiveMarker, error) {
	var value PositiveMarker
	if err := decodeCanonical(data, &value); err != nil {
		return PositiveMarker{}, fmt.Errorf("positive approval marker: %w", err)
	}
	canonical, err := CanonicalMarkerJSON(value)
	if err != nil {
		return PositiveMarker{}, err
	}
	if !bytes.Equal(data, canonical) {
		return PositiveMarker{}, fmt.Errorf("positive approval marker is not exact canonical JSON")
	}
	return value, nil
}

func marshalBounded(value any) ([]byte, error) {
	data, err := json.Marshal(value)
	if err != nil {
		return nil, fmt.Errorf("encode canonical JSON: %w", err)
	}
	if len(data) == 0 || len(data) > maxWireBytes || !utf8.Valid(data) {
		return nil, fmt.Errorf("canonical JSON exceeds the wire boundary")
	}
	return data, nil
}

func decodeCanonical(data []byte, target any) error {
	if len(data) == 0 || len(data) > maxWireBytes || !utf8.Valid(data) {
		return fmt.Errorf("wire bytes are empty, oversized, or invalid UTF-8")
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		return fmt.Errorf("decode strict JSON: %w", err)
	}
	if decoder.More() {
		return fmt.Errorf("wire has trailing JSON")
	}
	return nil
}

func validDigest(value string) bool {
	if len(value) != sha256.Size*2 {
		return false
	}
	for _, character := range value {
		if !(character >= '0' && character <= '9') && !(character >= 'a' && character <= 'f') {
			return false
		}
	}
	return true
}

func validScalar(value string, max int) bool {
	if value == "" || len([]byte(value)) > max || !utf8.ValidString(value) {
		return false
	}
	for _, character := range value {
		if unicode.IsControl(character) {
			return false
		}
	}
	return true
}
