package graphrelease

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"unicode/utf8"
)

var errInvalidControl = errors.New("invalid Node Dispatch release control")

// DecodeControl reads one bounded, strict, exactly canonical release-control
// snapshot and fully revalidates every durable binding before returning it.
func DecodeControl(reader io.Reader) (ReleaseControl, error) {
	data, err := io.ReadAll(io.LimitReader(reader, MaxReleaseControlBytes+1))
	if err != nil || len(data) == 0 || len(data) > MaxReleaseControlBytes ||
		!utf8.Valid(data) || !validUnicodeEscapes(data) {
		return ReleaseControl{}, errInvalidControl
	}
	if rejectDuplicateFields(data) != nil {
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
	var control ReleaseControl
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&control); err != nil {
		return ReleaseControl{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return ReleaseControl{}, errInvalidControl
	}
	return control, nil
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
