package planningownership

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"strconv"
	"unicode"
	"unicode/utf8"
)

func parseCanonicalObject(data []byte, maximum int) (map[string]any, error) {
	if len(data) == 0 || len(data) > maximum || !utf8.Valid(data) {
		return nil, fmt.Errorf("JSON input is empty, oversized, or invalid UTF-8")
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	value, err := decodeJSONNode(decoder, 1)
	if err != nil {
		return nil, err
	}
	if token, trailing := decoder.Token(); trailing != io.EOF {
		return nil, fmt.Errorf("trailing JSON token %v", token)
	}
	object, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("JSON root must be an object")
	}
	canonical, err := canonicalJSON(object)
	if err != nil || !bytes.Equal(canonical, data) {
		return nil, fmt.Errorf("JSON is not exact compact canonical form")
	}
	return object, nil
}

func decodeJSONNode(decoder *json.Decoder, depth int) (any, error) {
	if depth > maxJSONDepth {
		return nil, fmt.Errorf("JSON depth exceeds %d", maxJSONDepth)
	}
	token, err := decoder.Token()
	if err != nil {
		return nil, err
	}
	switch typed := token.(type) {
	case json.Delim:
		return decodeJSONContainer(decoder, typed, depth)
	case json.Number:
		return parseCanonicalInt(typed.String())
	case string:
		return typed, validateWireString(typed)
	case bool, nil:
		return typed, nil
	default:
		return nil, fmt.Errorf("unsupported JSON token %T", token)
	}
}

func decodeJSONContainer(decoder *json.Decoder, delimiter json.Delim, depth int) (any, error) {
	if delimiter == '{' {
		return decodeJSONObject(decoder, depth)
	}
	if delimiter == '[' {
		return decodeJSONArray(decoder, depth)
	}
	return nil, fmt.Errorf("unexpected JSON delimiter %q", delimiter)
}

func decodeJSONObject(decoder *json.Decoder, depth int) (map[string]any, error) {
	object := make(map[string]any)
	for decoder.More() {
		if len(object) >= maxJSONFields {
			return nil, fmt.Errorf("JSON object field count exceeds %d", maxJSONFields)
		}
		key, err := decodeJSONKey(decoder, object)
		if err != nil {
			return nil, err
		}
		child, err := decodeJSONNode(decoder, depth+1)
		if err != nil {
			return nil, fmt.Errorf("field %q: %w", key, err)
		}
		object[key] = child
	}
	if closing, err := decoder.Token(); err != nil || closing != json.Delim('}') {
		return nil, fmt.Errorf("JSON object is not closed")
	}
	return object, nil
}

func decodeJSONKey(decoder *json.Decoder, object map[string]any) (string, error) {
	token, err := decoder.Token()
	key, ok := token.(string)
	if err != nil || !ok || !isASCIISnakeKey(key) || validateWireString(key) != nil {
		return "", fmt.Errorf("invalid JSON object key")
	}
	if _, exists := object[key]; exists {
		return "", fmt.Errorf("duplicate JSON object key %q", key)
	}
	return key, nil
}

func decodeJSONArray(decoder *json.Decoder, depth int) ([]any, error) {
	result := make([]any, 0)
	for decoder.More() {
		if len(result) >= maxJSONItems {
			return nil, fmt.Errorf("JSON array item count exceeds %d", maxJSONItems)
		}
		child, err := decodeJSONNode(decoder, depth+1)
		if err != nil {
			return nil, fmt.Errorf("array item %d: %w", len(result), err)
		}
		result = append(result, child)
	}
	if closing, err := decoder.Token(); err != nil || closing != json.Delim(']') {
		return nil, fmt.Errorf("JSON array is not closed")
	}
	return result, nil
}

func parseCanonicalInt(raw string) (int64, error) {
	if raw == "" || raw[0] == '+' || bytes.ContainsAny([]byte(raw), ".eE") {
		return 0, fmt.Errorf("number %q is not a canonical signed int64", raw)
	}
	value, err := strconv.ParseInt(raw, 10, 64)
	if err != nil || strconv.FormatInt(value, 10) != raw {
		return 0, fmt.Errorf("number %q is not a canonical signed int64", raw)
	}
	return value, nil
}

func validateWireString(value string) error {
	if !utf8.ValidString(value) || len(value) > maxJSONStringBytes {
		return fmt.Errorf("string is invalid UTF-8 or oversized")
	}
	for _, character := range value {
		if forbiddenRune(character) {
			return fmt.Errorf("string contains forbidden Unicode U+%04X", character)
		}
	}
	return nil
}

func forbiddenRune(value rune) bool {
	if unicode.Is(unicode.Cc, value) || value == 0x7f || value == 0x2028 || value == 0x2029 {
		return true
	}
	return value == 0x061c || value == 0x200e || value == 0x200f ||
		(value >= 0x202a && value <= 0x202e) || (value >= 0x2066 && value <= 0x2069)
}

func isASCIISnakeKey(value string) bool {
	if value == "" || value[0] < 'a' || value[0] > 'z' {
		return false
	}
	for index := 1; index < len(value); index++ {
		character := value[index]
		if (character < 'a' || character > 'z') && (character < '0' || character > '9') && character != '_' {
			return false
		}
	}
	return true
}
