package approvalrecordcontract

import (
	"bytes"
	"fmt"
	"sort"
	"strconv"
)

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
