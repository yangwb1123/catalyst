package graphdispatch

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"unicode/utf8"
)

var errInvalidControl = errors.New("invalid Group Agent Graph control snapshot")

// DecodeControl reads one bounded, strict, exactly canonical control snapshot.
func DecodeControl(reader io.Reader) (ControlSnapshot, error) {
	data, err := io.ReadAll(io.LimitReader(reader, MaxControlSnapshotBytes+1))
	if err != nil || len(data) == 0 || len(data) > MaxControlSnapshotBytes ||
		!utf8.Valid(data) || !validUnicodeEscapes(data) {
		return ControlSnapshot{}, errInvalidControl
	}
	if rejectDuplicateFields(data) != nil || validateControlShape(data) != nil {
		return ControlSnapshot{}, errInvalidControl
	}
	snapshot, err := decodeControlWire(data)
	if err != nil || validateControl(snapshot) != nil {
		return ControlSnapshot{}, errInvalidControl
	}
	canonical, err := canonicalBytes(snapshot)
	if err != nil || !bytes.Equal(data, canonical) {
		return ControlSnapshot{}, errInvalidControl
	}
	return snapshot, nil
}

func decodeControlWire(data []byte) (ControlSnapshot, error) {
	var snapshot ControlSnapshot
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&snapshot); err != nil {
		return ControlSnapshot{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return ControlSnapshot{}, errInvalidControl
	}
	return snapshot, nil
}

func rejectDuplicateFields(data []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	if err := scanJSONValue(decoder); err != nil {
		return err
	}
	if _, err := decoder.Token(); err != io.EOF {
		return errInvalidControl
	}
	return nil
}

func scanJSONValue(decoder *json.Decoder) error {
	token, err := decoder.Token()
	if err != nil {
		return err
	}
	delimiter, compound := token.(json.Delim)
	if !compound {
		return nil
	}
	switch delimiter {
	case '{':
		return scanJSONObject(decoder)
	case '[':
		return scanJSONArray(decoder)
	default:
		return errInvalidControl
	}
}

func scanJSONObject(decoder *json.Decoder) error {
	seen := make(map[string]struct{})
	for decoder.More() {
		token, err := decoder.Token()
		key, valid := token.(string)
		if err != nil || !valid {
			return errInvalidControl
		}
		if _, duplicate := seen[key]; duplicate {
			return errInvalidControl
		}
		seen[key] = struct{}{}
		if err := scanJSONValue(decoder); err != nil {
			return err
		}
	}
	return consumeDelimiter(decoder, '}')
}

func scanJSONArray(decoder *json.Decoder) error {
	for decoder.More() {
		if err := scanJSONValue(decoder); err != nil {
			return err
		}
	}
	return consumeDelimiter(decoder, ']')
}

func consumeDelimiter(decoder *json.Decoder, expected json.Delim) error {
	token, err := decoder.Token()
	if err != nil || token != expected {
		return errInvalidControl
	}
	return nil
}
