package persist

import (
	"bytes"
	"encoding/json"
	"fmt"
)

// rejectDuplicateCheckpointFields prevents encoding/json's last-key-wins
// behavior from making a v4 recovery binding ambiguous.
func rejectDuplicateCheckpointFields(data []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	token, err := decoder.Token()
	if err != nil {
		return fmt.Errorf("persist: checkpoint fields: %w", err)
	}
	if delimiter, ok := token.(json.Delim); !ok || delimiter != '{' {
		return fmt.Errorf("persist: checkpoint must be a JSON object")
	}
	seen := make(map[string]struct{})
	for decoder.More() {
		token, err := decoder.Token()
		if err != nil {
			return fmt.Errorf("persist: checkpoint field: %w", err)
		}
		name, ok := token.(string)
		if !ok {
			return fmt.Errorf("persist: checkpoint contains a non-string field name")
		}
		if _, duplicate := seen[name]; duplicate {
			return fmt.Errorf("persist: checkpoint contains duplicate field %q", name)
		}
		seen[name] = struct{}{}
		var value json.RawMessage
		if err := decoder.Decode(&value); err != nil {
			return fmt.Errorf("persist: checkpoint field %q: %w", name, err)
		}
	}
	if _, err := decoder.Token(); err != nil {
		return fmt.Errorf("persist: checkpoint close: %w", err)
	}
	return nil
}

func validateCheckpointReferenceObjects(fields map[string]json.RawMessage) error {
	for _, name := range []string{
		"phase_semantic_outputs", "phase_output_receipts",
		"stage_output_receipts", "stage_approval_contexts",
	} {
		if err := validateSortedUniqueJSONObject(fields[name], name); err != nil {
			return err
		}
	}
	return nil
}

func validateSortedUniqueJSONObject(data []byte, name string) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	token, err := decoder.Token()
	if err != nil {
		return fmt.Errorf("persist: checkpoint %s: %w", name, err)
	}
	if delimiter, ok := token.(json.Delim); !ok || delimiter != '{' {
		return fmt.Errorf("persist: checkpoint %s must be an object", name)
	}
	prior := ""
	for decoder.More() {
		token, err := decoder.Token()
		key, ok := token.(string)
		if err != nil || !ok {
			return fmt.Errorf("persist: checkpoint %s has invalid key", name)
		}
		if prior != "" && key <= prior {
			return fmt.Errorf("persist: checkpoint %s keys must be sorted and unique", name)
		}
		prior = key
		var value json.RawMessage
		if err := decoder.Decode(&value); err != nil {
			return fmt.Errorf("persist: checkpoint %s value: %w", name, err)
		}
		if bytes.Equal(bytes.TrimSpace(value), []byte("null")) {
			return fmt.Errorf("persist: checkpoint %s value for %q is null", name, key)
		}
	}
	_, err = decoder.Token()
	return err
}
