//go:build unix

package planningownership

import (
	"os"
	"reflect"
	"syscall"
)

func openRegularNoFollow(path string) (*os.File, error) {
	descriptor, err := syscall.Open(path, syscall.O_RDONLY|syscall.O_CLOEXEC|syscall.O_NOFOLLOW|syscall.O_NONBLOCK, 0)
	if err != nil {
		return nil, err
	}
	return os.NewFile(uintptr(descriptor), path), nil
}

func samePlatformChangeTime(left, right os.FileInfo) bool {
	firstSeconds, firstNanos, firstOK := platformChangeTime(left)
	secondSeconds, secondNanos, secondOK := platformChangeTime(right)
	return firstOK && secondOK && firstSeconds == secondSeconds && firstNanos == secondNanos
}

func platformChangeTime(info os.FileInfo) (int64, int64, bool) {
	value := reflect.ValueOf(info.Sys())
	if value.Kind() != reflect.Pointer || value.IsNil() {
		return 0, 0, false
	}
	value = value.Elem()
	var timestamp reflect.Value
	for _, name := range []string{"Ctim", "Ctimespec", "Ctimspec"} {
		timestamp = value.FieldByName(name)
		if timestamp.IsValid() {
			break
		}
	}
	if !timestamp.IsValid() {
		return 0, 0, false
	}
	seconds, nanos := timestamp.FieldByName("Sec"), timestamp.FieldByName("Nsec")
	if !seconds.IsValid() || !nanos.IsValid() || !seconds.CanInt() || !nanos.CanInt() {
		return 0, 0, false
	}
	return seconds.Int(), nanos.Int(), true
}
