package commandobservationevidencecontract

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
	if err := requireKeys(observation, "api_version", "canonicalization", "command", "ended_at_unix_ms", "evidence_type", "producer", "source", "started_at_unix_ms", "streams", "termination"); err != nil {
		return fmt.Errorf("observation: %w", err)
	}
	if err := validateCommandShape(observation); err != nil {
		return err
	}
	if err := validateProducerAndSourceShape(observation); err != nil {
		return err
	}
	if err := validateStreamsShape(observation); err != nil {
		return err
	}
	termination, err := objectField(observation, "termination")
	if err != nil {
		return err
	}
	if err := requireKeys(termination, "exit_code", "kind"); err != nil {
		return fmt.Errorf("termination: %w", err)
	}
	return nil
}

func validateCommandShape(observation map[string]any) error {
	command, err := objectField(observation, "command")
	if err != nil {
		return err
	}
	if err := requireKeys(command, "argv", "cwd", "environment_sha256", "stdin_bytes", "stdin_sha256", "timeout_ms", "tool_snapshot_sha256"); err != nil {
		return fmt.Errorf("command: %w", err)
	}
	_, err = arrayField(command, "argv")
	return err
}

func validateProducerAndSourceShape(observation map[string]any) error {
	producer, err := objectField(observation, "producer")
	if err != nil {
		return err
	}
	if err := requireKeys(producer, "producer_id", "producer_type", "producer_version", "run_id"); err != nil {
		return fmt.Errorf("producer: %w", err)
	}
	source, err := objectField(observation, "source")
	if err != nil {
		return err
	}
	if err := requireKeys(source, "source_revision", "source_tree_sha256"); err != nil {
		return fmt.Errorf("source: %w", err)
	}
	return nil
}

func validateStreamsShape(observation map[string]any) error {
	streams, err := objectField(observation, "streams")
	if err != nil {
		return err
	}
	if err := requireKeys(streams, "combined", "stderr", "stdout"); err != nil {
		return fmt.Errorf("streams: %w", err)
	}
	for _, name := range []string{"combined", "stderr", "stdout"} {
		stream, err := objectField(streams, name)
		if err != nil {
			return err
		}
		if err := requireKeys(stream, "bytes", "retained_bytes", "retained_sha256", "sha256"); err != nil {
			return fmt.Errorf("stream %s: %w", name, err)
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
