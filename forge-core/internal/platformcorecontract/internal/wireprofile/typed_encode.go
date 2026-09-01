package wireprofile

import (
	"encoding/json"
	"fmt"
	"math"
	"reflect"
	"sort"
	"strconv"
	"strings"
)

var jsonNumberType = reflect.TypeOf(json.Number(""))

type typedJSONField struct {
	name  string
	value reflect.Value
}

func encodeTypedCanonical(value any, maximum int) ([]byte, error) {
	buffer := &boundedJSONBuffer{maximum: maximum}
	if err := appendTypedJSON(buffer, reflect.ValueOf(value), 1); err != nil {
		return nil, fmt.Errorf("typed JSON encoding failed: %w", err)
	}
	return buffer.data, nil
}

func appendTypedJSON(buffer *boundedJSONBuffer, value reflect.Value, depth int) error {
	resolved, null, err := indirectJSONValue(value)
	if err != nil {
		return err
	}
	if depth > maxJSONDepth {
		return fmt.Errorf("JSON depth exceeds %d", maxJSONDepth)
	}
	if err := buffer.claimNode(); err != nil {
		return err
	}
	if null {
		return buffer.writeString("null")
	}
	return appendTypedKind(buffer, resolved, depth)
}

func indirectJSONValue(value reflect.Value) (reflect.Value, bool, error) {
	for hops := 0; hops <= maxJSONDepth; hops++ {
		if !value.IsValid() {
			return reflect.Value{}, true, nil
		}
		switch value.Kind() {
		case reflect.Interface, reflect.Pointer:
			if value.IsNil() {
				return reflect.Value{}, true, nil
			}
			value = value.Elem()
		default:
			return value, false, nil
		}
	}
	return reflect.Value{}, false, fmt.Errorf("JSON pointer/interface indirection is cyclic or too deep")
}

func appendTypedKind(buffer *boundedJSONBuffer, value reflect.Value, depth int) error {
	if value.Type() == jsonNumberType {
		integer, err := parseCanonicalInteger(value.String())
		if err != nil {
			return err
		}
		return buffer.writeString(strconv.FormatInt(integer, 10))
	}
	switch value.Kind() {
	case reflect.Struct:
		return appendTypedStruct(buffer, value, depth)
	case reflect.Map:
		return appendTypedMap(buffer, value, depth)
	case reflect.Slice, reflect.Array:
		return appendTypedArray(buffer, value, depth)
	case reflect.String:
		return appendTypedString(buffer, value.String(), "JSON string", false)
	case reflect.Bool:
		return buffer.writeString(strconv.FormatBool(value.Bool()))
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		return buffer.writeString(strconv.FormatInt(value.Int(), 10))
	case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64:
		if value.Uint() > math.MaxInt64 {
			return fmt.Errorf("JSON integer exceeds signed int64")
		}
		return buffer.writeString(strconv.FormatUint(value.Uint(), 10))
	default:
		return fmt.Errorf("cannot canonicalize typed JSON value %s", value.Kind())
	}
}

func appendTypedArray(buffer *boundedJSONBuffer, value reflect.Value, depth int) error {
	if value.Kind() == reflect.Slice && value.IsNil() {
		return buffer.writeString("null")
	}
	if value.Len() > maxArrayItems {
		return fmt.Errorf("JSON array exceeds %d items", maxArrayItems)
	}
	if err := buffer.writeByte('['); err != nil {
		return err
	}
	for index := 0; index < value.Len(); index++ {
		if index > 0 {
			if err := buffer.writeByte(','); err != nil {
				return err
			}
		}
		if err := appendTypedJSON(buffer, value.Index(index), depth+1); err != nil {
			return err
		}
	}
	return buffer.writeByte(']')
}

func appendTypedMap(buffer *boundedJSONBuffer, value reflect.Value, depth int) error {
	if value.IsNil() {
		return buffer.writeString("null")
	}
	if value.Type().Key().Kind() != reflect.String {
		return fmt.Errorf("JSON object keys must be strings")
	}
	if value.Len() > maxObjectFields {
		return fmt.Errorf("JSON object exceeds %d fields", maxObjectFields)
	}
	keys := value.MapKeys()
	for _, key := range keys {
		if err := ValidateText(key.String(), "JSON object key", maxStringBytes, true); err != nil {
			return err
		}
	}
	sort.Slice(keys, func(left, right int) bool { return keys[left].String() < keys[right].String() })
	if err := buffer.writeByte('{'); err != nil {
		return err
	}
	for index, key := range keys {
		if err := appendTypedMemberPrefix(buffer, index, key.String()); err != nil {
			return err
		}
		if err := appendTypedJSON(buffer, value.MapIndex(key), depth+1); err != nil {
			return err
		}
	}
	return buffer.writeByte('}')
}

func appendTypedStruct(buffer *boundedJSONBuffer, value reflect.Value, depth int) error {
	fields, err := typedStructFields(value)
	if err != nil {
		return err
	}
	if err := buffer.writeByte('{'); err != nil {
		return err
	}
	for index, field := range fields {
		if err := appendTypedMemberPrefix(buffer, index, field.name); err != nil {
			return err
		}
		if err := appendTypedJSON(buffer, field.value, depth+1); err != nil {
			return err
		}
	}
	return buffer.writeByte('}')
}

func typedStructFields(value reflect.Value) ([]typedJSONField, error) {
	if value.NumField() > maxObjectFields {
		return nil, fmt.Errorf("JSON object exceeds %d fields", maxObjectFields)
	}
	fields := make([]typedJSONField, 0, value.NumField())
	typeValue := value.Type()
	for index := 0; index < value.NumField(); index++ {
		fieldType := typeValue.Field(index)
		if fieldType.PkgPath != "" {
			continue
		}
		name, skip := typedJSONFieldName(fieldType)
		if skip {
			continue
		}
		if err := ValidateText(name, "JSON object key", maxStringBytes, true); err != nil {
			return nil, err
		}
		fields = append(fields, typedJSONField{name: name, value: value.Field(index)})
	}
	sort.Slice(fields, func(left, right int) bool { return fields[left].name < fields[right].name })
	for index := 1; index < len(fields); index++ {
		if fields[index-1].name == fields[index].name {
			return nil, fmt.Errorf("duplicate typed JSON field %q", fields[index].name)
		}
	}
	return fields, nil
}

func typedJSONFieldName(field reflect.StructField) (string, bool) {
	tag := field.Tag.Get("json")
	name, _, _ := strings.Cut(tag, ",")
	if name == "-" {
		return "", true
	}
	if name == "" {
		name = field.Name
	}
	return name, false
}

func appendTypedMemberPrefix(buffer *boundedJSONBuffer, index int, key string) error {
	if index > 0 {
		if err := buffer.writeByte(','); err != nil {
			return err
		}
	}
	if err := appendTypedString(buffer, key, "JSON object key", true); err != nil {
		return err
	}
	return buffer.writeByte(':')
}

func appendTypedString(
	buffer *boundedJSONBuffer, value, label string, nonempty bool,
) error {
	if err := ValidateText(value, label, maxStringBytes, nonempty); err != nil {
		return err
	}
	return appendJSONString(buffer, value)
}
