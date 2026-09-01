package wireprofile

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"strconv"
	"unicode/utf8"
)

// ParseStrictJSON decodes one bounded JSON value with duplicate rejection.
func ParseStrictJSON(data []byte, maximum int) (any, error) {
	if len(data) == 0 || len(data) > maximum {
		return nil, fmt.Errorf("JSON byte length must be 1..%d", maximum)
	}
	if !utf8.Valid(data) {
		return nil, fmt.Errorf("JSON is not valid UTF-8")
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	value, err := decodeJSONNode(decoder, 1)
	if err != nil {
		return nil, err
	}
	if token, err := decoder.Token(); err != io.EOF {
		if err == nil {
			return nil, fmt.Errorf("trailing JSON token %v", token)
		}
		return nil, fmt.Errorf("trailing JSON: %w", err)
	}
	return value, nil
}

func decodeJSONNode(decoder *json.Decoder, depth int) (any, error) {
	if depth > maxJSONDepth {
		return nil, fmt.Errorf("JSON depth exceeds %d", maxJSONDepth)
	}
	token, err := decoder.Token()
	if err != nil {
		return nil, err
	}
	switch value := token.(type) {
	case json.Delim:
		return decodeJSONContainer(decoder, value, depth)
	case json.Number:
		return parseCanonicalInteger(value.String())
	case string:
		return value, ValidateText(value, "JSON string", maxStringBytes, false)
	case bool, nil:
		return value, nil
	default:
		return nil, fmt.Errorf("unsupported JSON token %T", token)
	}
}

func decodeJSONContainer(decoder *json.Decoder, delimiter json.Delim, depth int) (any, error) {
	switch delimiter {
	case '{':
		return decodeJSONObject(decoder, depth)
	case '[':
		return decodeJSONArray(decoder, depth)
	default:
		return nil, fmt.Errorf("unexpected JSON delimiter %q", delimiter)
	}
}

func decodeJSONObject(decoder *json.Decoder, depth int) (map[string]any, error) {
	object := make(map[string]any)
	for decoder.More() {
		if len(object) >= maxObjectFields {
			return nil, fmt.Errorf("JSON object exceeds %d fields", maxObjectFields)
		}
		key, err := decodeJSONKey(decoder, object)
		if err != nil {
			return nil, err
		}
		value, err := decodeJSONNode(decoder, depth+1)
		if err != nil {
			return nil, fmt.Errorf("field %q: %w", key, err)
		}
		object[key] = value
	}
	if token, err := decoder.Token(); err != nil || token != json.Delim('}') {
		return nil, fmt.Errorf("JSON object is not closed")
	}
	return object, nil
}

func decodeJSONKey(decoder *json.Decoder, object map[string]any) (string, error) {
	token, err := decoder.Token()
	if err != nil {
		return "", err
	}
	key, ok := token.(string)
	if !ok {
		return "", fmt.Errorf("JSON object key is not a string")
	}
	if err := ValidateText(key, "JSON object key", maxStringBytes, true); err != nil {
		return "", err
	}
	if _, exists := object[key]; exists {
		return "", fmt.Errorf("duplicate JSON key %q", key)
	}
	return key, nil
}

func decodeJSONArray(decoder *json.Decoder, depth int) ([]any, error) {
	array := make([]any, 0)
	for decoder.More() {
		if len(array) >= maxArrayItems {
			return nil, fmt.Errorf("JSON array exceeds %d items", maxArrayItems)
		}
		value, err := decodeJSONNode(decoder, depth+1)
		if err != nil {
			return nil, fmt.Errorf("array item %d: %w", len(array), err)
		}
		array = append(array, value)
	}
	if token, err := decoder.Token(); err != nil || token != json.Delim(']') {
		return nil, fmt.Errorf("JSON array is not closed")
	}
	return array, nil
}

func parseCanonicalInteger(raw string) (int64, error) {
	if raw == "" || raw[0] == '+' || bytes.ContainsAny([]byte(raw), ".eE") {
		return 0, fmt.Errorf("number %q is not a canonical signed int64", raw)
	}
	value, err := strconv.ParseInt(raw, 10, 64)
	if err != nil || strconv.FormatInt(value, 10) != raw {
		return 0, fmt.Errorf("number %q is not a canonical signed int64", raw)
	}
	return value, nil
}
