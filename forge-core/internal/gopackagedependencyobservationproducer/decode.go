package gopackagedependencyobservationproducer

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"strconv"
	"unicode"
	"unicode/utf8"
)

const (
	maxJSONDepth         = 16
	maxJSONObjectFields  = 64
	maxJSONArrayItems    = 65_536
	maxJSONStringBytes   = 16_384
	maxJSONStringScalars = 4_096
)

func DecodeProduction(raw []byte) (*Production, error) {
	if len(raw) == 0 || len(raw) > maxProductionBytes || !utf8.Valid(raw) {
		return nil, fmt.Errorf("production JSON is empty, oversized, or invalid UTF-8")
	}
	if err := validateJSONShape(raw); err != nil {
		return nil, fmt.Errorf("production JSON shape: %w", err)
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	var value ProductionPackage
	if err := decoder.Decode(&value); err != nil {
		return nil, fmt.Errorf("decode production JSON: %w", err)
	}
	if err := requireJSONEOF(decoder); err != nil {
		return nil, err
	}
	canonical, err := canonicalJSON(value, maxProductionBytes)
	if err != nil || !bytes.Equal(canonical, raw) {
		return nil, fmt.Errorf("production JSON is not exact canonical encoding")
	}
	return sealProduction(value)
}

func validateJSONShape(raw []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	if err := consumeJSONValue(decoder, 1); err != nil {
		return err
	}
	return requireJSONEOF(decoder)
}

func consumeJSONValue(decoder *json.Decoder, depth int) error {
	if depth > maxJSONDepth {
		return fmt.Errorf("depth exceeds %d", maxJSONDepth)
	}
	token, err := decoder.Token()
	if err != nil {
		return err
	}
	if delimiter, ok := token.(json.Delim); ok {
		switch delimiter {
		case '{':
			return consumeJSONObject(decoder, depth)
		case '[':
			return consumeJSONArray(decoder, depth)
		default:
			return fmt.Errorf("unexpected closing delimiter")
		}
	}
	return validateJSONScalar(token)
}

func consumeJSONObject(decoder *json.Decoder, depth int) error {
	seen, count := make(map[string]struct{}), 0
	for decoder.More() {
		keyToken, err := decoder.Token()
		key, ok := keyToken.(string)
		if err != nil || !ok || !validJSONKey(key) {
			return fmt.Errorf("invalid object key")
		}
		if _, exists := seen[key]; exists {
			return fmt.Errorf("duplicate object key %q", key)
		}
		seen[key], count = struct{}{}, count+1
		if count > maxJSONObjectFields {
			return fmt.Errorf("object exceeds %d fields", maxJSONObjectFields)
		}
		if err := consumeJSONValue(decoder, depth+1); err != nil {
			return err
		}
	}
	return requireDelimiter(decoder, '}')
}

func consumeJSONArray(decoder *json.Decoder, depth int) error {
	count := 0
	for decoder.More() {
		count++
		if count > maxJSONArrayItems {
			return fmt.Errorf("array exceeds %d items", maxJSONArrayItems)
		}
		if err := consumeJSONValue(decoder, depth+1); err != nil {
			return err
		}
	}
	return requireDelimiter(decoder, ']')
}

func validateJSONScalar(value any) error {
	switch typed := value.(type) {
	case nil, bool:
		return nil
	case string:
		if !boundedJSONString(typed) {
			return fmt.Errorf("string violates bounded text profile")
		}
		return nil
	case json.Number:
		if _, err := strconv.ParseInt(string(typed), 10, 64); err != nil {
			return fmt.Errorf("number is not a signed int64")
		}
		return nil
	default:
		return fmt.Errorf("unsupported JSON scalar")
	}
}

func boundedJSONString(value string) bool {
	if len(value) > maxJSONStringBytes || utf8.RuneCountInString(value) > maxJSONStringScalars {
		return false
	}
	for _, character := range value {
		if unicode.Is(unicode.Cc, character) || character == 0x061c ||
			character == 0x200e || character == 0x200f ||
			character == 0x2028 || character == 0x2029 ||
			character >= 0x202a && character <= 0x202e ||
			character >= 0x2066 && character <= 0x2069 {
			return false
		}
	}
	return true
}

func validJSONKey(value string) bool {
	if value == "" || !boundedJSONString(value) || value[0] < 'a' || value[0] > 'z' {
		return false
	}
	for _, character := range value {
		if character != '_' && (character < 'a' || character > 'z') &&
			(character < '0' || character > '9') {
			return false
		}
	}
	return true
}

func requireDelimiter(decoder *json.Decoder, expected json.Delim) error {
	token, err := decoder.Token()
	if err != nil || token != expected {
		return fmt.Errorf("JSON delimiter is malformed")
	}
	return nil
}

func requireJSONEOF(decoder *json.Decoder) error {
	if _, err := decoder.Token(); err != io.EOF {
		return fmt.Errorf("production JSON has trailing data")
	}
	return nil
}
