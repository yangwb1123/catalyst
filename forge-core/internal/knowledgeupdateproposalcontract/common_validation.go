package knowledgeupdateproposalcontract

import "fmt"

func validatePrincipal(node map[string]any, label string) error {
	if err := requireKeys(node, "authority_domain", "principal_id", "principal_type"); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	for _, key := range []string{"authority_domain", "principal_id"} {
		value, err := stringValue(node, key)
		if err != nil || validateText(value, label+"."+key, maxShortBytes) != nil {
			return fmt.Errorf("%s.%s is invalid", label, key)
		}
	}
	principalType, err := stringValue(node, "principal_type")
	if err != nil {
		return err
	}
	return validateEnum(principalType, label+".principal_type", "agent", "human", "operator", "service")
}

func validateTaskBinding(node map[string]any, label string) error {
	keys := []string{"attempt_id", "change_id", "environment_class", "environment_id", "node_id",
		"project_id", "role", "run_id", "target_id", "task_id"}
	if err := requireKeys(node, keys...); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	for _, key := range []string{"change_id", "environment_id", "node_id", "project_id", "role", "run_id", "task_id"} {
		value, err := stringValue(node, key)
		if err != nil || validateText(value, label+"."+key, maxShortBytes) != nil {
			return fmt.Errorf("%s.%s is invalid", label, key)
		}
	}
	for _, key := range []string{"attempt_id", "target_id"} {
		value, err := nullableStringValue(node, key)
		if err != nil || (value != nil && validateText(*value, label+"."+key, maxShortBytes) != nil) {
			return fmt.Errorf("%s.%s is invalid", label, key)
		}
	}
	class, err := stringValue(node, "environment_class")
	if err != nil {
		return err
	}
	return validateEnum(class, label+".environment_class", "development", "local", "production", "staging", "test")
}

func validateGrantRef(node map[string]any, label string) error {
	if err := requireKeys(node, "authority_domain", "grant_id", "grant_sha256"); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	identifier, idErr := stringValue(node, "grant_id")
	digest, hashErr := stringValue(node, "grant_sha256")
	domain, domainErr := stringValue(node, "authority_domain")
	if hashErr != nil || validateHash(digest, label+".grant_sha256") != nil ||
		idErr != nil || identifier != "capability-grant-"+digest ||
		domainErr != nil || validateText(domain, label+".authority_domain", maxShortBytes) != nil {
		return fmt.Errorf("%s identity is invalid", label)
	}
	return nil
}

func validateBindings(node map[string]any, label string) error {
	keys := []string{"artifacts", "context_sha256", "impact_sha256", "plan_sha256",
		"policy_sha256", "risk_sha256", "source_revision", "source_tree_sha256"}
	if err := requireKeys(node, keys...); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	artifacts, err := arrayValue(node, "artifacts")
	if err != nil || len(artifacts) > maxArtifacts {
		return fmt.Errorf("%s.artifacts must contain 0..%d items", label, maxArtifacts)
	}
	for index, value := range artifacts {
		artifact, ok := value.(map[string]any)
		if !ok || validateArtifact(artifact, fmt.Sprintf("%s.artifacts[%d]", label, index)) != nil {
			return fmt.Errorf("%s.artifacts[%d] is invalid", label, index)
		}
	}
	if err := validateSortedNodes(artifacts, label+".artifacts"); err != nil {
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

func validateKnowledgeScope(node map[string]any, label string) error {
	if err := requireKeys(node, "object_kind", "object_ref", "object_scope_sha256", "scope_kind"); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	if err := requireStringLiteral(node, "scope_kind", "governance_object"); err != nil {
		return err
	}
	if err := requireStringLiteral(node, "object_kind", "knowledge"); err != nil {
		return err
	}
	reference, err := stringValue(node, "object_ref")
	if err != nil || validateIdentifier(reference, label+".object_ref") != nil {
		return fmt.Errorf("%s.object_ref is invalid", label)
	}
	digest, err := stringValue(node, "object_scope_sha256")
	if err != nil || validateHash(digest, label+".object_scope_sha256") != nil {
		return fmt.Errorf("%s.object_scope_sha256 is invalid", label)
	}
	return nil
}
