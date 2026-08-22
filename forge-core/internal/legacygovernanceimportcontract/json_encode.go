package legacygovernanceimportcontract

import (
	"bytes"
	"fmt"
	"sort"
	"strconv"
)

func canonicalJSON(value any, maximum int, label string) ([]byte, error) {
	buffer := bytes.NewBuffer(nil)
	if err := appendCanonical(buffer, value, 1, maximum); err != nil {
		return nil, fmt.Errorf("%s: %w", label, err)
	}
	if buffer.Len() == 0 || buffer.Len() > maximum {
		return nil, fmt.Errorf("%s byte length must be 1..%d", label, maximum)
	}
	return buffer.Bytes(), nil
}

func appendCanonical(buffer *bytes.Buffer, value any, depth, maximum int) error {
	if depth > maxJSONDepth || buffer.Len() > maximum {
		return fmt.Errorf("canonical JSON exceeds depth or byte bound")
	}
	switch typed := value.(type) {
	case map[string]any:
		return appendObject(buffer, typed, depth, maximum)
	case []any:
		return appendArray(buffer, typed, depth, maximum)
	case string:
		if err := validateWireString(typed, maxStringBytes, false); err != nil {
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
	if buffer.Len() > maximum {
		return fmt.Errorf("canonical JSON exceeds byte bound %d", maximum)
	}
	return nil
}

func appendObject(buffer *bytes.Buffer, object map[string]any, depth, maximum int) error {
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
		if err := appendCanonical(buffer, object[key], depth+1, maximum); err != nil {
			return err
		}
	}
	buffer.WriteByte('}')
	return nil
}

func appendArray(buffer *bytes.Buffer, array []any, depth, maximum int) error {
	if len(array) > maxArrayItems {
		return fmt.Errorf("array item count exceeds %d", maxArrayItems)
	}
	buffer.WriteByte('[')
	for index, child := range array {
		if index > 0 {
			buffer.WriteByte(',')
		}
		if err := appendCanonical(buffer, child, depth+1, maximum); err != nil {
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
