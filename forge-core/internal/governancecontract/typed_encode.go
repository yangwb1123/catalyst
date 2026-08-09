package governancecontract

import (
	"bytes"
	"encoding/json"
	"fmt"
	"reflect"
	"unicode/utf8"
)

func encodeTypedJSON(value any) ([]byte, error) {
	if err := rejectInvalidUTF8(reflect.ValueOf(value)); err != nil {
		return nil, err
	}
	buffer := bytes.NewBuffer(nil)
	encoder := json.NewEncoder(buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, err
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, fmt.Errorf("typed JSON encoder did not terminate predictably")
	}
	return append([]byte(nil), encoded[:len(encoded)-1]...), nil
}

func rejectInvalidUTF8(value reflect.Value) error {
	if !value.IsValid() {
		return nil
	}
	switch value.Kind() {
	case reflect.Interface, reflect.Pointer:
		if value.IsNil() {
			return nil
		}
		return rejectInvalidUTF8(value.Elem())
	case reflect.String:
		if !utf8.ValidString(value.String()) {
			return fmt.Errorf("typed string is not valid UTF-8")
		}
	case reflect.Struct:
		return rejectStructUTF8(value)
	case reflect.Slice, reflect.Array:
		return rejectSequenceUTF8(value)
	case reflect.Map:
		return rejectMapUTF8(value)
	}
	return nil
}

func rejectStructUTF8(value reflect.Value) error {
	for index := 0; index < value.NumField(); index++ {
		if err := rejectInvalidUTF8(value.Field(index)); err != nil {
			return err
		}
	}
	return nil
}

func rejectSequenceUTF8(value reflect.Value) error {
	for index := 0; index < value.Len(); index++ {
		if err := rejectInvalidUTF8(value.Index(index)); err != nil {
			return err
		}
	}
	return nil
}

func rejectMapUTF8(value reflect.Value) error {
	iterator := value.MapRange()
	for iterator.Next() {
		if err := rejectInvalidUTF8(iterator.Key()); err != nil {
			return err
		}
		if err := rejectInvalidUTF8(iterator.Value()); err != nil {
			return err
		}
	}
	return nil
}
