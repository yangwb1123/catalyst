package authenticatedadrlifecycleauthority

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
	maxJSONDepth    = 16
	maxObjectFields = 64
	maxArrayItems   = 256
	maxStringBytes  = 512 * 1024
)

func parseCanonicalJSON(data []byte, maximum int, label string) (any, error) {
	if len(data) == 0 || len(data) > maximum || !utf8.Valid(data) {
		return nil, fmt.Errorf("%s must be valid UTF-8 within 1..%d bytes", label, maximum)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	value, err := decodeNode(decoder, 1)
	if err != nil {
		return nil, fmt.Errorf("%s: %w", label, err)
	}
	if token, tailErr := decoder.Token(); tailErr != io.EOF {
		if tailErr == nil {
			return nil, fmt.Errorf("%s has trailing token %v", label, token)
		}
		return nil, fmt.Errorf("%s has trailing JSON: %w", label, tailErr)
	}
	canonical, err := canonicalJSON(value, maximum, label)
	if err != nil || !bytes.Equal(canonical, data) {
		return nil, fmt.Errorf("%s is not exact compact canonical JSON", label)
	}
	return value, nil
}

func decodeNode(decoder *json.Decoder, depth int) (any, error) {
	if depth > maxJSONDepth {
		return nil, fmt.Errorf("JSON depth exceeds %d", maxJSONDepth)
	}
	token, err := decoder.Token()
	if err != nil {
		return nil, err
	}
	switch value := token.(type) {
	case json.Delim:
		if value == '{' {
			return decodeObject(decoder, depth)
		}
		if value == '[' {
			return decodeArray(decoder, depth)
		}
		return nil, fmt.Errorf("unexpected delimiter %q", value)
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

func decodeObject(decoder *json.Decoder, depth int) (map[string]any, error) {
	object := make(map[string]any)
	for decoder.More() {
		if len(object) >= maxObjectFields {
			return nil, fmt.Errorf("object field count exceeds %d", maxObjectFields)
		}
		token, err := decoder.Token()
		if err != nil {
			return nil, err
		}
		key, ok := token.(string)
		if !ok || !asciiSnakeKey(key) {
			return nil, fmt.Errorf("object key is not ASCII snake_case")
		}
		if _, exists := object[key]; exists {
			return nil, fmt.Errorf("duplicate JSON key %q", key)
		}
		child, err := decodeNode(decoder, depth+1)
		if err != nil {
			return nil, fmt.Errorf("field %q: %w", key, err)
		}
		object[key] = child
	}
	if token, err := decoder.Token(); err != nil || token != json.Delim('}') {
		return nil, fmt.Errorf("object is not closed")
	}
	return object, nil
}

func decodeArray(decoder *json.Decoder, depth int) ([]any, error) {
	array := make([]any, 0)
	for decoder.More() {
		if len(array) >= maxArrayItems {
			return nil, fmt.Errorf("array item count exceeds %d", maxArrayItems)
		}
		child, err := decodeNode(decoder, depth+1)
		if err != nil {
			return nil, err
		}
		array = append(array, child)
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

func canonicalJSON(value any, maximum int, label string) ([]byte, error) {
	size, err := measureCanonical(value, 1, maximum)
	if err != nil {
		return nil, fmt.Errorf("%s: %w", label, err)
	}
	buffer := bytes.NewBuffer(make([]byte, 0, size))
	if err = appendCanonical(buffer, value, 1); err != nil {
		return nil, fmt.Errorf("%s: %w", label, err)
	}
	return buffer.Bytes(), nil
}

func measureCanonical(value any, depth, remaining int) (int, error) {
	if depth > maxJSONDepth {
		return 0, fmt.Errorf("JSON depth exceeds %d", maxJSONDepth)
	}
	switch typed := value.(type) {
	case map[string]any:
		return measureObject(typed, depth, remaining)
	case []any:
		return measureArray(typed, depth, remaining)
	case string:
		return measureString(typed, remaining)
	case int64:
		return boundedSize(len(strconv.FormatInt(typed, 10)), remaining)
	case bool:
		if typed {
			return boundedSize(4, remaining)
		}
		return boundedSize(5, remaining)
	case nil:
		return boundedSize(4, remaining)
	default:
		return 0, fmt.Errorf("cannot canonicalize %T", value)
	}
}

func measureObject(value map[string]any, depth, remaining int) (int, error) {
	if len(value) > maxObjectFields {
		return 0, fmt.Errorf("object field count exceeds %d", maxObjectFields)
	}
	total, err := boundedSize(2, remaining)
	if err != nil {
		return 0, err
	}
	for key, child := range value {
		if !asciiSnakeKey(key) {
			return 0, fmt.Errorf("object key %q is not ASCII snake_case", key)
		}
		keySize, err := measureString(key, remaining-total)
		if err != nil {
			return 0, err
		}
		childSize, err := measureCanonical(child, depth+1, remaining-total-keySize-1)
		if err != nil {
			return 0, err
		}
		total += keySize + 1 + childSize
	}
	if len(value) > 1 {
		total += len(value) - 1
	}
	return boundedSize(total, remaining)
}

func measureArray(value []any, depth, remaining int) (int, error) {
	if len(value) > maxArrayItems {
		return 0, fmt.Errorf("array item count exceeds %d", maxArrayItems)
	}
	total, err := boundedSize(2, remaining)
	if err != nil {
		return 0, err
	}
	for _, child := range value {
		size, childErr := measureCanonical(child, depth+1, remaining-total)
		if childErr != nil {
			return 0, childErr
		}
		total += size
	}
	if len(value) > 1 {
		total += len(value) - 1
	}
	return boundedSize(total, remaining)
}

func measureString(value string, remaining int) (int, error) {
	if err := validateWireString(value); err != nil {
		return 0, err
	}
	size := 2 + len(value) + bytes.Count([]byte(value), []byte{'"'}) +
		bytes.Count([]byte(value), []byte{'\\'})
	return boundedSize(size, remaining)
}

func boundedSize(size, maximum int) (int, error) {
	if size < 1 || size > maximum {
		return 0, fmt.Errorf("canonical JSON exceeds byte ceiling")
	}
	return size, nil
}

func appendCanonical(buffer *bytes.Buffer, value any, depth int) error {
	if depth > maxJSONDepth {
		return fmt.Errorf("JSON depth exceeds %d", maxJSONDepth)
	}
	switch typed := value.(type) {
	case map[string]any:
		return appendObject(buffer, typed, depth)
	case []any:
		return appendArray(buffer, typed, depth)
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

func appendObject(buffer *bytes.Buffer, object map[string]any, depth int) error {
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
		if err := appendCanonical(buffer, object[key], depth+1); err != nil {
			return err
		}
	}
	buffer.WriteByte('}')
	return nil
}

func appendArray(buffer *bytes.Buffer, array []any, depth int) error {
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

func appendJSONString(buffer *bytes.Buffer, value string) {
	buffer.WriteByte('"')
	for _, current := range value {
		if current == '"' || current == '\\' {
			buffer.WriteByte('\\')
		}
		buffer.WriteRune(current)
	}
	buffer.WriteByte('"')
}

func validateWireString(value string) error {
	if !utf8.ValidString(value) || len(value) > maxStringBytes {
		return fmt.Errorf("string must be valid UTF-8 and at most %d bytes", maxStringBytes)
	}
	for _, current := range value {
		if forbiddenRune(current) {
			return fmt.Errorf("string contains forbidden Unicode U+%04X", current)
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

func asciiSnakeKey(value string) bool {
	if value == "" || value[0] < 'a' || value[0] > 'z' {
		return false
	}
	for index := 1; index < len(value); index++ {
		current := value[index]
		if (current < 'a' || current > 'z') && (current < '0' || current > '9') && current != '_' {
			return false
		}
	}
	return true
}

func cloneValue(value any) any {
	switch typed := value.(type) {
	case map[string]any:
		result := make(map[string]any, len(typed))
		for key, child := range typed {
			result[key] = cloneValue(child)
		}
		return result
	case []any:
		result := make([]any, len(typed))
		for index, child := range typed {
			result[index] = cloneValue(child)
		}
		return result
	default:
		return typed
	}
}
