package cognitiveatomcontract

import "fmt"

func validateAtomShape(root map[string]any) error {
	if err := requireKeys(root, "api_version", "integrity", "kind", "metadata", "source", "spec"); err != nil {
		return fmt.Errorf("atom: %w", err)
	}
	if err := validateEnvelopeShape(root); err != nil {
		return err
	}
	return validateSpecShape(root)
}

func validateEnvelopeShape(root map[string]any) error {
	integrity, err := objectField(root, "integrity")
	if err != nil {
		return err
	}
	if err := requireKeys(integrity, "canonical_sha256", "canonicalization"); err != nil {
		return fmt.Errorf("integrity: %w", err)
	}
	metadata, err := objectField(root, "metadata")
	if err != nil {
		return err
	}
	if err := requireKeys(metadata, "atom_id", "context_sha256", "policy_sha256", "project_id", "scope", "source_revision", "source_tree_sha256", "task_id"); err != nil {
		return fmt.Errorf("metadata: %w", err)
	}
	source, err := objectField(root, "source")
	if err != nil {
		return err
	}
	if err := requireKeys(source, "canonical_sha256", "claim_aggregate_id", "claim_record_id", "claim_sequence", "closure_byte_count", "closure_record_count", "closure_sha256", "record_kind"); err != nil {
		return fmt.Errorf("source: %w", err)
	}
	return nil
}

func validateSpecShape(root map[string]any) error {
	spec, err := objectField(root, "spec")
	if err != nil {
		return err
	}
	if err := requireKeys(spec, "atom_type", "authority_ref", "contradicting_evidence_record_ids", "derived_from_claim_record_ids", "epistemic_state", "hardness", "instruction_allowed", "projection_confidence_micros", "projection_mode", "proposition", "supporting_evidence_record_ids", "validity"); err != nil {
		return fmt.Errorf("spec: %w", err)
	}
	for _, key := range []string{"contradicting_evidence_record_ids", "derived_from_claim_record_ids", "supporting_evidence_record_ids"} {
		if _, err := arrayField(spec, key); err != nil {
			return err
		}
	}
	proposition, err := objectField(spec, "proposition")
	if err != nil {
		return err
	}
	if err := requireKeys(proposition, "object_type", "object_value", "predicate", "subject"); err != nil {
		return fmt.Errorf("proposition: %w", err)
	}
	validity, err := objectField(spec, "validity")
	if err != nil {
		return err
	}
	if err := requireKeys(validity, "valid_from_unix_ms", "valid_until_unix_ms"); err != nil {
		return fmt.Errorf("validity: %w", err)
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
