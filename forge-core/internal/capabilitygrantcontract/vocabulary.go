package capabilitygrantcontract

import (
	"fmt"
	"slices"
)

func validateVocabulary(vocabulary map[string]any) error {
	if err := requireKeys(vocabulary, "api_version", "canonicalization", "effects", "kind",
		"vocabulary_sha256"); err != nil {
		return fmt.Errorf("effect vocabulary: %w", err)
	}
	if err := requireStringLiteral(vocabulary, "api_version",
		"forgeos.governance.effect-vocabulary/v1"); err != nil {
		return err
	}
	if err := requireStringLiteral(vocabulary, "canonicalization", "forgeos.canonical-json/v1"); err != nil {
		return err
	}
	if err := requireStringLiteral(vocabulary, "kind", "EffectVocabulary"); err != nil {
		return err
	}
	effects, err := arrayValue(vocabulary, "effects")
	if err != nil || len(effects) != len(frozenEffects) {
		return fmt.Errorf("effects must contain the frozen %d descriptors", len(frozenEffects))
	}
	for index, expected := range frozenEffects {
		node, ok := effects[index].(map[string]any)
		if !ok {
			return fmt.Errorf("effect %d must be an object", index)
		}
		if err := validateDescriptor(node, expected); err != nil {
			return fmt.Errorf("effect %d: %w", index, err)
		}
	}
	if err := validateCanonicalByteLimit(vocabulary, maxVocabularyBytes, "effect vocabulary"); err != nil {
		return err
	}
	return validateVocabularyDigest(vocabulary)
}

func validateDescriptor(node map[string]any, expected effectDescriptor) error {
	if err := requireKeys(node, "allowed_scope_kinds", "effect_id", "production_restriction",
		"required_scope_kinds", "scope_profile"); err != nil {
		return err
	}
	checks := map[string]string{
		"effect_id": expected.id, "production_restriction": expected.restriction,
		"scope_profile": expected.profile,
	}
	for key, value := range checks {
		if err := requireStringLiteral(node, key, value); err != nil {
			return err
		}
	}
	allowed, err := readStringArray(node, "allowed_scope_kinds", 1, 9)
	if err != nil || !slices.Equal(allowed, expected.allowed) {
		return fmt.Errorf("allowed_scope_kinds differs from frozen vocabulary")
	}
	required, err := readStringArray(node, "required_scope_kinds", 1, 9)
	if err != nil || !slices.Equal(required, expected.required) {
		return fmt.Errorf("required_scope_kinds differs from frozen vocabulary")
	}
	return nil
}

func validateVocabularyDigest(vocabulary map[string]any) error {
	claimed, err := stringValue(vocabulary, "vocabulary_sha256")
	if err != nil || claimed != frozenVocabularySHA256 {
		return fmt.Errorf("vocabulary_sha256 is not the frozen v1 digest")
	}
	preimage := cloneNode(vocabulary)
	preimage["vocabulary_sha256"] = ""
	computed, err := digestNode(vocabularyDigestDomain, preimage)
	if err != nil || computed != claimed {
		return fmt.Errorf("vocabulary_sha256 does not match canonical vocabulary")
	}
	return nil
}

func readStringArray(parent map[string]any, key string, minimum, maximum int) ([]string, error) {
	values, err := arrayValue(parent, key)
	if err != nil || len(values) < minimum || len(values) > maximum {
		return nil, fmt.Errorf("%s item count must be %d..%d", key, minimum, maximum)
	}
	result := make([]string, len(values))
	for index, value := range values {
		text, ok := value.(string)
		if !ok || validateText(text, key, 160) != nil {
			return nil, fmt.Errorf("%s item %d must be bounded text", key, index)
		}
		result[index] = text
	}
	if !sortedUniqueStrings(result) {
		return nil, fmt.Errorf("%s must be sorted and unique", key)
	}
	return result, nil
}

func requireStringLiteral(parent map[string]any, key, expected string) error {
	value, err := stringValue(parent, key)
	if err != nil || value != expected {
		return fmt.Errorf("%s must equal %q", key, expected)
	}
	return nil
}
