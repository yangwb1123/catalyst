package goimpactprescan

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
	maxJSONDepth        = 16
	maxJSONObjectFields = 64
	maxJSONArrayItems   = 65_536
	maxJSONStringBytes  = 22_369_622
)

func validateEnvelopeJSONShape(raw []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	if err := consumeJSONValue(decoder, 1); err != nil {
		return err
	}
	if _, err := decoder.Token(); err != io.EOF {
		return fmt.Errorf("JSON document has trailing data")
	}
	return nil
}

func consumeJSONValue(decoder *json.Decoder, depth int) error {
	if depth > maxJSONDepth {
		return fmt.Errorf("JSON depth exceeds %d", maxJSONDepth)
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
			return fmt.Errorf("unexpected JSON closing delimiter")
		}
	}
	return validateJSONScalar(token)
}

func consumeJSONObject(decoder *json.Decoder, depth int) error {
	seen, count := make(map[string]struct{}), 0
	for decoder.More() {
		token, err := decoder.Token()
		key, ok := token.(string)
		if err != nil || !ok || !validJSONKey(key) {
			return fmt.Errorf("invalid JSON object key")
		}
		if _, exists := seen[key]; exists {
			return fmt.Errorf("duplicate JSON object key")
		}
		seen[key], count = struct{}{}, count+1
		if count > maxJSONObjectFields {
			return fmt.Errorf("JSON object exceeds %d fields", maxJSONObjectFields)
		}
		if err := consumeJSONValue(decoder, depth+1); err != nil {
			return err
		}
	}
	return requireJSONDelimiter(decoder, '}')
}

func consumeJSONArray(decoder *json.Decoder, depth int) error {
	count := 0
	for decoder.More() {
		count++
		if count > maxJSONArrayItems {
			return fmt.Errorf("JSON array exceeds %d items", maxJSONArrayItems)
		}
		if err := consumeJSONValue(decoder, depth+1); err != nil {
			return err
		}
	}
	return requireJSONDelimiter(decoder, ']')
}

func validateJSONScalar(value any) error {
	switch typed := value.(type) {
	case nil, bool:
		return nil
	case string:
		if !boundedJSONString(typed) {
			return fmt.Errorf("JSON string violates bounded text profile")
		}
		return nil
	case json.Number:
		if _, err := strconv.ParseInt(string(typed), 10, 64); err != nil {
			return fmt.Errorf("JSON number is not a signed int64")
		}
		return nil
	default:
		return fmt.Errorf("unsupported JSON scalar")
	}
}

func boundedJSONString(value string) bool {
	if !utf8.ValidString(value) || len(value) > maxJSONStringBytes {
		return false
	}
	for _, character := range value {
		if unicode.Is(unicode.Cc, character) || forbiddenDirectional(character) {
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

func requireJSONDelimiter(decoder *json.Decoder, expected json.Delim) error {
	token, err := decoder.Token()
	if err != nil || token != expected {
		return fmt.Errorf("JSON delimiter is malformed")
	}
	return nil
}
