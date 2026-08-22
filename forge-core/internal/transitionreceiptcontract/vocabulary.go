package transitionreceiptcontract

import "fmt"

var vocabularyKeys = []string{
	"api_version", "canonicalization", "edges", "kind", "rework_targets", "states",
	"terminal_states", "vocabulary_sha256",
}

func authoredVocabulary() (map[string]any, error) {
	edges := make([]any, 0, len(allowedEdges)-len(terminalStates))
	for _, state := range states {
		allowed := allowedEdges[state]
		if len(allowed) == 0 {
			continue
		}
		edges = append(edges, map[string]any{
			"allowed_to_states": stringsToAny(allowed), "from_state": state,
		})
	}
	vocabulary := map[string]any{
		"api_version": vocabularyAPI, "canonicalization": canonicalization,
		"edges": edges, "kind": "TransitionStateVocabulary",
		"rework_targets": stringsToAny(reworkStates), "states": stringsToAny(states),
		"terminal_states": stringsToAny(terminalStates), "vocabulary_sha256": "",
	}
	digest, err := vocabularyDigest(vocabulary)
	if err != nil {
		return nil, err
	}
	vocabulary["vocabulary_sha256"] = digest
	return vocabulary, nil
}

func validateVocabulary(vocabulary map[string]any) error {
	if err := requireKeys(vocabulary, vocabularyKeys...); err != nil {
		return fmt.Errorf("Transition state vocabulary: %w", err)
	}
	if err := validateCanonicalByteLimit(vocabulary, maxVocabularyBytes,
		"Transition state vocabulary"); err != nil {
		return err
	}
	expected, err := authoredVocabulary()
	if err != nil || !canonicalValuesEqual(vocabulary, expected) {
		return fmt.Errorf("Transition state vocabulary differs from the exact authored graph")
	}
	claimed, _ := stringValue(vocabulary, "vocabulary_sha256")
	computed, err := vocabularyDigest(vocabulary)
	if err != nil || claimed != computed {
		return fmt.Errorf("Transition state vocabulary self digest does not match")
	}
	return nil
}

func vocabularyDigest(vocabulary map[string]any) (string, error) {
	preimage := cloneNode(vocabulary)
	preimage["vocabulary_sha256"] = ""
	return digestNode(vocabularyDomain, preimage)
}
