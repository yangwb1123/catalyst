package wireprofile

import (
	"fmt"
	"sort"
	"strconv"
)

// CanonicalJSON emits exact bounded Platform Core canonical JSON.
func CanonicalJSON(value any, maximum int) ([]byte, error) {
	buffer := &boundedJSONBuffer{maximum: maximum}
	if err := appendCanonicalJSON(buffer, value, 1); err != nil {
		return nil, err
	}
	return buffer.data, nil
}

type boundedJSONBuffer struct {
	data    []byte
	maximum int
	nodes   int
}

func (buffer *boundedJSONBuffer) claimNode() error {
	buffer.nodes++
	if buffer.nodes > buffer.maximum {
		return fmt.Errorf("canonical JSON exceeds %d bytes", buffer.maximum)
	}
	return nil
}

func (buffer *boundedJSONBuffer) writeByte(value byte) error {
	if len(buffer.data) >= buffer.maximum {
		return fmt.Errorf("canonical JSON exceeds %d bytes", buffer.maximum)
	}
	buffer.data = append(buffer.data, value)
	return nil
}

func (buffer *boundedJSONBuffer) writeString(value string) error {
	if len(value) > buffer.maximum-len(buffer.data) {
		return fmt.Errorf("canonical JSON exceeds %d bytes", buffer.maximum)
	}
	buffer.data = append(buffer.data, value...)
	return nil
}

func appendCanonicalJSON(buffer *boundedJSONBuffer, value any, depth int) error {
	if depth > maxJSONDepth {
		return fmt.Errorf("JSON depth exceeds %d", maxJSONDepth)
	}
	if err := buffer.claimNode(); err != nil {
		return err
	}
	switch typed := value.(type) {
	case map[string]any:
		if typed == nil {
			return fmt.Errorf("typed nil JSON object is ambiguous; use nil for null")
		}
		return appendCanonicalObject(buffer, typed, depth)
	case []any:
		if typed == nil {
			return fmt.Errorf("typed nil JSON array is ambiguous; use nil for null")
		}
		return appendCanonicalArray(buffer, typed, depth)
	case string:
		if err := ValidateText(typed, "JSON string", maxStringBytes, false); err != nil {
			return err
		}
		return appendJSONString(buffer, typed)
	case int64:
		return buffer.writeString(strconv.FormatInt(typed, 10))
	case bool:
		return buffer.writeString(strconv.FormatBool(typed))
	case nil:
		return buffer.writeString("null")
	default:
		return fmt.Errorf("cannot canonicalize JSON value %T", value)
	}
}

func appendCanonicalObject(buffer *boundedJSONBuffer, object map[string]any, depth int) error {
	if len(object) > maxObjectFields {
		return fmt.Errorf("JSON object exceeds %d fields", maxObjectFields)
	}
	keys := make([]string, 0, len(object))
	for key := range object {
		if err := ValidateText(key, "JSON object key", maxStringBytes, true); err != nil {
			return err
		}
		keys = append(keys, key)
	}
	sort.Strings(keys)
	if err := buffer.writeByte('{'); err != nil {
		return err
	}
	for index, key := range keys {
		if index > 0 {
			if err := buffer.writeByte(','); err != nil {
				return err
			}
		}
		if err := appendJSONString(buffer, key); err != nil {
			return err
		}
		if err := buffer.writeByte(':'); err != nil {
			return err
		}
		if err := appendCanonicalJSON(buffer, object[key], depth+1); err != nil {
			return err
		}
	}
	return buffer.writeByte('}')
}

func appendCanonicalArray(buffer *boundedJSONBuffer, array []any, depth int) error {
	if len(array) > maxArrayItems {
		return fmt.Errorf("JSON array exceeds %d items", maxArrayItems)
	}
	if err := buffer.writeByte('['); err != nil {
		return err
	}
	for index, child := range array {
		if index > 0 {
			if err := buffer.writeByte(','); err != nil {
				return err
			}
		}
		if err := appendCanonicalJSON(buffer, child, depth+1); err != nil {
			return err
		}
	}
	return buffer.writeByte(']')
}

func appendJSONString(buffer *boundedJSONBuffer, value string) error {
	if err := buffer.writeByte('"'); err != nil {
		return err
	}
	for index := 0; index < len(value); index++ {
		if value[index] == '"' || value[index] == '\\' {
			if err := buffer.writeByte('\\'); err != nil {
				return err
			}
		}
		if err := buffer.writeByte(value[index]); err != nil {
			return err
		}
	}
	return buffer.writeByte('"')
}
