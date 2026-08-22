package transitionreceiptcontract

import "fmt"

func validateBindings(node map[string]any, label string) error {
	keys := []string{"artifacts", "context_sha256", "impact_sha256", "plan_sha256",
		"policy_sha256", "risk_sha256", "source_revision", "source_tree_sha256"}
	if err := requireKeys(node, keys...); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	artifacts, err := arrayValue(node, "artifacts")
	if err != nil {
		return err
	}
	if err := validateReferenceArray(artifacts, label+".artifacts", 32, validateArtifact); err != nil {
		return err
	}
	for _, key := range []string{"context_sha256", "policy_sha256", "source_tree_sha256"} {
		value, valueErr := stringValue(node, key)
		if valueErr != nil || validateHash(value, label+"."+key) != nil {
			return fmt.Errorf("%s.%s is invalid", label, key)
		}
	}
	for _, key := range []string{"impact_sha256", "plan_sha256", "risk_sha256"} {
		value, valueErr := nullableStringValue(node, key)
		if valueErr != nil || (value != nil && validateHash(*value, label+"."+key) != nil) {
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
	for key, maximum := range map[string]int{"artifact_kind": maxShortBytes, "artifact_ref": maxReferenceBytes} {
		value, err := stringValue(node, key)
		if err != nil || validateText(value, label+"."+key, maximum) != nil {
			return fmt.Errorf("%s.%s is invalid", label, key)
		}
	}
	digest, err := stringValue(node, "artifact_sha256")
	if err != nil || validateHash(digest, label+".artifact_sha256") != nil {
		return fmt.Errorf("%s.artifact_sha256 is invalid", label)
	}
	return nil
}
