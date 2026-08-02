package graphterminal

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"unicode/utf8"
)

var errInvalidControl = errors.New("invalid Graph node terminal control")

// DecodeControl reads one bounded, strict, canonical control and rebuilds all
// identities before returning it.
func DecodeControl(reader io.Reader) (TerminalControl, error) {
	data, err := io.ReadAll(io.LimitReader(reader, MaxTerminalControlBytes+1))
	if err != nil || len(data) == 0 || len(data) > MaxTerminalControlBytes ||
		!utf8.Valid(data) || !validUnicodeEscapes(data) || rejectDuplicateFields(data) != nil {
		return TerminalControl{}, errInvalidControl
	}
	control, err := decodeControlWire(data)
	if err != nil || validateControl(control) != nil {
		return TerminalControl{}, errInvalidControl
	}
	canonical, err := canonicalBytes(control)
	if err != nil || !bytes.Equal(data, canonical) {
		return TerminalControl{}, errInvalidControl
	}
	return control, nil
}

func decodeControlWire(data []byte) (TerminalControl, error) {
	var control TerminalControl
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&control); err != nil {
		return TerminalControl{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return TerminalControl{}, errInvalidControl
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
