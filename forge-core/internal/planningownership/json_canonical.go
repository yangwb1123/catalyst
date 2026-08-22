package planningownership

import (
	"bytes"
	"fmt"
	"sort"
	"strconv"
)

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
		if err := appendCanonical(buffer, object[key]); err != nil {
			return err
		}
	}
	buffer.WriteByte('}')
	return nil
}

func appendCanonicalArray(buffer *bytes.Buffer, array []any) error {
	buffer.WriteByte('[')
	for index, child := range array {
		if index > 0 {
			buffer.WriteByte(',')
		}
		if err := appendCanonical(buffer, child); err != nil {
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
