package governancecontract

import "fmt"

func validateRecordShape(root map[string]any, kind string) error {
	if err := requireKeys(root, "api_version", "integrity", "kind", "metadata", "spec", "status"); err != nil {
		return fmt.Errorf("record: %w", err)
	}
	if err := validateCommonShape(root); err != nil {
		return err
	}
	switch kind {
	case EvidenceKind:
		return validateEvidenceShape(root)
	case ClaimKind:
		return validateClaimShape(root)
	default:
		return fmt.Errorf("unsupported governance record kind %q", kind)
	}
}

func validateCommonShape(root map[string]any) error {
	metadata, err := objectField(root, "metadata")
	if err != nil {
		return err
	}
	metadataKeys := []string{"aggregate_id", "context_sha256", "created_at_unix_ms", "created_by", "policy_sha256", "project_id", "record_id", "scope", "sequence", "source_revision", "source_tree_sha256", "supersedes_record_ids"}
	if err := requireKeys(metadata, metadataKeys...); err != nil {
		return fmt.Errorf("metadata: %w", err)
	}
	creator, err := objectField(metadata, "created_by")
	if err != nil {
		return err
	}
	if err := requireKeys(creator, "authority_domain", "principal_id", "principal_type", "role", "run_id"); err != nil {
		return fmt.Errorf("created_by: %w", err)
	}
	if _, err := arrayField(metadata, "supersedes_record_ids"); err != nil {
		return err
	}
	integrity, err := objectField(root, "integrity")
	if err != nil {
		return err
	}
	return requireKeys(integrity, "canonical_sha256", "canonicalization")
}

func validateEvidenceShape(root map[string]any) error {
	spec, err := objectField(root, "spec")
	if err != nil {
		return err
	}
	keys := []string{"artifact_sha256", "collector", "content_role", "directness", "evidence_type", "locator", "observed_at_unix_ms", "sensitivity", "source_snapshot", "source_trust", "subjects"}
	if err := requireKeys(spec, keys...); err != nil {
		return fmt.Errorf("evidence spec: %w", err)
	}
	if err := validateEvidenceNestedShape(spec); err != nil {
		return err
	}
	return validateStatusShape(root)
}

func validateEvidenceNestedShape(spec map[string]any) error {
	collector, err := objectField(spec, "collector")
	if err != nil {
		return err
	}
	if err := requireKeys(collector, "collector_id", "collector_type", "collector_version", "parameters_sha256", "run_id"); err != nil {
		return fmt.Errorf("collector: %w", err)
	}
	locator, err := objectField(spec, "locator")
	if err != nil {
		return err
	}
	if err := requireKeys(locator, "content_sha256", "exit_code", "line_end", "line_start", "locator_ref", "locator_type"); err != nil {
		return fmt.Errorf("locator: %w", err)
	}
	snapshot, err := objectField(spec, "source_snapshot")
	if err != nil {
		return err
	}
	if err := requireKeys(snapshot, "snapshot_id", "snapshot_sha256", "snapshot_type"); err != nil {
		return fmt.Errorf("source_snapshot: %w", err)
	}
	_, err = arrayField(spec, "subjects")
	return err
}

func validateClaimShape(root map[string]any) error {
	spec, err := objectField(root, "spec")
	if err != nil {
		return err
	}
	keys := []string{"claim_type", "confidence_micros", "contradicting_evidence_record_ids", "decision_authority", "derived_from_claim_record_ids", "object_type", "object_value", "owner", "predicate", "queue_ref", "reasoning", "review_by_unix_ms", "subject", "supporting_evidence_record_ids", "validation_plan"}
	if err := requireKeys(spec, keys...); err != nil {
		return fmt.Errorf("claim spec: %w", err)
	}
	if err := validateClaimNestedShape(spec); err != nil {
		return err
	}
	return validateStatusShape(root)
}

func validateClaimNestedShape(spec map[string]any) error {
	owner, err := objectField(spec, "owner")
	if err != nil {
		return err
	}
	if err := requireKeys(owner, "principal_id", "principal_type"); err != nil {
		return fmt.Errorf("claim owner: %w", err)
	}
	for _, key := range []string{"contradicting_evidence_record_ids", "derived_from_claim_record_ids", "supporting_evidence_record_ids"} {
		if _, err := arrayField(spec, key); err != nil {
			return err
		}
	}
	if err := optionalObjectShape(spec, "decision_authority", []string{"adr_ref", "approval_ref"}); err != nil {
		return err
	}
	planKeys := []string{"due_at_unix_ms", "impact_if_false", "method", "owner_id", "required_evidence_types"}
	return optionalPlanShape(spec, planKeys)
}

func optionalPlanShape(spec map[string]any, keys []string) error {
	if spec["validation_plan"] == nil {
		return nil
	}
	plan, err := objectField(spec, "validation_plan")
	if err != nil {
		return err
	}
	if err := requireKeys(plan, keys...); err != nil {
		return fmt.Errorf("validation_plan: %w", err)
	}
	_, err = arrayField(plan, "required_evidence_types")
	return err
}

func optionalObjectShape(parent map[string]any, key string, keys []string) error {
	if parent[key] == nil {
		return nil
	}
	object, err := objectField(parent, key)
	if err != nil {
		return err
	}
	return requireKeys(object, keys...)
}

func validateStatusShape(root map[string]any) error {
	status, err := objectField(root, "status")
	if err != nil {
		return err
	}
	if err := requireKeys(status, "reason_codes", "state", "valid_from_unix_ms", "valid_until_unix_ms"); err != nil {
		return fmt.Errorf("status: %w", err)
	}
	_, err = arrayField(status, "reason_codes")
	return err
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
