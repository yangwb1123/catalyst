package evolvelocatorobservationproducer

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"reflect"
	"sort"
	"strconv"
	"strings"
	"unicode"
	"unicode/utf8"
)

func canonicalJSON(value any) ([]byte, error) {
	buffer := bytes.NewBuffer(nil)
	if err := appendCanonicalValue(buffer, reflect.ValueOf(value)); err != nil {
		return nil, err
	}
	return buffer.Bytes(), nil
}

func appendCanonicalValue(buffer *bytes.Buffer, value reflect.Value) error {
	if !value.IsValid() || value.Kind() == reflect.Pointer && value.IsNil() {
		buffer.WriteString("null")
		return nil
	}
	if value.Kind() == reflect.Pointer || value.Kind() == reflect.Interface {
		return appendCanonicalValue(buffer, value.Elem())
	}
	switch value.Kind() {
	case reflect.Struct:
		return appendCanonicalStruct(buffer, value)
	case reflect.Slice, reflect.Array:
		return appendCanonicalSlice(buffer, value)
	case reflect.String:
		return appendCanonicalString(buffer, value.String())
	case reflect.Bool:
		buffer.WriteString(strconv.FormatBool(value.Bool()))
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		buffer.WriteString(strconv.FormatInt(value.Int(), 10))
	default:
		return fmt.Errorf("cannot canonicalize %s", value.Kind())
	}
	return nil
}

type canonicalField struct {
	name  string
	value reflect.Value
}

func appendCanonicalStruct(buffer *bytes.Buffer, value reflect.Value) error {
	fields := make([]canonicalField, 0, value.NumField())
	typeValue := value.Type()
	for index := 0; index < value.NumField(); index++ {
		field := typeValue.Field(index)
		if field.PkgPath != "" {
			continue
		}
		name := strings.Split(field.Tag.Get("json"), ",")[0]
		if name == "" {
			name = field.Name
		}
		if name != "-" {
			fields = append(fields, canonicalField{name: name, value: value.Field(index)})
		}
	}
	sort.Slice(fields, func(i, j int) bool { return fields[i].name < fields[j].name })
	buffer.WriteByte('{')
	for index, field := range fields {
		if index > 0 {
			buffer.WriteByte(',')
		}
		if err := appendCanonicalString(buffer, field.name); err != nil {
			return err
		}
		buffer.WriteByte(':')
		if err := appendCanonicalValue(buffer, field.value); err != nil {
			return fmt.Errorf("field %q: %w", field.name, err)
		}
	}
	buffer.WriteByte('}')
	return nil
}

func appendCanonicalSlice(buffer *bytes.Buffer, value reflect.Value) error {
	buffer.WriteByte('[')
	for index := 0; index < value.Len(); index++ {
		if index > 0 {
			buffer.WriteByte(',')
		}
		if err := appendCanonicalValue(buffer, value.Index(index)); err != nil {
			return err
		}
	}
	buffer.WriteByte(']')
	return nil
}

func appendCanonicalString(buffer *bytes.Buffer, value string) error {
	if err := validateCanonicalText(value); err != nil {
		return err
	}
	buffer.WriteByte('"')
	for _, character := range value {
		if character == '"' || character == '\\' {
			buffer.WriteByte('\\')
		}
		buffer.WriteRune(character)
	}
	buffer.WriteByte('"')
	return nil
}

func validateCanonicalText(value string) error {
	if !utf8.ValidString(value) {
		return fmt.Errorf("string is not valid UTF-8")
	}
	for _, character := range value {
		if unicode.Is(unicode.Cc, character) || forbiddenDirectionalRune(character) ||
			character == '\u2028' || character == '\u2029' {
			return fmt.Errorf("string contains forbidden Unicode U+%04X", character)
		}
	}
	return nil
}

func forbiddenDirectionalRune(value rune) bool {
	return value == '\u061c' || value == '\u200e' || value == '\u200f' ||
		(value >= '\u202a' && value <= '\u202e') ||
		(value >= '\u2066' && value <= '\u2069')
}

func sha256Bytes(value []byte) string {
	digest := sha256.Sum256(value)
	return hex.EncodeToString(digest[:])
}

func domainDigest(domain string, value []byte) string {
	preimage := append([]byte(domain+"\x00"), value...)
	return sha256Bytes(preimage)
}
