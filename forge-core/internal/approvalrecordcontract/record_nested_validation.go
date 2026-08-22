package approvalrecordcontract

import (
	"fmt"
	"regexp"
)

var identifierPattern = regexp.MustCompile(`^[a-z][a-z0-9._-]{0,159}$`)

func validateScope(node map[string]any, label string) error {
	if err := requireKeys(node, "change_id", "effect_id", "environment_class", "environment_id",
		"gate_id", "materiality_level", "project_id", "scope_type"); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	for _, key := range []string{"change_id", "environment_id", "project_id"} {
		value, err := stringValue(node, key)
		if err != nil || validateText(value, label+"."+key, maxShortBytes) != nil {
			return fmt.Errorf("%s.%s is invalid", label, key)
		}
	}
	environment, envErr := stringValue(node, "environment_class")
	materiality, materialityErr := stringValue(node, "materiality_level")
	scopeType, scopeErr := stringValue(node, "scope_type")
	if envErr != nil || validateEnum(environment, label+".environment_class", "development", "local",
		"production", "staging", "test") != nil {
		return fmt.Errorf("%s.environment_class is invalid", label)
	}
	if materialityErr != nil || validateEnum(materiality, label+".materiality_level",
		"L0", "L1", "L2", "L3", "L4") != nil {
		return fmt.Errorf("%s.materiality_level is invalid", label)
	}
	if scopeErr != nil || validateEnum(scopeType, label+".scope_type", "effect", "gate") != nil {
		return fmt.Errorf("%s.scope_type is invalid", label)
	}
	return validateScopeDiscriminator(node, label, scopeType)
}

func validateScopeDiscriminator(node map[string]any, label, scopeType string) error {
	effect, effectErr := nullableStringValue(node, "effect_id")
	gate, gateErr := nullableStringValue(node, "gate_id")
	if effectErr != nil || gateErr != nil {
		return fmt.Errorf("%s effect_id/gate_id is invalid", label)
	}
	if scopeType == "gate" {
		if gate == nil || validateText(*gate, label+".gate_id", maxShortBytes) != nil || effect != nil {
			return fmt.Errorf("%s gate scope requires gate_id and effect_id=null", label)
		}
		return nil
	}
	if effect == nil || validateEnum(*effect, label+".effect_id", effects...) != nil || gate != nil {
		return fmt.Errorf("%s effect scope requires a frozen effect_id and gate_id=null", label)
	}
	return nil
}

func validateBindings(node map[string]any, label string) error {
	if err := requireKeys(node, "artifacts", "context_sha256", "impact_sha256", "plan_sha256",
		"policy_sha256", "risk_sha256", "source_revision", "source_tree_sha256"); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	artifacts, err := arrayValue(node, "artifacts")
	if err != nil || len(artifacts) < 1 || len(artifacts) > 32 {
		return fmt.Errorf("%s.artifacts must contain 1..32 items", label)
	}
	for index, value := range artifacts {
		artifact, ok := value.(map[string]any)
		if !ok || validateArtifact(artifact, fmt.Sprintf("%s.artifacts[%d]", label, index)) != nil {
			return fmt.Errorf("%s.artifacts[%d] is invalid", label, index)
		}
	}
	if err := validateSortedNodes(artifacts, canonicalNodeKey); err != nil {
		return fmt.Errorf("%s.artifacts: %w", label, err)
	}
	for _, key := range []string{"context_sha256", "impact_sha256", "plan_sha256", "policy_sha256",
		"risk_sha256", "source_tree_sha256"} {
		value, valueErr := stringValue(node, key)
		if valueErr != nil || validateHash(value, label+"."+key) != nil {
			return fmt.Errorf("%s.%s is invalid", label, key)
		}
	}
	revision, err := stringValue(node, "source_revision")
	if err != nil || validateText(revision, label+".source_revision", maxShortBytes) != nil {
		return fmt.Errorf("%s.source_revision is invalid", label)
	}
	return nil
}

func validateArtifact(node map[string]any, label string) error {
	if err := requireKeys(node, "artifact_kind", "artifact_ref", "artifact_sha256"); err != nil {
		return err
	}
	for key, maximum := range map[string]int{"artifact_kind": maxShortBytes, "artifact_ref": 4096} {
		value, err := stringValue(node, key)
		if err != nil || validateText(value, label+"."+key, maximum) != nil {
			return fmt.Errorf("%s.%s is invalid", label, key)
		}
	}
	hash, err := stringValue(node, "artifact_sha256")
	if err != nil {
		return err
	}
	return validateHash(hash, label+".artifact_sha256")
}

func validateDecisionBasis(node map[string]any) error {
	if err := requireKeys(node, "rationale_ref", "rationale_sha256", "reason_codes"); err != nil {
		return err
	}
	reference, refErr := stringValue(node, "rationale_ref")
	hash, hashErr := stringValue(node, "rationale_sha256")
	if refErr != nil || validateText(reference, "decision_basis.rationale_ref", 4096) != nil ||
		hashErr != nil || validateHash(hash, "decision_basis.rationale_sha256") != nil {
		return fmt.Errorf("decision_basis rationale is invalid")
	}
	reasons, err := readStringArray(node, "reason_codes", 1, 16)
	if err != nil {
		return err
	}
	for _, reason := range reasons {
		if !identifierPattern.MatchString(reason) {
			return fmt.Errorf("decision_basis reason code is not a lowercase stable identifier")
		}
	}
	return nil
}
