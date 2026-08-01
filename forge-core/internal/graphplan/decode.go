package graphplan

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"unicode/utf8"
)

var errInvalidSpec = errors.New("invalid Group Agent Graph spec")

type specWire struct {
	V       *uint16  `json:"v"`
	Manager *Manager `json:"manager"`
	Nodes   *[]Node  `json:"nodes"`
	Edges   *[]Edge  `json:"edges"`
}

// Decode reads one bounded, strict, UTF-8 Group Agent Graph v1 specification.
func Decode(reader io.Reader) (Spec, error) {
	data, err := readBounded(reader)
	if err != nil || !utf8.Valid(data) || !validUnicodeEscapes(data) ||
		rejectDuplicateFields(data) != nil || validateStrictShape(data) != nil {
		return Spec{}, errInvalidSpec
	}
	wire, err := decodeWire(data)
	if err != nil || wire.V == nil || wire.Manager == nil ||
		wire.Nodes == nil || wire.Edges == nil {
		return Spec{}, errInvalidSpec
	}
	spec := Spec{
		V:       *wire.V,
		Manager: *wire.Manager,
		Nodes:   *wire.Nodes,
		Edges:   *wire.Edges,
	}
	if _, err := validateAndPlan(spec); err != nil {
		return Spec{}, errInvalidSpec
	}
	return spec, nil
}

func readBounded(reader io.Reader) ([]byte, error) {
	data, err := io.ReadAll(io.LimitReader(reader, MaxSpecBytes+1))
	if err != nil || len(data) == 0 || len(data) > MaxSpecBytes {
		return nil, errInvalidSpec
	}
	return data, nil
}

func decodeWire(data []byte) (specWire, error) {
	var wire specWire
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&wire); err != nil {
		return specWire{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return specWire{}, errInvalidSpec
	}
	return wire, nil
}

func rejectDuplicateFields(data []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	if err := scanJSONValue(decoder); err != nil {
		return err
	}
	if _, err := decoder.Token(); err != io.EOF {
		return errInvalidSpec
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
		return errInvalidSpec
	}
}

func scanJSONObject(decoder *json.Decoder) error {
	seen := make(map[string]struct{})
	for decoder.More() {
		token, err := decoder.Token()
		key, valid := token.(string)
		if err != nil || !valid {
			return errInvalidSpec
		}
		if _, duplicate := seen[key]; duplicate {
			return errInvalidSpec
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
		return errInvalidSpec
	}
	return nil
}
