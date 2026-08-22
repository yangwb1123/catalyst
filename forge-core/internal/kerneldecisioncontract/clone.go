package kerneldecisioncontract

import (
	"bytes"
	"encoding/json"
	"fmt"
	"reflect"
	"strings"
	"unicode"
	"unicode/utf8"
)

const (
	maxTypedDepth       = 16
	maxTypedFields      = 64
	maxTypedArrayItems  = 256
	maxTypedStringBytes = 16_384
)

var rawMessageType = reflect.TypeOf(json.RawMessage{})

func cloneValue[T any](value *T) (*T, error) {
	if err := validateCloneTree(reflect.ValueOf(value), 1); err != nil {
		return nil, err
	}
	raw, err := encodeTypedJSON(value)
	if err != nil {
		return nil, fmt.Errorf("clone encode: %w", err)
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	decoder.UseNumber()
	var cloned T
	if err := decoder.Decode(&cloned); err != nil {
		return nil, fmt.Errorf("clone decode: %w", err)
	}
	return &cloned, requireEOF(decoder)
}

func validateCloneTree(value reflect.Value, depth int) error {
	if depth > maxTypedDepth {
		return fmt.Errorf("typed value depth exceeds %d", maxTypedDepth)
	}
	if !value.IsValid() {
		return nil
	}
	if value.Type() == rawMessageType {
		return validateRawMessage(value.Bytes())
	}
	if value.Kind() == reflect.Slice && value.Type().Elem().Kind() == reflect.Uint8 {
		return fmt.Errorf("typed byte slices are unsupported JSON values")
	}
	switch value.Kind() {
	case reflect.Interface, reflect.Pointer:
		if value.IsNil() {
			return nil
		}
		return validateCloneTree(value.Elem(), depth)
	case reflect.String:
		return validateCloneString(value.String())
	case reflect.Float32, reflect.Float64:
		return fmt.Errorf("typed JSON floating-point values are unsupported")
	case reflect.Struct:
		return validateCloneStruct(value, depth)
	case reflect.Array, reflect.Slice:
		for index := 0; index < value.Len(); index++ {
			if err := validateCloneTree(value.Index(index), depth+1); err != nil {
				return err
			}
		}
	case reflect.Map:
		return validateCloneMap(value, depth)
	}
	return nil
}

func validateCloneStruct(value reflect.Value, depth int) error {
	typeOfValue := value.Type()
	for index := 0; index < value.NumField(); index++ {
		fieldType := typeOfValue.Field(index)
		fieldValue := value.Field(index)
		tag := fieldType.Tag.Get("json")
		name, options, _ := strings.Cut(tag, ",")
		if name == "-" || cloneFieldIsUnexported(fieldType) ||
			hasJSONOption(options, "omitempty") && emptyJSONValue(fieldValue) ||
			hasJSONOption(options, "omitzero") && fieldValue.IsZero() {
			continue
		}
		if err := validateCloneTree(fieldValue, depth+1); err != nil {
			return err
		}
	}
	return nil
}

func cloneFieldIsUnexported(field reflect.StructField) bool {
	if field.PkgPath == "" {
		return false
	}
	typeOfField := field.Type
	if typeOfField.Kind() == reflect.Pointer {
		typeOfField = typeOfField.Elem()
	}
	return !field.Anonymous || typeOfField.Kind() != reflect.Struct
}

func hasJSONOption(options, expected string) bool {
	for _, option := range strings.Split(options, ",") {
		if option == expected {
			return true
		}
	}
	return false
}

func emptyJSONValue(value reflect.Value) bool {
	switch value.Kind() {
	case reflect.Array, reflect.Map, reflect.Slice, reflect.String:
		return value.Len() == 0
	case reflect.Bool, reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64,
		reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64, reflect.Uintptr,
		reflect.Float32, reflect.Float64, reflect.Interface, reflect.Pointer:
		return value.IsZero()
	}
	return false
}

func validateRawMessage(raw []byte) error {
	if len(raw) == 0 {
		return nil
	}
	canonical, err := canonicalizeEncodedJSON(raw, maxClosureBytes)
	if err != nil {
		return fmt.Errorf("typed raw JSON is invalid: %w", err)
	}
	if !bytes.Equal(canonical, raw) {
		return fmt.Errorf("typed raw JSON must be exact canonical JSON")
	}
	return nil
}

func validateCloneMap(value reflect.Value, depth int) error {
	iterator := value.MapRange()
	for iterator.Next() {
		if iterator.Key().Kind() != reflect.String {
			return fmt.Errorf("typed JSON map key must be a string")
		}
		if err := validateCloneString(iterator.Key().String()); err != nil {
			return err
		}
		if err := validateCloneTree(iterator.Value(), depth+1); err != nil {
			return err
		}
	}
	return nil
}

func validateCloneString(value string) error {
	if !utf8.ValidString(value) || len(value) > maxTypedStringBytes {
		return fmt.Errorf("typed string must be valid UTF-8 and at most %d bytes", maxTypedStringBytes)
	}
	for _, character := range value {
		if unicode.IsControl(character) || character == 0x061c || character == 0x200e ||
			character == 0x200f || character >= 0x2028 && character <= 0x202e ||
			character >= 0x2066 && character <= 0x2069 {
			return fmt.Errorf("typed string contains a forbidden Unicode scalar")
		}
	}
	return nil
}
