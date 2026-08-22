package outputbinding

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"strconv"
	"unicode/utf8"
)

const (
	maxJSONDepth    = 8
	maxObjectFields = 64
	maxArrayItems   = maxManifestItems
	maxWireString   = maxReferenceBytes
)

func parseStrictJSONObject(data []byte, maximum int) (map[string]any, error) {
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
	if token, trailingErr := decoder.Token(); trailingErr != io.EOF {
		if trailingErr == nil {
			return nil, fmt.Errorf("trailing JSON token %v", token)
		}
		return nil, fmt.Errorf("trailing JSON: %w", trailingErr)
	}
	object, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("JSON root must be an object")
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
	switch value := token.(type) {
	case json.Delim:
		return decodeJSONContainer(decoder, value, depth)
	case json.Number:
		return decodeCanonicalInt(value.String())
	case string:
		return value, validateWireText(value, true, maxWireString)
	case bool, nil:
		return value, nil
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
		if len(object) >= maxObjectFields {
			return nil, fmt.Errorf("object field count exceeds %d", maxObjectFields)
		}
		key, err := decodeJSONObjectKey(decoder, object)
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

func decodeJSONObjectKey(decoder *json.Decoder, object map[string]any) (string, error) {
	token, err := decoder.Token()
	if err != nil {
		return "", err
	}
	key, ok := token.(string)
	if !ok || !asciiSnakeKey(key) {
		return "", fmt.Errorf("object key %q is not ASCII snake_case", key)
	}
	if _, duplicate := object[key]; duplicate {
		return "", fmt.Errorf("duplicate JSON key %q", key)
	}
	return key, nil
}

func decodeJSONArray(decoder *json.Decoder, depth int) ([]any, error) {
	array := make([]any, 0)
	for decoder.More() {
		if len(array) >= maxArrayItems {
			return nil, fmt.Errorf("array item count exceeds %d", maxArrayItems)
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

func decodeCanonicalInt(raw string) (int64, error) {
	if raw == "" || raw[0] == '+' || bytes.ContainsAny([]byte(raw), ".eE") {
		return 0, fmt.Errorf("number %q is not a canonical signed integer", raw)
	}
	value, err := strconv.ParseInt(raw, 10, 64)
	if err != nil || strconv.FormatInt(value, 10) != raw {
		return 0, fmt.Errorf("number %q is not a canonical signed integer", raw)
	}
	return value, nil
}

func asciiSnakeKey(value string) bool {
	if value == "" || value[0] < 'a' || value[0] > 'z' {
		return false
	}
	for index := 1; index < len(value); index++ {
		character := value[index]
		if (character < 'a' || character > 'z') &&
			(character < '0' || character > '9') && character != '_' {
			return false
		}
	}
	return true
}
