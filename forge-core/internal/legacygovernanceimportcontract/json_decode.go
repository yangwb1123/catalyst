package legacygovernanceimportcontract

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"strconv"
	"unicode/utf8"
)

type numberLexeme string

func parseStrictJSON(data []byte, maximum int, rawNumbers bool) (any, error) {
	if len(data) == 0 || len(data) > maximum {
		return nil, fmt.Errorf("JSON byte length must be 1..%d", maximum)
	}
	if !utf8.Valid(data) {
		return nil, fmt.Errorf("JSON is not valid UTF-8")
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	value, err := decodeNode(decoder, 1, rawNumbers)
	if err != nil {
		return nil, err
	}
	if token, err := decoder.Token(); err != io.EOF {
		if err == nil {
			return nil, fmt.Errorf("trailing JSON token %v", token)
		}
		return nil, fmt.Errorf("trailing JSON: %w", err)
	}
	return value, nil
}

func decodeNode(decoder *json.Decoder, depth int, rawNumbers bool) (any, error) {
	if depth > maxJSONDepth {
		return nil, fmt.Errorf("JSON depth exceeds %d", maxJSONDepth)
	}
	token, err := decoder.Token()
	if err != nil {
		return nil, err
	}
	switch value := token.(type) {
	case json.Delim:
		return decodeContainer(decoder, value, depth, rawNumbers)
	case json.Number:
		if rawNumbers {
			return numberLexeme(value.String()), nil
		}
		return parseCanonicalInt(value.String())
	case string:
		return value, validateWireString(value, maxStringBytes, true)
	case bool, nil:
		return value, nil
	default:
		return nil, fmt.Errorf("unsupported JSON token %T", token)
	}
}

func decodeContainer(decoder *json.Decoder, delimiter json.Delim, depth int,
	rawNumbers bool) (any, error) {
	switch delimiter {
	case '{':
		return decodeObject(decoder, depth, rawNumbers)
	case '[':
		return decodeArray(decoder, depth, rawNumbers)
	default:
		return nil, fmt.Errorf("unexpected delimiter %q", delimiter)
	}
}

func decodeObject(decoder *json.Decoder, depth int,
	rawNumbers bool) (map[string]any, error) {
	object := make(map[string]any)
	for decoder.More() {
		if len(object) >= maxObjectFields {
			return nil, fmt.Errorf("object field count exceeds %d", maxObjectFields)
		}
		keyToken, err := decoder.Token()
		if err != nil {
			return nil, err
		}
		key, ok := keyToken.(string)
		if !ok || (!isASCIISnakeKey(key) && !(rawNumbers && key == "_format")) {
			return nil, fmt.Errorf("object key %q is not ASCII snake_case", key)
		}
		if _, exists := object[key]; exists {
			return nil, fmt.Errorf("duplicate JSON key %q", key)
		}
		value, err := decodeNode(decoder, depth+1, rawNumbers)
		if err != nil {
			return nil, fmt.Errorf("field %q: %w", key, err)
		}
		object[key] = value
	}
	if token, err := decoder.Token(); err != nil || token != json.Delim('}') {
		return nil, fmt.Errorf("object is not closed")
	}
	return object, nil
}

func decodeArray(decoder *json.Decoder, depth int, rawNumbers bool) ([]any, error) {
	array := make([]any, 0)
	for decoder.More() {
		if len(array) >= maxArrayItems {
			return nil, fmt.Errorf("array item count exceeds %d", maxArrayItems)
		}
		value, err := decodeNode(decoder, depth+1, rawNumbers)
		if err != nil {
			return nil, fmt.Errorf("array item %d: %w", len(array), err)
		}
		array = append(array, value)
	}
	if token, err := decoder.Token(); err != nil || token != json.Delim(']') {
		return nil, fmt.Errorf("array is not closed")
	}
	return array, nil
}

func parseCanonicalInt(raw string) (int64, error) {
	if raw == "" || raw[0] == '+' || bytes.ContainsAny([]byte(raw), ".eE") {
		return 0, fmt.Errorf("number %q is not a canonical signed int64", raw)
	}
	value, err := strconv.ParseInt(raw, 10, 64)
	if err != nil || strconv.FormatInt(value, 10) != raw {
		return 0, fmt.Errorf("number %q is not a canonical signed int64", raw)
	}
	return value, nil
}

func validateWireString(value string, maximum int, nonempty bool) error {
	if !utf8.ValidString(value) || len(value) > maximum || (nonempty && value == "") {
		return fmt.Errorf("string must be valid UTF-8 with byte length in its bound")
	}
	for _, runeValue := range value {
		if forbiddenRune(runeValue) {
			return fmt.Errorf("string contains forbidden Unicode U+%04X", runeValue)
		}
	}
	return nil
}

func forbiddenRune(value rune) bool {
	if value <= 0x1f || value == 0x7f || value == 0x2028 || value == 0x2029 {
		return true
	}
	return value == 0x061c || value == 0x200e || value == 0x200f ||
		(value >= 0x202a && value <= 0x202e) || (value >= 0x2066 && value <= 0x2069)
}

func isASCIISnakeKey(value string) bool {
	if value == "" || value[0] < 'a' || value[0] > 'z' {
		return false
	}
	for index := 1; index < len(value); index++ {
		character := value[index]
		if (character < 'a' || character > 'z') &&
			(character < '0' || character > '9') && character != '_' {
			return false
		}
	}
	return true
}
