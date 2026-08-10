package evolverepolocatorevidencecontract

import "fmt"

func validateRequestShape(root map[string]any) error {
	if err := requireKeys(root, "api_version", "binding", "canonicalization", "observation"); err != nil {
		return fmt.Errorf("request: %w", err)
	}
	binding, err := objectField(root, "binding")
	if err != nil {
		return err
	}
	if err := requireKeys(binding, "aggregate_id", "context_sha256", "policy_sha256", "project_id", "scope", "sensitivity", "sequence", "subjects", "supersedes_record_ids"); err != nil {
		return fmt.Errorf("binding: %w", err)
	}
	if _, err := arrayField(binding, "subjects"); err != nil {
		return err
	}
	if _, err := arrayField(binding, "supersedes_record_ids"); err != nil {
		return err
	}
	observation, err := objectField(root, "observation")
	if err != nil {
		return err
	}
	return validateObservationShape(observation)
}

func validateObservationShape(observation map[string]any) error {
	if err := requireKeys(observation, "api_version", "canonicalization", "content", "locator", "observed_at_unix_ms", "producer", "scan_context", "source"); err != nil {
		return fmt.Errorf("observation: %w", err)
	}
	objects := []struct {
		name string
		keys []string
	}{
		{"content", []string{"bytes", "sha256"}},
		{"locator", []string{"detail", "line", "path"}},
		{"producer", []string{"parameters_sha256", "producer_id", "producer_type", "producer_version", "run_id"}},
		{"scan_context", []string{"contract", "depth", "dimension", "opportunity_id", "relation", "report_sha256"}},
		{"source", []string{"source_revision", "source_tree_sha256"}},
	}
	for _, expected := range objects {
		object, err := objectField(observation, expected.name)
		if err != nil {
			return err
		}
		if err := requireKeys(object, expected.keys...); err != nil {
			return fmt.Errorf("%s: %w", expected.name, err)
		}
	}
	return nil
}

func requireKeys(object map[string]any, keys ...string) error {
	if len(object) != len(keys) {
		return fmt.Errorf("has %d fields; expected exactly %d", len(object), len(keys))
	}
	for _, key := range keys {
		if _, exists := object[key]; !exists {
			return fmt.Errorf("missing required field %q", key)
		}
	}
	return nil
}

func objectField(parent map[string]any, key string) (map[string]any, error) {
	value, exists := parent[key]
	if !exists {
		return nil, fmt.Errorf("missing required field %q", key)
	}
	object, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("field %q must be an object", key)
	}
	return object, nil
}

func arrayField(parent map[string]any, key string) ([]any, error) {
	value, exists := parent[key]
	if !exists {
		return nil, fmt.Errorf("missing required field %q", key)
	}
	array, ok := value.([]any)
	if !ok {
		return nil, fmt.Errorf("field %q must be an array", key)
	}
	return array, nil
}
