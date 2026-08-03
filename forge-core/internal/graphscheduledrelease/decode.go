package graphscheduledrelease

import (
	"bytes"
	"encoding/json"
	"io"
	"unicode/utf8"
)

// DecodeControl reads one bounded, strict, exact canonical release control.
func DecodeControl(reader io.Reader) (ReleaseControl, error) {
	data, err := io.ReadAll(io.LimitReader(reader, MaxReleaseControlBytes+1))
	if err != nil || len(data) == 0 || len(data) > MaxReleaseControlBytes ||
		!utf8.Valid(data) || !validUnicodeEscapes(data) || rejectDuplicateFields(data) != nil {
		return ReleaseControl{}, errInvalidControl
	}
	control, err := decodeControlWire(data)
	if err != nil || validateReleaseControl(control) != nil {
		return ReleaseControl{}, errInvalidControl
	}
	canonical, err := canonicalBytes(control)
	if err != nil || !bytes.Equal(data, canonical) {
		return ReleaseControl{}, errInvalidControl
	}
	return control, nil
}

func decodeControlWire(data []byte) (ReleaseControl, error) {
	var value ReleaseControl
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&value); err != nil {
		return ReleaseControl{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return ReleaseControl{}, errInvalidControl
	}
	return value, nil
}

func decodeExact[T any](data []byte) (T, error) {
	var value T
	if len(data) == 0 || !utf8.Valid(data) || !validUnicodeEscapes(data) ||
		rejectDuplicateFields(data) != nil {
		return value, errInvalidControl
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&value); err != nil {
		return value, errInvalidControl
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return value, errInvalidControl
	}
	canonical, err := canonicalBytes(value)
	if err != nil || !bytes.Equal(data, canonical) {
		return value, errInvalidControl
	}
	return value, nil
}
