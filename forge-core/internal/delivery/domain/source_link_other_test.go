//go:build !unix && !windows

package domain

import (
	"os"
	"reflect"
)

func sourceLinkCount(_ string, info os.FileInfo) (uint64, bool) {
	value := reflect.ValueOf(info.Sys())
	for value.IsValid() && (value.Kind() == reflect.Pointer || value.Kind() == reflect.Interface) {
		if value.IsNil() {
			return 0, false
		}
		value = value.Elem()
	}
	if !value.IsValid() || value.Kind() != reflect.Struct {
		return 0, false
	}
	field := value.FieldByName("Nlink")
	if !field.IsValid() {
		field = value.FieldByName("NumberOfLinks")
	}
	if field.IsValid() && field.CanUint() {
		return field.Uint(), true
	}
	return 0, false
}
