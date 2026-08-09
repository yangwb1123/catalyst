package cognitiveatomcontract

import (
	"bytes"
	"encoding/json"
	"fmt"
	"reflect"
	"sort"
	"strconv"
	"unicode/utf8"
)

const (
	maxAtomBytes   = 131072
	maxSetBytes    = 1048576
	maxDepth       = 16
	maxFields      = 64
	maxArrayItems  = 256
	maxStringBytes = 16384
)

func parseStrictJSONBounded(data []byte, maximum int) (any, error) {
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
	if token, err := decoder.Token(); err == nil {
		return nil, fmt.Errorf("trailing JSON token %v", token)
	}
	return value, nil
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
		switch value {
		case '{':
			return decodeObject(decoder, depth)
		case '[':
			return decodeArray(decoder, depth)
		default:
			return nil, fmt.Errorf("unexpected delimiter %q", value)
		}
	case json.Number:
		return parseInt64Number(value.String())
	case string:
		return value, validateString(value)
	case bool, nil:
		return value, nil
	default:
		return nil, fmt.Errorf("unsupported JSON token %T", token)
	}
}

func decodeObject(decoder *json.Decoder, depth int) (map[string]any, error) {
	object := make(map[string]any)
	for decoder.More() {
		if len(object) >= maxFields {
			return nil, fmt.Errorf("object field count exceeds %d", maxFields)
		}
		token, err := decoder.Token()
		if err != nil {
			return nil, err
		}
		key, ok := token.(string)
		if !ok || !isASCIISnakeKey(key) {
			return nil, fmt.Errorf("object key %q is not ASCII snake_case", key)
		}
		if _, exists := object[key]; exists {
			return nil, fmt.Errorf("duplicate JSON key %q", key)
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

func parseInt64Number(raw string) (int64, error) {
	if raw == "" || raw[0] == '+' || bytes.ContainsAny([]byte(raw), ".eE") {
		return 0, fmt.Errorf("number %q is not a canonical signed int64", raw)
	}
	value, err := strconv.ParseInt(raw, 10, 64)
	if err != nil || strconv.FormatInt(value, 10) != raw {
		return 0, fmt.Errorf("number %q is not a canonical signed int64", raw)
	}
	return value, nil
}

func validateString(value string) error {
	if !utf8.ValidString(value) {
		return fmt.Errorf("string is not valid UTF-8")
	}
	if len(value) > maxStringBytes {
		return fmt.Errorf("string byte length exceeds %d", maxStringBytes)
	}
	for _, runeValue := range value {
		if isForbiddenRune(runeValue) {
			return fmt.Errorf("string contains forbidden Unicode U+%04X", runeValue)
		}
	}
	return nil
}

func isForbiddenRune(value rune) bool {
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
		char := value[index]
		if (char < 'a' || char > 'z') && (char < '0' || char > '9') && char != '_' {
			return false
		}
	}
	return true
}

func canonicalJSON(value any) ([]byte, error) {
	buffer := bytes.NewBuffer(nil)
	if err := appendCanonical(buffer, value); err != nil {
		return nil, err
	}
	return buffer.Bytes(), nil
}

func appendCanonical(buffer *bytes.Buffer, value any) error {
	switch typed := value.(type) {
	case map[string]any:
		keys := make([]string, 0, len(typed))
		for key := range typed {
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
			if err := appendCanonical(buffer, typed[key]); err != nil {
				return err
			}
		}
		buffer.WriteByte('}')
	case []any:
		buffer.WriteByte('[')
		for index, child := range typed {
			if index > 0 {
				buffer.WriteByte(',')
			}
			if err := appendCanonical(buffer, child); err != nil {
				return err
			}
		}
		buffer.WriteByte(']')
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

func cloneNode(value any) any {
	switch typed := value.(type) {
	case map[string]any:
		copyObject := make(map[string]any, len(typed))
		for key, child := range typed {
			copyObject[key] = cloneNode(child)
		}
		return copyObject
	case []any:
		copyArray := make([]any, len(typed))
		for index, child := range typed {
			copyArray[index] = cloneNode(child)
		}
		return copyArray
	default:
		return value
	}
}

func encodeTypedJSON(value any) ([]byte, error) {
	if err := rejectInvalidUTF8(reflect.ValueOf(value)); err != nil {
		return nil, err
	}
	buffer := bytes.NewBuffer(nil)
	encoder := json.NewEncoder(buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, err
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, fmt.Errorf("typed JSON encoder did not terminate predictably")
	}
	return append([]byte(nil), encoded[:len(encoded)-1]...), nil
}

func rejectInvalidUTF8(value reflect.Value) error {
	if !value.IsValid() {
		return nil
	}
	switch value.Kind() {
	case reflect.Interface, reflect.Pointer:
		if value.IsNil() {
			return nil
		}
		return rejectInvalidUTF8(value.Elem())
	case reflect.String:
		if !utf8.ValidString(value.String()) {
			return fmt.Errorf("typed string is not valid UTF-8")
		}
	case reflect.Struct:
		for index := 0; index < value.NumField(); index++ {
			if err := rejectInvalidUTF8(value.Field(index)); err != nil {
				return err
			}
		}
	case reflect.Slice, reflect.Array:
		for index := 0; index < value.Len(); index++ {
			if err := rejectInvalidUTF8(value.Index(index)); err != nil {
				return err
			}
		}
	case reflect.Map:
		iterator := value.MapRange()
		for iterator.Next() {
			if err := rejectInvalidUTF8(iterator.Key()); err != nil {
				return err
			}
			if err := rejectInvalidUTF8(iterator.Value()); err != nil {
				return err
			}
		}
	}
	return nil
}
