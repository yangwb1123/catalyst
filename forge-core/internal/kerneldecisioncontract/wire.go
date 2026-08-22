package kerneldecisioncontract

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"reflect"
	"sort"
	"strconv"
)

func canonicalBytes(value any, maximum int) ([]byte, error) {
	if err := validateCloneTree(reflect.ValueOf(value), 1); err != nil {
		return nil, err
	}
	raw, err := encodeTypedJSON(value)
	if err != nil {
		return nil, fmt.Errorf("canonical JSON encode: %w", err)
	}
	return canonicalizeEncodedJSON(raw, maximum)
}

func canonicalizeEncodedJSON(raw []byte, maximum int) ([]byte, error) {
	if len(raw) == 0 || len(raw) > maximum {
		return nil, fmt.Errorf("canonical JSON byte length must be 1..%d", maximum)
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	var node any
	if err := decoder.Decode(&node); err != nil {
		return nil, fmt.Errorf("canonical JSON decode: %w", err)
	}
	if err := requireEOF(decoder); err != nil {
		return nil, err
	}
	buffer := bytes.NewBuffer(make([]byte, 0, len(raw)))
	if err := appendCanonicalNode(buffer, node, 1); err != nil {
		return nil, err
	}
	if buffer.Len() == 0 || buffer.Len() > maximum {
		return nil, fmt.Errorf("canonical JSON byte length must be 1..%d", maximum)
	}
	return append([]byte(nil), buffer.Bytes()...), nil
}

func encodeTypedJSON(value any) ([]byte, error) {
	buffer := bytes.NewBuffer(nil)
	encoder := json.NewEncoder(buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, err
	}
	raw := buffer.Bytes()
	if len(raw) == 0 || raw[len(raw)-1] != '\n' {
		return nil, fmt.Errorf("typed JSON encoder did not terminate one value")
	}
	return append([]byte(nil), raw[:len(raw)-1]...), nil
}

func appendCanonicalNode(buffer *bytes.Buffer, value any, depth int) error {
	if depth > maxTypedDepth {
		return fmt.Errorf("JSON depth exceeds %d", maxTypedDepth)
	}
	switch member := value.(type) {
	case map[string]any:
		return appendCanonicalObject(buffer, member, depth)
	case []any:
		return appendCanonicalArray(buffer, member, depth)
	case string:
		if len(member) > maxTypedStringBytes {
			return fmt.Errorf("string exceeds %d bytes", maxTypedStringBytes)
		}
		if err := validateCloneString(member); err != nil {
			return err
		}
		appendCanonicalString(buffer, member)
	case json.Number:
		number, err := strconv.ParseInt(member.String(), 10, 64)
		if err != nil || strconv.FormatInt(number, 10) != member.String() {
			return fmt.Errorf("number %q is not a canonical signed int64", member)
		}
		buffer.WriteString(member.String())
	case bool:
		buffer.WriteString(strconv.FormatBool(member))
	case nil:
		buffer.WriteString("null")
	default:
		return fmt.Errorf("cannot canonicalize %T", value)
	}
	return nil
}

func appendCanonicalObject(buffer *bytes.Buffer, value map[string]any, depth int) error {
	if len(value) > maxTypedFields {
		return fmt.Errorf("object field count exceeds %d", maxTypedFields)
	}
	keys := make([]string, 0, len(value))
	for key := range value {
		if len(key) > maxTypedStringBytes || !asciiSnakeKey(key) || validateCloneString(key) != nil {
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
		appendCanonicalString(buffer, key)
		buffer.WriteByte(':')
		if err := appendCanonicalNode(buffer, value[key], depth+1); err != nil {
			return err
		}
	}
	buffer.WriteByte('}')
	return nil
}

func appendCanonicalArray(buffer *bytes.Buffer, value []any, depth int) error {
	if len(value) > maxTypedArrayItems {
		return fmt.Errorf("array item count exceeds %d", maxTypedArrayItems)
	}
	buffer.WriteByte('[')
	for index, item := range value {
		if index > 0 {
			buffer.WriteByte(',')
		}
		if err := appendCanonicalNode(buffer, item, depth+1); err != nil {
			return err
		}
	}
	buffer.WriteByte(']')
	return nil
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

func decodeTyped(raw []byte, maximum int, target any) error {
	if len(raw) == 0 || len(raw) > maximum {
		return fmt.Errorf("JSON byte length must be 1..%d", maximum)
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	decoder.UseNumber()
	if err := decoder.Decode(target); err != nil {
		return fmt.Errorf("invalid typed JSON: %w", err)
	}
	if err := requireEOF(decoder); err != nil {
		return err
	}
	canonical, err := canonicalBytes(target, maximum)
	if err != nil {
		return err
	}
	if !bytes.Equal(canonical, raw) {
		index := 0
		for index < len(canonical) && index < len(raw) && canonical[index] == raw[index] {
			index++
		}
		start, end := index-40, index+80
		if start < 0 {
			start = 0
		}
		if end > len(raw) {
			end = len(raw)
		}
		canonicalEnd := end
		if canonicalEnd > len(canonical) {
			canonicalEnd = len(canonical)
		}
		return fmt.Errorf("input is not exact compact canonical JSON at byte %d: input=%q encoded=%q",
			index, raw[start:end], canonical[start:canonicalEnd])
	}
	return nil
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

func exactRawObject(raw json.RawMessage, fields ...string) (map[string]any, error) {
	var value any
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	if err := decoder.Decode(&value); err != nil {
		return nil, fmt.Errorf("invalid reference JSON: %w", err)
	}
	if err := requireEOF(decoder); err != nil {
		return nil, err
	}
	member, ok := value.(map[string]any)
	if !ok || len(member) != len(fields) {
		return nil, fmt.Errorf("reference must have exact fields")
	}
	for _, field := range fields {
		if _, exists := member[field]; !exists {
			return nil, fmt.Errorf("reference missing field %q", field)
		}
	}
	canonical, err := canonicalBytes(member, maxStringBytes)
	if err != nil || !bytes.Equal(canonical, raw) {
		return nil, fmt.Errorf("reference must be exact canonical JSON")
	}
	return member, nil
}

func rawIsNull(raw json.RawMessage) bool {
	return bytes.Equal(raw, []byte("null")) || len(raw) == 0
}
