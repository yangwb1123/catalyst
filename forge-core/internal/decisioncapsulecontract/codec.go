package decisioncapsulecontract

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"sort"
	"strconv"
	"unicode"
	"unicode/utf8"
)

// CanonicalJSON emits exact canonical JSON under the local 28 MiB outer ceiling.
func CanonicalJSON(value any) ([]byte, error) {
	return canonicalBytes(value, maxClosureBytes)
}

func canonicalBytes(value any, maximum int) ([]byte, error) {
	measured, err := measureTypedJSON(value, maximum)
	if err != nil {
		return nil, err
	}
	raw, err := encodeTypedJSON(value, maximum)
	if err != nil {
		return nil, err
	}
	if len(raw) != measured {
		return nil, fmt.Errorf("typed JSON measured %d bytes but encoded %d", measured, len(raw))
	}
	return canonicalizeEncodedJSON(raw, maximum)
}

func encodeTypedJSON(value any, maximum int) ([]byte, error) {
	buffer := bytes.NewBuffer(make([]byte, 0, min(maximum+1, 64*1024)))
	encoder := json.NewEncoder(buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, fmt.Errorf("canonical JSON byte length exceeds %d: %w", maximum, err)
	}
	raw := buffer.Bytes()
	if len(raw) == 0 || raw[len(raw)-1] != '\n' {
		return nil, fmt.Errorf("typed JSON encoder did not terminate one value")
	}
	raw = raw[:len(raw)-1]
	if len(raw) == 0 || len(raw) > maximum {
		return nil, fmt.Errorf("canonical JSON byte length must be 1..%d", maximum)
	}
	return append([]byte(nil), raw...), nil
}

