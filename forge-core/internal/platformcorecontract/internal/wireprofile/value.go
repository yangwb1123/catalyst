package wireprofile

import (
	"bytes"
	"encoding/json"
	"fmt"
)

// TypedCanonical captures and canonicalizes one typed value.
func TypedCanonical(value any, maximum int) ([]byte, error) {
	return encodeTypedCanonical(value, maximum)
}

// DecodeTypedCanonical decodes exact canonical bytes into one strict typed target.
func DecodeTypedCanonical(data []byte, maximum int, target any) error {
	node, err := ParseStrictJSON(data, maximum)
	if err != nil {
		return err
	}
	canonical, err := CanonicalJSON(node, maximum)
	if err != nil {
		return err
	}
	if !bytes.Equal(data, canonical) {
		return fmt.Errorf("input is not exact compact canonical JSON")
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		return fmt.Errorf("typed JSON decode failed: %w", err)
	}
	return nil
}

// NormalizeJSONNumbers converts strict decoder numbers to signed int64 values.
func NormalizeJSONNumbers(value any) (any, error) {
	switch typed := value.(type) {
	case json.Number:
		return parseCanonicalInteger(typed.String())
	case []any:
		for index, child := range typed {
			normalized, err := NormalizeJSONNumbers(child)
			if err != nil {
				return nil, err
			}
			typed[index] = normalized
		}
	case map[string]any:
		for key, child := range typed {
			normalized, err := NormalizeJSONNumbers(child)
			if err != nil {
				return nil, err
			}
			typed[key] = normalized
		}
	}
	return value, nil
}
