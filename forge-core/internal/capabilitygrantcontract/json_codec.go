// Package capabilitygrantcontract validates the authority-neutral
// CapabilityGrant v1 wire contract. It performs no I/O and grants no authority.
package capabilitygrantcontract

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"sort"
	"strconv"
	"unicode/utf8"
)

const (
	maxVocabularyBytes        = 128 * 1024
	maxGrantBytes             = 1024 * 1024
	maxAssessmentRequestBytes = 2 * 1024 * 1024
	maxAssessmentBytes        = 256 * 1024
	maxDepth                  = 16
	maxFields                 = 64
	maxArrayItems             = 256
	maxStringBytes            = 16384
)

func parseStrictJSON(data []byte, maximum int) (any, error) {
	if len(data) == 0 || len(data) > maximum {
		return nil, fmt.Errorf("JSON byte length must be 1..%d", maximum)
	}
	if !utf8.Valid(data) {
		return nil, fmt.Errorf("JSON is not valid UTF-8")
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	value, err := decodeNode(decoder, 1)
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

func validateCanonicalByteLimit(document map[string]any, maximum int, label string) error {
	size, err := measureCanonical(document, 1, maximum)
	if err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	if size > maximum {
		return fmt.Errorf("%s canonical byte length exceeds %d", label, maximum)
	}
	return nil
}

func decodeNode(decoder *json.Decoder, depth int) (any, error) {
	if depth > maxDepth {
		return nil, fmt.Errorf("JSON depth exceeds %d", maxDepth)
	}
	token, err := decoder.Token()
	if err != nil {
		return nil, err
	}
	switch value := token.(type) {
	case json.Delim:
		return decodeContainer(decoder, value, depth)
	case json.Number:
		return parseCanonicalInt(value.String())
	case string:
		return value, validateWireString(value)
	case bool, nil:
		return value, nil
	default:
		return nil, fmt.Errorf("unsupported JSON token %T", token)
	}
}

func decodeContainer(decoder *json.Decoder, delimiter json.Delim, depth int) (any, error) {
	switch delimiter {
	case '{':
		return decodeObject(decoder, depth)
	case '[':
		return decodeArray(decoder, depth)
	default:
		return nil, fmt.Errorf("unexpected delimiter %q", delimiter)
	}
}

func decodeObject(decoder *json.Decoder, depth int) (map[string]any, error) {
	object := make(map[string]any)
	for decoder.More() {
		if len(object) >= maxFields {
			return nil, fmt.Errorf("object field count exceeds %d", maxFields)
		}
		key, err := decodeObjectKey(decoder, object)
		if err != nil {
			return nil, err
		}
		value, err := decodeNode(decoder, depth+1)
		if err != nil {
			return nil, fmt.Errorf("field %q: %w", key, err)
		}
		object[key] = value
	}
	if token, err := decoder.Token(); err != nil || token != json.Delim('}') {
		return nil, fmt.Errorf("object is not closed")
	}
	return object, nil
}

func decodeObjectKey(decoder *json.Decoder, object map[string]any) (string, error) {
	token, err := decoder.Token()
	if err != nil {
		return "", err
	}
	key, ok := token.(string)
	if !ok || !isASCIISnakeKey(key) {
		return "", fmt.Errorf("object key %q is not ASCII snake_case", key)
	}
	if _, exists := object[key]; exists {
		return "", fmt.Errorf("duplicate JSON key %q", key)
	}
	return key, nil
}

func decodeArray(decoder *json.Decoder, depth int) ([]any, error) {
	array := make([]any, 0)
	for decoder.More() {
		if len(array) >= maxArrayItems {
			return nil, fmt.Errorf("array item count exceeds %d", maxArrayItems)
		}
		value, err := decodeNode(decoder, depth+1)
		if err != nil {
			return nil, fmt.Errorf("array item %d: %w", len(array), err)
		}
		array = append(array, value)
	}
	if token, err := decoder.Token(); err != nil || token != json.Delim(']') {
		return nil, fmt.Errorf("array is not closed")
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

func validateWireString(value string) error {
	if !utf8.ValidString(value) || len(value) > maxStringBytes {
		return fmt.Errorf("string must be valid UTF-8 and at most %d bytes", maxStringBytes)
	}
	for _, runeValue := range value {
		if forbiddenRune(runeValue) {
			return fmt.Errorf("string contains forbidden Unicode U+%04X", runeValue)
		}
	}
	return nil
}

func forbiddenRune(value rune) bool {
	if value <= 0x1f || value == 0x7f || value == 0x2028 || value == 0x2029 {
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
		if (character < 'a' || character > 'z') &&
			(character < '0' || character > '9') && character != '_' {
			return false
		}
	}
	return true
}

func canonicalJSON(value any) ([]byte, error) {
	size, err := measureCanonical(value, 1, int(^uint(0)>>1))
	if err != nil {
		return nil, err
	}
	buffer := bytes.NewBuffer(nil)
	buffer.Grow(size)
	if err := appendCanonical(buffer, value, 1); err != nil {
		return nil, err
	}
	return buffer.Bytes(), nil
}

func appendCanonical(buffer *bytes.Buffer, value any, depth int) error {
	if depth > maxDepth {
		return fmt.Errorf("JSON depth exceeds %d", maxDepth)
	}
	switch typed := value.(type) {
	case map[string]any:
		return appendCanonicalObject(buffer, typed, depth)
	case []any:
		return appendCanonicalArray(buffer, typed, depth)
	case string:
		if err := validateWireString(typed); err != nil {
			return err
		}
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

func appendCanonicalObject(buffer *bytes.Buffer, object map[string]any, depth int) error {
	if len(object) > maxFields {
		return fmt.Errorf("object field count exceeds %d", maxFields)
	}
	keys := make([]string, 0, len(object))
	for key := range object {
		if !isASCIISnakeKey(key) {
			return fmt.Errorf("object key %q is not ASCII snake_case", key)
		}
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
		if err := appendCanonical(buffer, object[key], depth+1); err != nil {
			return err
		}
	}
	buffer.WriteByte('}')
	return nil
}

func appendCanonicalArray(buffer *bytes.Buffer, array []any, depth int) error {
	if len(array) > maxArrayItems {
		return fmt.Errorf("array item count exceeds %d", maxArrayItems)
	}
	buffer.WriteByte('[')
	for index, child := range array {
		if index > 0 {
			buffer.WriteByte(',')
		}
		if err := appendCanonical(buffer, child, depth+1); err != nil {
			return err
		}
	}
	buffer.WriteByte(']')
	return nil
}

// appendJSONString deliberately avoids encoding/json's HTML escaping. The
// contract canonical form keeps valid Unicode scalars (including <, >, and &)
// verbatim and escapes only JSON syntax characters.
func appendJSONString(buffer *bytes.Buffer, value string) {
	buffer.WriteByte('"')
	for _, runeValue := range value {
		switch runeValue {
		case '"', '\\':
			buffer.WriteByte('\\')
			buffer.WriteRune(runeValue)
		default:
			buffer.WriteRune(runeValue)
		}
	}
	buffer.WriteByte('"')
}

func requireKeys(object map[string]any, keys ...string) error {
	if len(object) != len(keys) {
		return fmt.Errorf("fields mismatch")
	}
	for _, key := range keys {
		if _, ok := object[key]; !ok {
			return fmt.Errorf("missing field %q", key)
		}
	}
	return nil
}
