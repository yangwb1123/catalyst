package evolvescan

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
)

func validateReportJSONShape(payload []byte) error {
	if err := rejectDuplicateJSONKeys(payload); err != nil {
		return err
	}
	top, err := strictObject(payload, "report",
		[]string{"version", "depth", "dimensions", "opportunities"}, nil)
	if err != nil {
		return err
	}
	dimensions, err := strictArray(top["dimensions"], "dimensions")
	if err != nil {
		return err
	}
	for i, raw := range dimensions {
		fields, fieldErr := strictObject(raw, fmt.Sprintf("dimension[%d]", i),
			[]string{"name", "status", "evidence"}, []string{"unavailable_reason"})
		if fieldErr != nil {
			return fieldErr
		}
		if err := validateEvidenceJSON(fields["evidence"], fmt.Sprintf("dimension[%d].evidence", i)); err != nil {
			return err
		}
	}
	opportunities, err := strictArray(top["opportunities"], "opportunities")
	if err != nil {
		return err
	}
	for i, raw := range opportunities {
		fields, fieldErr := strictObject(raw, fmt.Sprintf("opportunity[%d]", i),
			[]string{"id", "dimension", "title", "evidence", "obvious"}, []string{"candidate_task"})
		if fieldErr != nil {
			return fieldErr
		}
		if err := validateEvidenceJSON(fields["evidence"], fmt.Sprintf("opportunity[%d].evidence", i)); err != nil {
			return err
		}
	}
	return nil
}

func validateEvidenceJSON(raw json.RawMessage, label string) error {
	records, err := strictArray(raw, label)
	if err != nil {
		return err
	}
	for i, record := range records {
		if _, err := strictObject(record, fmt.Sprintf("%s[%d]", label, i),
			[]string{"path", "detail"}, []string{"line"}); err != nil {
			return err
		}
	}
	return nil
}

func strictObject(raw []byte, label string, required, optional []string) (map[string]json.RawMessage, error) {
	trimmed := bytes.TrimSpace(raw)
	if len(trimmed) == 0 || trimmed[0] != '{' {
		return nil, fmt.Errorf("%s must be a JSON object", label)
	}
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(trimmed, &fields); err != nil {
		return nil, fmt.Errorf("%s: %w", label, err)
	}
	allowed := make(map[string]bool, len(required)+len(optional))
	for _, name := range append(append([]string(nil), required...), optional...) {
		allowed[name] = true
	}
	for name := range fields {
		if !allowed[name] {
			return nil, fmt.Errorf("%s has unknown field %q", label, name)
		}
	}
	for _, name := range required {
		if _, ok := fields[name]; !ok {
			return nil, fmt.Errorf("%s is missing required field %q", label, name)
		}
	}
	return fields, nil
}

func strictArray(raw []byte, label string) ([]json.RawMessage, error) {
	trimmed := bytes.TrimSpace(raw)
	if len(trimmed) == 0 || trimmed[0] != '[' {
		return nil, fmt.Errorf("%s must be a JSON array, not null or an omitted value", label)
	}
	var records []json.RawMessage
	if err := json.Unmarshal(trimmed, &records); err != nil {
		return nil, fmt.Errorf("%s: %w", label, err)
	}
	return records, nil
}

func rejectDuplicateJSONKeys(payload []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(payload))
	decoder.UseNumber()
	if err := walkJSONValue(decoder); err != nil {
		return fmt.Errorf("strict JSON: %w", err)
	}
	if _, err := decoder.Token(); err != io.EOF {
		if err == nil {
			return fmt.Errorf("strict JSON: multiple JSON values")
		}
		return fmt.Errorf("strict JSON: trailing data: %w", err)
	}
	return nil
}

func walkJSONValue(decoder *json.Decoder) error {
	token, err := decoder.Token()
	if err != nil {
		return err
	}
	delim, compound := token.(json.Delim)
	if !compound {
		return nil
	}
	switch delim {
	case '{':
		seen := map[string]bool{}
		for decoder.More() {
			keyToken, err := decoder.Token()
			if err != nil {
				return err
			}
			key, ok := keyToken.(string)
			if !ok {
				return fmt.Errorf("object key is not a string")
			}
			if seen[key] {
				return fmt.Errorf("duplicate JSON key %q", key)
			}
			seen[key] = true
			if err := walkJSONValue(decoder); err != nil {
				return err
			}
		}
	case '[':
		for decoder.More() {
			if err := walkJSONValue(decoder); err != nil {
				return err
			}
		}
	default:
		return fmt.Errorf("unexpected JSON delimiter %q", delim)
	}
	_, err = decoder.Token()
	return err
}
