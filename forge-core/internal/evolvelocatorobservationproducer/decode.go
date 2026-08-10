package evolvelocatorobservationproducer

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"strconv"
	"strings"
	"unicode/utf8"
)

const (
	maxJSONDepth       = 16
	maxObjectFields    = 64
	maxJSONItems       = 65_536
	maxJSONStringBytes = 1 << 20
)

// DecodeProduction accepts only exact compact canonical production bytes and
// revalidates every report/source/observation relationship.
func DecodeProduction(data []byte) (ProductionPackage, error) {
	if len(data) < 1 || len(data) > maxProductionBytes || !utf8.Valid(data) {
		return ProductionPackage{}, fmt.Errorf("production JSON must be 1..%d valid UTF-8 bytes", maxProductionBytes)
	}
	if err := validateStrictJSON(data); err != nil {
		return ProductionPackage{}, fmt.Errorf("strict production JSON: %w", err)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var value ProductionPackage
	if err := decoder.Decode(&value); err != nil {
		return ProductionPackage{}, fmt.Errorf("decode production JSON: %w", err)
	}
	if err := requireDecodeEOF(decoder); err != nil {
		return ProductionPackage{}, err
	}
	canonical, err := canonicalJSON(value)
	if err != nil {
		return ProductionPackage{}, err
	}
	if !bytes.Equal(data, canonical) {
		return ProductionPackage{}, fmt.Errorf("production JSON is not exact compact canonical bytes")
	}
	if err := validateProductionPackage(value); err != nil {
		return ProductionPackage{}, err
	}
	return cloneProductionPackage(value), nil
}

func validateStrictJSON(data []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	if err := walkJSONValue(decoder, 1); err != nil {
		return err
	}
	if _, err := decoder.Token(); err != io.EOF {
		if err == nil {
			return fmt.Errorf("multiple JSON values")
		}
		return fmt.Errorf("trailing JSON: %w", err)
	}
	return nil
}

func walkJSONValue(decoder *json.Decoder, depth int) error {
	if depth > maxJSONDepth {
		return fmt.Errorf("depth exceeds %d", maxJSONDepth)
	}
	token, err := decoder.Token()
	if err != nil {
		return err
	}
	switch value := token.(type) {
	case json.Delim:
		if value == '{' {
			return walkJSONObject(decoder, depth)
		}
		if value == '[' {
			return walkJSONArray(decoder, depth)
		}
		return fmt.Errorf("unexpected delimiter %q", value)
	case json.Number:
		return validateJSONInteger(value.String())
	case string:
		if len(value) > maxJSONStringBytes {
			return fmt.Errorf("string exceeds %d bytes", maxJSONStringBytes)
		}
		return validateCanonicalText(value)
	case bool, nil:
		return nil
	default:
		return fmt.Errorf("unsupported token %T", token)
	}
}

func walkJSONObject(decoder *json.Decoder, depth int) error {
	seen := make(map[string]struct{})
	for decoder.More() {
		if len(seen) == maxObjectFields {
			return fmt.Errorf("object exceeds %d fields", maxObjectFields)
		}
		token, err := decoder.Token()
		if err != nil {
			return err
		}
		key, ok := token.(string)
		if !ok || !canonicalObjectKey(key) {
			return fmt.Errorf("object key %q is not canonical snake_case", key)
		}
		if _, duplicate := seen[key]; duplicate {
			return fmt.Errorf("duplicate object key %q", key)
		}
		seen[key] = struct{}{}
		if err := walkJSONValue(decoder, depth+1); err != nil {
			return fmt.Errorf("field %q: %w", key, err)
		}
	}
	return closeDelimiter(decoder, '}')
}

func walkJSONArray(decoder *json.Decoder, depth int) error {
	count := 0
	for decoder.More() {
		if count == maxJSONItems {
			return fmt.Errorf("array exceeds %d items", maxJSONItems)
		}
		if err := walkJSONValue(decoder, depth+1); err != nil {
			return fmt.Errorf("array item %d: %w", count, err)
		}
		count++
	}
	return closeDelimiter(decoder, ']')
}

func closeDelimiter(decoder *json.Decoder, expected json.Delim) error {
	token, err := decoder.Token()
	if err != nil || token != expected {
		return fmt.Errorf("JSON compound is not closed with %q", expected)
	}
	return nil
}

func validateJSONInteger(raw string) error {
	if raw == "" || raw[0] == '+' || strings.ContainsAny(raw, ".eE") {
		return fmt.Errorf("number %q is not a canonical signed int64", raw)
	}
	value, err := strconv.ParseInt(raw, 10, 64)
	if err != nil || strconv.FormatInt(value, 10) != raw {
		return fmt.Errorf("number %q is not a canonical signed int64", raw)
	}
	return nil
}

func canonicalObjectKey(value string) bool {
	if value == "" || value[0] < 'a' || value[0] > 'z' {
		return false
	}
	for _, character := range value[1:] {
		if character < 'a' || character > 'z' {
			if character < '0' || character > '9' {
				if character != '_' {
					return false
				}
			}
		}
	}
	return true
}

func requireDecodeEOF(decoder *json.Decoder) error {
	var extra any
	if err := decoder.Decode(&extra); err != io.EOF {
		if err == nil {
			return fmt.Errorf("production JSON has a trailing value")
		}
		return fmt.Errorf("decode trailing production JSON: %w", err)
	}
	return nil
}
