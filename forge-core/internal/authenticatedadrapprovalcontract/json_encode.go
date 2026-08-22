package authenticatedadrapprovalcontract

import (
	"bytes"
	"fmt"
	"sort"
	"strconv"
)

func canonicalJSON(value any) ([]byte, error) {
	if _, err := measureCanonical(value, 1, maxBundleBytes); err != nil {
		return nil, err
	}
	buffer := bytes.NewBuffer(nil)
	if err := appendCanonical(buffer, value, 1); err != nil {
		return nil, err
	}
	return buffer.Bytes(), nil
}

func boundedCanonicalJSON(value any, maximum int, label string) ([]byte, error) {
	size, err := measureCanonical(value, 1, maximum)
	if err != nil {
		return nil, fmt.Errorf("%s: %w", label, err)
	}
	buffer := bytes.NewBuffer(make([]byte, 0, size))
	if err := appendCanonical(buffer, value, 1); err != nil {
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
		if !isASCIISnakeKey(key) {
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
		size, err := measureCanonical(child, depth+1, remaining-total)
		if err != nil {
			return 0, err
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
		return 0, fmt.Errorf("canonical JSON exceeds its configured byte ceiling")
	}
	return size, nil
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

func appendCanonicalArray(buffer *bytes.Buffer, array []any, depth int) error {
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
