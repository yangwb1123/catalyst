package adrv2

import (
	"bytes"
	"encoding/json"
	"fmt"
	"sort"
	"strconv"
	"unicode"
	"unicode/utf8"
)

const maxJSONDepth = 16
const maxArrayItems = 64
const maxObjectFields = 64

func parseCanonicalJSON(data []byte) (map[string]any, error) {
	if len(data) == 0 || len(data) > MaxFrontmatter || !utf8.Valid(data) {
		return nil, fmt.Errorf("frontmatter JSON must be valid UTF-8 within 1..%d bytes", MaxFrontmatter)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	value, err := decodeJSONNode(decoder, 1)
	if err != nil {
		return nil, err
	}
	if token, err := decoder.Token(); err == nil {
		return nil, fmt.Errorf("trailing JSON token %v", token)
	}
	root, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("frontmatter JSON root must be an object")
	}
	canonical, err := canonicalJSON(root)
	if err != nil || !bytes.Equal(data, canonical) {
		return nil, fmt.Errorf("frontmatter must be one exact compact canonical JSON line")
	}
	return root, nil
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
		return parseCanonicalInt(value.String())
	case string:
		return value, validateJSONText(value)
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
		keyToken, err := decoder.Token()
		key, ok := keyToken.(string)
		if err != nil || !ok || !asciiSnakeKey(key) {
			return nil, fmt.Errorf("object key is not ASCII snake_case")
		}
		if _, duplicate := object[key]; duplicate {
			return nil, fmt.Errorf("duplicate JSON key %q", key)
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

func validateJSONText(value string) error {
	if !utf8.ValidString(value) || len(value) > 4096 {
		return fmt.Errorf("JSON string must be valid UTF-8 within 4096 bytes")
	}
	for _, character := range value {
		if forbiddenRune(character) {
			return fmt.Errorf("JSON string contains forbidden Unicode U+%04X", character)
		}
	}
	return nil
}

func forbiddenRune(value rune) bool {
	if unicode.Is(unicode.Cc, value) || value == 0x2028 || value == 0x2029 {
		return true
	}
	return value == 0x061c || value == 0x200e || value == 0x200f ||
		(value >= 0x202a && value <= 0x202e) || (value >= 0x2066 && value <= 0x2069)
}

func asciiSnakeKey(value string) bool {
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

func canonicalJSON(value any) ([]byte, error) {
	buffer := bytes.NewBuffer(nil)
	if err := appendCanonicalJSON(buffer, value); err != nil {
		return nil, err
	}
	return buffer.Bytes(), nil
}

func appendCanonicalJSON(buffer *bytes.Buffer, value any) error {
	switch typed := value.(type) {
	case map[string]any:
		return appendCanonicalObject(buffer, typed)
	case []any:
		return appendCanonicalArray(buffer, typed)
	case string:
		appendJSONString(buffer, typed)
	case int64:
		buffer.WriteString(strconv.FormatInt(typed, 10))
	case bool:
		buffer.WriteString(strconv.FormatBool(typed))
	case nil:
		buffer.WriteString("null")
	default:
		return fmt.Errorf("cannot canonicalize %T", value)
	}
	return nil
}

func appendCanonicalObject(buffer *bytes.Buffer, object map[string]any) error {
	keys := make([]string, 0, len(object))
	for key := range object {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	buffer.WriteByte('{')
	for index, key := range keys {
		if index > 0 {
			buffer.WriteByte(',')
		}
		appendJSONString(buffer, key)
		buffer.WriteByte(':')
		if err := appendCanonicalJSON(buffer, object[key]); err != nil {
			return err
		}
	}
	buffer.WriteByte('}')
	return nil
}

func appendCanonicalArray(buffer *bytes.Buffer, values []any) error {
	buffer.WriteByte('[')
	for index, value := range values {
		if index > 0 {
			buffer.WriteByte(',')
		}
		if err := appendCanonicalJSON(buffer, value); err != nil {
			return err
		}
	}
	buffer.WriteByte(']')
	return nil
}

func appendJSONString(buffer *bytes.Buffer, value string) {
	buffer.WriteByte('"')
	for _, character := range value {
		if character == '"' || character == '\\' {
			buffer.WriteByte('\\')
		}
		buffer.WriteRune(character)
	}
	buffer.WriteByte('"')
}
