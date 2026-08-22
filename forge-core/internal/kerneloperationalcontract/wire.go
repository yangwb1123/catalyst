package kerneloperationalcontract

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"reflect"
	"sort"
	"strconv"
	"unicode"
	"unicode/utf8"
)

func parseStrictJSON(data []byte, maximum int) (any, error) {
	if len(data) == 0 || len(data) > maximum {
		return nil, fmt.Errorf("JSON byte length must be 1..%d", maximum)
	}
	if !utf8.Valid(data) {
		return nil, fmt.Errorf("JSON is not valid UTF-8")
	}
	if err := validateSurrogateEscapes(data); err != nil {
		return nil, err
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

func validateSurrogateEscapes(data []byte) error {
	inString := false
	for index := 0; index < len(data); index++ {
		if data[index] == '"' {
			inString = !inString
			continue
		}
		if !inString || data[index] != '\\' {
			continue
		}
		if index+1 >= len(data) {
			return fmt.Errorf("truncated JSON escape")
		}
		if data[index+1] != 'u' {
			index++
			continue
		}
		code, ok := hexQuad(data, index+2)
		if !ok {
			return fmt.Errorf("invalid JSON Unicode escape")
		}
		if code >= 0xdc00 && code <= 0xdfff {
			return fmt.Errorf("unpaired low surrogate escape")
		}
		if code >= 0xd800 && code <= 0xdbff {
			low, paired := hexQuad(data, index+8)
			if index+7 >= len(data) || data[index+6] != '\\' || data[index+7] != 'u' ||
				!paired || low < 0xdc00 || low > 0xdfff {
				return fmt.Errorf("unpaired high surrogate escape")
			}
			index += 11
			continue
		}
		index += 5
	}
	return nil
}

func hexQuad(data []byte, start int) (uint16, bool) {
	if start+4 > len(data) {
		return 0, false
	}
	var value uint16
	for _, character := range data[start : start+4] {
		value <<= 4
		switch {
		case character >= '0' && character <= '9':
			value += uint16(character - '0')
		case character >= 'a' && character <= 'f':
			value += uint16(character-'a') + 10
		case character >= 'A' && character <= 'F':
			value += uint16(character-'A') + 10
		default:
			return 0, false
		}
	}
	return value, true
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
		if len(object) >= maxObjectFields {
			return nil, fmt.Errorf("object field count exceeds %d", maxObjectFields)
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
	if err := validateWireString(key); err != nil {
		return "", fmt.Errorf("object key %q: %w", key, err)
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
	if unicode.Is(unicode.Cc, value) || value == 0x2028 || value == 0x2029 {
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

func canonicalJSON(value any, maximum int) ([]byte, error) {
	buffer := bytes.NewBuffer(nil)
	if err := appendCanonical(buffer, value, 1); err != nil {
		return nil, err
	}
	if buffer.Len() > maximum {
		return nil, fmt.Errorf("canonical JSON exceeds %d bytes", maximum)
	}
	return buffer.Bytes(), nil
}

func appendCanonical(buffer *bytes.Buffer, value any, depth int) error {
	if depth > maxJSONDepth {
		return fmt.Errorf("JSON depth exceeds %d", maxJSONDepth)
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
	if len(object) > maxObjectFields {
		return fmt.Errorf("object field count exceeds %d", maxObjectFields)
	}
	keys := make([]string, 0, len(object))
	for key := range object {
		if !isASCIISnakeKey(key) || validateWireString(key) != nil {
			return fmt.Errorf("object key %q is invalid", key)
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

func appendJSONString(buffer *bytes.Buffer, value string) {
	buffer.WriteByte('"')
	for _, runeValue := range value {
		if runeValue == '"' || runeValue == '\\' {
			buffer.WriteByte('\\')
		}
		buffer.WriteRune(runeValue)
	}
	buffer.WriteByte('"')
}

func typedNode(value any, maximum int) (any, error) {
	if err := validateTypedValue(reflect.ValueOf(value), 1); err != nil {
		return nil, err
	}
	data, err := json.Marshal(value)
	if err != nil {
		return nil, fmt.Errorf("typed JSON encode: %w", err)
	}
	return parseStrictJSON(data, maximum)
}

func validateTypedValue(value reflect.Value, depth int) error {
	if depth > maxJSONDepth {
		return fmt.Errorf("typed value depth exceeds %d", maxJSONDepth)
	}
	if !value.IsValid() {
		return nil
	}
	switch value.Kind() {
	case reflect.Interface, reflect.Pointer:
		if value.IsNil() {
			return nil
		}
		return validateTypedValue(value.Elem(), depth)
	case reflect.String:
		return validateWireString(value.String())
	case reflect.Struct:
		for index := 0; index < value.NumField(); index++ {
			if err := validateTypedValue(value.Field(index), depth+1); err != nil {
				return err
			}
		}
	case reflect.Array, reflect.Slice:
		for index := 0; index < value.Len(); index++ {
			if err := validateTypedValue(value.Index(index), depth+1); err != nil {
				return err
			}
		}
	case reflect.Map:
		iterator := value.MapRange()
		for iterator.Next() {
			if iterator.Key().Kind() != reflect.String {
				return fmt.Errorf("typed JSON map key must be a string")
			}
			if err := validateWireString(iterator.Key().String()); err != nil {
				return err
			}
			if err := validateTypedValue(iterator.Value(), depth+1); err != nil {
				return err
			}
		}
	}
	return nil
}

// CanonicalJSON emits exact canonical JSON for one supported typed value.
func CanonicalJSON(value any) ([]byte, error) {
	node, err := typedNode(value, maxClosureBytes)
	if err != nil {
		return nil, err
	}
	return canonicalJSON(node, maxClosureBytes)
}

func decodeTypedExact(data []byte, maximum int, target any) error {
	node, err := parseStrictJSON(data, maximum)
	if err != nil {
		return err
	}
	canonical, err := canonicalJSON(node, maximum)
	if err != nil || !bytes.Equal(canonical, data) {
		return fmt.Errorf("input is not exact compact canonical JSON")
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		return fmt.Errorf("typed decode: %w", err)
	}
	typed, err := typedNode(target, maximum)
	if err != nil {
		return err
	}
	typedCanonical, err := canonicalJSON(typed, maximum)
	if err != nil || !bytes.Equal(typedCanonical, data) {
		return fmt.Errorf("input omits or changes an exact typed field")
	}
	return nil
}
