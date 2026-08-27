package graphscheduledrelease

import (
	"bytes"
	"encoding/json"
	"io"
	"unicode/utf8"
)

// DecodeReadyReleaseControl reads one bounded, strict canonical v2 control.
func DecodeReadyReleaseControl(reader io.Reader) (ReadyReleaseControl, error) {
	data, err := io.ReadAll(io.LimitReader(reader, MaxReadyReleaseControlBytes+1))
	if err != nil || len(data) == 0 || len(data) > MaxReadyReleaseControlBytes ||
		!utf8.Valid(data) || !validUnicodeEscapes(data) || rejectDuplicateFields(data) != nil {
		return ReadyReleaseControl{}, errInvalidControl
	}
	value, err := decodeReadyReleaseControlWire(data)
	if err != nil || validateReadyReleaseControl(value) != nil {
		return ReadyReleaseControl{}, errInvalidControl
	}
	canonical, err := canonicalBytes(value)
	if err != nil || !bytes.Equal(data, canonical) {
		return ReadyReleaseControl{}, errInvalidControl
	}
	return value, nil
}

func decodeReadyReleaseControlWire(data []byte) (ReadyReleaseControl, error) {
	var value ReadyReleaseControl
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&value); err != nil {
		return ReadyReleaseControl{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return ReadyReleaseControl{}, errInvalidControl
	}
	return value, nil
}