func canonicalizeEncodedJSON(raw []byte, maximum int) ([]byte, error) {
	if len(raw) == 0 || len(raw) > maximum {
		return nil, fmt.Errorf("JSON byte length must be 1..%d", maximum)
	}
	if !utf8.Valid(raw) {
		return nil, fmt.Errorf("JSON is not valid UTF-8")
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	canonical, err := parseCanonicalValue(decoder, 1)
	if err != nil {
		return nil, err
	}
	if err := requireEOF(decoder); err != nil {
		return nil, err
	}
	if len(canonical) == 0 || len(canonical) > maximum {
		return nil, fmt.Errorf("canonical JSON byte length must be 1..%d", maximum)
	}
	return canonical, nil
}

func parseCanonicalValue(decoder *json.Decoder, depth int) ([]byte, error) {
	if depth > maxDepth {
		return nil, fmt.Errorf("JSON depth exceeds %d", maxDepth)
	}
	token, err := decoder.Token()
	if err != nil {
		return nil, fmt.Errorf("canonical JSON decode: %w", err)
	}
	switch value := token.(type) {
	case json.Delim:
		if value == '{' {
			return parseCanonicalObject(decoder, depth)
		}
		if value == '[' {
			return parseCanonicalArray(decoder, depth)
		}
		return nil, fmt.Errorf("unexpected JSON delimiter %q", value)
	case string:
		if err := validateString(value); err != nil {
			return nil, err
		}
		buffer := bytes.NewBuffer(nil)
		appendCanonicalString(buffer, value)
		return buffer.Bytes(), nil
	case json.Number:
		number, err := strconv.ParseInt(value.String(), 10, 64)
		if err != nil || strconv.FormatInt(number, 10) != value.String() {
			return nil, fmt.Errorf("number %q is not a canonical signed int64", value)
		}
		return []byte(value.String()), nil
	case bool:
		return []byte(strconv.FormatBool(value)), nil
	case nil:
		return []byte("null"), nil
	default:
		return nil, fmt.Errorf("cannot canonicalize %T", token)
	}
}

func parseCanonicalObject(decoder *json.Decoder, depth int) ([]byte, error) {
	values := make(map[string][]byte)
	for decoder.More() {
		if len(values) >= maxObjectFields {
			return nil, fmt.Errorf("object field count exceeds %d", maxObjectFields)
		}
		token, err := decoder.Token()
		if err != nil {
			return nil, err
		}
		key, ok := token.(string)
		if !ok || !asciiSnakeKey(key) {
			return nil, fmt.Errorf("object key %q is not ASCII snake_case", token)
		}
		if err := validateString(key); err != nil {
			return nil, fmt.Errorf("object key %q: %w", key, err)
		}
		if _, exists := values[key]; exists {
			return nil, fmt.Errorf("duplicate JSON key %q", key)
		}
		child, err := parseCanonicalValue(decoder, depth+1)
		if err != nil {
			return nil, fmt.Errorf("field %q: %w", key, err)
		}
		values[key] = child
	}
	if token, err := decoder.Token(); err != nil || token != json.Delim('}') {
		return nil, fmt.Errorf("object is not closed")
	}
	keys := make([]string, 0, len(values))
	for key := range values {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	buffer := bytes.NewBuffer(nil)
	buffer.WriteByte('{')
	for index, key := range keys {
		if index > 0 {
			buffer.WriteByte(',')
		}
		appendCanonicalString(buffer, key)
		buffer.WriteByte(':')
		buffer.Write(values[key])
	}
	buffer.WriteByte('}')
	return buffer.Bytes(), nil
}

func parseCanonicalArray(decoder *json.Decoder, depth int) ([]byte, error) {
	children := make([][]byte, 0)
	for decoder.More() {
		if len(children) >= maxArrayItems {
			return nil, fmt.Errorf("array item count exceeds %d", maxArrayItems)
		}
		child, err := parseCanonicalValue(decoder, depth+1)
		if err != nil {
			return nil, fmt.Errorf("array item %d: %w", len(children), err)
		}
		children = append(children, child)
	}
	if token, err := decoder.Token(); err != nil || token != json.Delim(']') {
		return nil, fmt.Errorf("array is not closed")
	}
	buffer := bytes.NewBuffer(nil)
	buffer.WriteByte('[')
	for index, child := range children {
		if index > 0 {
			buffer.WriteByte(',')
		}
		buffer.Write(child)
	}
	buffer.WriteByte(']')
	return buffer.Bytes(), nil
}

func appendCanonicalString(buffer *bytes.Buffer, value string) {
	buffer.WriteByte('"')
	for _, character := range value {
		if character == '"' || character == '\\' {
			buffer.WriteByte('\\')
		}
		buffer.WriteRune(character)
	}
	buffer.WriteByte('"')
}

func validateString(value string) error {
	if !utf8.ValidString(value) || len(value) > maxStringBytes {
		return fmt.Errorf("string must be valid UTF-8 and at most %d bytes", maxStringBytes)
	}
	for _, character := range value {
		if unicode.IsControl(character) || character == 0x061c || character == 0x200e ||
			character == 0x200f || character == 0x2028 || character == 0x2029 ||
			(character >= 0x202a && character <= 0x202e) ||
			(character >= 0x2066 && character <= 0x2069) {
			return fmt.Errorf("string contains forbidden Unicode U+%04X", character)
		}
	}
	return nil
}

func asciiSnakeKey(value string) bool {
	if value == "" || value[0] < 'a' || value[0] > 'z' {
		return false
	}
	for _, character := range []byte(value[1:]) {
		if (character < 'a' || character > 'z') &&
			(character < '0' || character > '9') && character != '_' {
			return false
		}
	}
	return true
}

func requireEOF(decoder *json.Decoder) error {
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return fmt.Errorf("trailing JSON value")
		}
		return fmt.Errorf("trailing JSON: %w", err)
	}
	return nil
}

func cloneValue[T any](value *T, maximum int) (*T, error) {
	if value == nil {
		return nil, fmt.Errorf("cannot clone nil value")
	}
	raw, err := canonicalBytes(value, maximum)
	if err != nil {
		return nil, err
	}
	var result T
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&result); err != nil {
		return nil, fmt.Errorf("clone decode: %w", err)
	}
	return &result, requireEOF(decoder)
}

func decodeExact[T any](raw []byte, maximum int) (*T, error) {
	canonical, err := canonicalizeEncodedJSON(raw, maximum)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(canonical, raw) {
		return nil, fmt.Errorf("input is not exact compact canonical JSON")
	}
	var value T
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	decoder.UseNumber()
	if err := decoder.Decode(&value); err != nil {
		return nil, fmt.Errorf("invalid typed JSON: %w", err)
	}
	if err := requireEOF(decoder); err != nil {
		return nil, err
	}
	typed, err := canonicalBytes(&value, maximum)
	if err != nil || !bytes.Equal(typed, raw) {
		return nil, fmt.Errorf("input omits or changes an exact typed field")
	}
	return &value, nil
}
