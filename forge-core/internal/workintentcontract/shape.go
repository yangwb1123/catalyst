package workintentcontract

import "fmt"

var (
	topFields = []string{"api_version", "attestations", "binding", "canonicalization",
		"declared_at_unix_ms", "declared_owner", "freshness", "intent", "kind",
		"materiality", "origin", "references", "requester", "status",
		"work_intent_id", "work_intent_sha256"}
	attestationFields = []string{"approval_attestation", "authentication_attestation",
		"authority_attestation", "completion_attestation", "effect_attestation",
		"execution_attestation", "freshness_attestation", "materiality_attestation",
		"ownership_attestation", "permission_attestation", "persistence_attestation",
		"reference_resolution_attestation", "scope_attestation", "truth_attestation"}
	principalFields = []string{"authority_domain", "principal_id", "principal_type"}
)

func validateRawShape(root map[string]any) error {
	if err := requireFields(root, topFields...); err != nil {
		return fmt.Errorf("WorkIntent: %w", err)
	}
	if err := validateRawSimpleObjects(root); err != nil {
		return err
	}
	if err := validateRawPrincipalValue(root["requester"], "requester"); err != nil {
		return err
	}
	if root["declared_owner"] != nil {
		if err := validateRawPrincipalValue(root["declared_owner"], "declared_owner"); err != nil {
			return err
		}
	}
	return validateRawReferences(root["references"])
}

func validateRawSimpleObjects(root map[string]any) error {
	checks := []struct {
		key    string
		fields []string
	}{
		{"attestations", attestationFields},
		{"binding", []string{"change_id", "project_id", "run_id"}},
		{"intent", []string{"deadline_unix_ms", "external_constraints", "goal", "non_goals",
			"open_questions", "scope", "success_signals", "work_type"}},
		{"materiality", []string{"basis", "level"}},
		{"origin", []string{"origin_kind", "origin_ref"}},
	}
	for _, check := range checks {
		object, err := rawObject(root[check.key], check.key)
		if err != nil {
			return err
		}
		if err := requireFields(object, check.fields...); err != nil {
			return fmt.Errorf("%s: %w", check.key, err)
		}
	}
	return nil
}

func validateRawPrincipalValue(value any, label string) error {
	object, err := rawObject(value, label)
	if err != nil {
		return err
	}
	if err := requireFields(object, principalFields...); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	return nil
}

func validateRawReferences(value any) error {
	refs, err := rawObject(value, "references")
	if err != nil {
		return err
	}
	fields := []string{"claim_record_refs", "evidence_record_refs",
		"local_artifact_declarations", "local_source_snapshot_declaration"}
	if err := requireFields(refs, fields...); err != nil {
		return fmt.Errorf("references: %w", err)
	}
	if err := validateRawObjectArray(refs["claim_record_refs"], "claim_record_refs",
		[]string{"canonical_sha256", "record_id"}); err != nil {
		return err
	}
	if err := validateRawObjectArray(refs["evidence_record_refs"], "evidence_record_refs",
		[]string{"canonical_sha256", "record_id"}); err != nil {
		return err
	}
	if err := validateRawObjectArray(refs["local_artifact_declarations"],
		"local_artifact_declarations",
		[]string{"artifact_kind", "artifact_ref", "artifact_sha256"}); err != nil {
		return err
	}
	return validateRawSnapshot(refs["local_source_snapshot_declaration"])
}

func validateRawObjectArray(value any, label string, fields []string) error {
	array, ok := value.([]any)
	if !ok {
		return fmt.Errorf("%s must be an array", label)
	}
	for index, item := range array {
		object, err := rawObject(item, fmt.Sprintf("%s[%d]", label, index))
		if err != nil {
			return err
		}
		if err := requireFields(object, fields...); err != nil {
			return fmt.Errorf("%s[%d]: %w", label, index, err)
		}
	}
	return nil
}

func validateRawSnapshot(value any) error {
	if value == nil {
		return nil
	}
	object, err := rawObject(value, "local_source_snapshot_declaration")
	if err != nil {
		return err
	}
	if err := requireFields(object, "snapshot_id", "snapshot_sha256", "snapshot_type"); err != nil {
		return fmt.Errorf("local_source_snapshot_declaration: %w", err)
	}
	return nil
}

func rawObject(value any, label string) (map[string]any, error) {
	object, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("%s must be an object", label)
	}
	return object, nil
}

func requireFields(object map[string]any, fields ...string) error {
	if len(object) != len(fields) {
		return fmt.Errorf("fields mismatch")
	}
	for _, field := range fields {
		if _, exists := object[field]; !exists {
			return fmt.Errorf("missing field %q", field)
		}
	}
	return nil
}
