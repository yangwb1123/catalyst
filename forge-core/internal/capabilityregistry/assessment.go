package capabilityregistry

import (
	"bytes"
	"fmt"
)

// DecodeAssessment validates a canonical authority-neutral assessment's own
// shape and digest. Use ValidateAssessment to bind it to source documents.
func DecodeAssessment(data []byte) (map[string]any, error) {
	return decodeCanonicalObject(data, maxAssessmentBytes, validateAssessment)
}

// ValidateAssessment re-resolves the supplied documents and requires the
// assessment to equal the deterministic reconstruction byte for byte.
func ValidateAssessment(registry, request, assessment map[string]any) error {
	if err := validateAssessment(assessment); err != nil {
		return err
	}
	expected, err := Resolve(registry, request)
	if err != nil {
		return err
	}
	expectedJSON, _ := canonicalJSON(expected)
	actualJSON, _ := canonicalJSON(assessment)
	if !bytes.Equal(expectedJSON, actualJSON) {
		return fmt.Errorf("assessment differs from deterministic declared resolution")
	}
	return nil
}

func validateAssessment(value map[string]any) error {
	keys := []string{
		"api_version", "assessment_mode", "assessment_sha256", "authorization_decision",
		"canonicalization", "effect_attestation", "gate_applicability_state",
		"implementation_availability_attestation", "invocation_attestation", "kind",
		"matched_key_entry_id", "matched_key_entry_sha256", "owner_authentication_state",
		"permission_attestation", "persistence_attestation", "proof_satisfaction_state",
		"reason_codes", "registry_authentication_state", "registry_sha256", "relations",
		"request_sha256", "resolution", "result", "rule_applicability_state",
		"runtime_routing_attestation", "test_pass_attestation", "transition_attestation",
	}
	if err := requireKeys(value, keys...); err != nil {
		return fmt.Errorf("resolution assessment: %w", err)
	}
	if err := validateAssessmentConstants(value); err != nil {
		return err
	}
	resolution, err := validateAssessmentIdentity(value)
	if err != nil {
		return err
	}
	if err := validateAssessmentReasons(value, resolution); err != nil {
		return err
	}
	relations, err := objectValue(value, "relations")
	if err != nil || validateRelations(relations, resolution) != nil {
		return fmt.Errorf("assessment relations are invalid")
	}
	if err := requireDigest(value, assessmentDigestDomain, "assessment_sha256"); err != nil {
		return err
	}
	return requireCanonicalSize(value, maxAssessmentBytes, "resolution assessment")
}

func validateAssessmentConstants(value map[string]any) error {
	constants := map[string]string{
		"api_version":            assessmentAPIVersion,
		"assessment_mode":        "authority_neutral_read_only_exact_declared_contract",
		"authorization_decision": "none", "canonicalization": canonicalization,
		"gate_applicability_state": "not_evaluated", "kind": "CapabilityRegistryDeclaredResolution",
		"owner_authentication_state": "not_evaluated", "proof_satisfaction_state": "not_evaluated",
		"registry_authentication_state": "not_evaluated", "result": assessmentResult,
		"rule_applicability_state": "not_evaluated",
	}
	for field, expected := range constants {
		if err := requireString(value, field, expected); err != nil {
			return err
		}
	}
	for _, field := range []string{
		"effect_attestation", "implementation_availability_attestation", "invocation_attestation",
		"permission_attestation", "persistence_attestation", "runtime_routing_attestation",
		"test_pass_attestation", "transition_attestation",
	} {
		if err := requireBool(value, field, false); err != nil {
			return err
		}
	}
	return nil
}

func validateAssessmentIdentity(value map[string]any) (string, error) {
	for _, field := range []string{"assessment_sha256", "registry_sha256", "request_sha256"} {
		digest, err := stringValue(value, field, 64, 64)
		if err != nil || !validHash(digest) {
			return "", fmt.Errorf("assessment field %q is not lowercase SHA-256", field)
		}
	}
	resolution, err := stringValue(value, "resolution", 1, 64)
	if err != nil || !oneOf(resolution, "capability_contract_digest_mismatch",
		"capability_id_not_found", "capability_version_not_found", "resolved_exact") {
		return "", fmt.Errorf("assessment resolution is invalid")
	}
	matched := resolution == "resolved_exact" || resolution == "capability_contract_digest_mismatch"
	if !matched {
		if value["matched_key_entry_id"] != nil || value["matched_key_entry_sha256"] != nil {
			return "", fmt.Errorf("unmatched resolution cannot name an entry")
		}
		return resolution, nil
	}
	id, idOK := value["matched_key_entry_id"].(string)
	digest, digestOK := value["matched_key_entry_sha256"].(string)
	if !idOK || !digestOK || !validHash(digest) || id != "capability-registry-entry-"+digest {
		return "", fmt.Errorf("matched entry identity is invalid")
	}
	return resolution, nil
}

func validateAssessmentReasons(value map[string]any, resolution string) error {
	reasons, err := arrayValue(value, "reason_codes", 0, 32)
	if err != nil || requireSortedUniqueStrings(reasons, validIdentifier) != nil {
		return fmt.Errorf("assessment reason_codes are invalid")
	}
	if resolution == "resolved_exact" && len(reasons) == 0 {
		return nil
	}
	if len(reasons) != 1 || reasons[0] != resolution {
		return fmt.Errorf("assessment reason_codes do not match resolution")
	}
	return nil
}

func validateRelations(value map[string]any, resolution string) error {
	if err := requireKeys(value, relationKeys...); err != nil {
		return err
	}
	identity, ok := value["identity"].(string)
	expectedIdentity := resolution
	if resolution == "resolved_exact" {
		expectedIdentity = "same_declared_identity"
	}
	if !ok || identity != expectedIdentity {
		return fmt.Errorf("identity relation does not match resolution")
	}
	sameCount := 0
	for _, key := range nonIdentityRelationKeys {
		relation, ok := value[key].(string)
		if !ok || !oneOf(relation, "not_evaluated", "same_declared_"+key, key+"_mismatch") {
			return fmt.Errorf("relation %q has invalid vocabulary", key)
		}
		if relation == "same_declared_"+key {
			sameCount++
		}
		if relation == key+"_mismatch" || resolution != "resolved_exact" && relation != "not_evaluated" {
			return fmt.Errorf("relation %q is not emitted by v1", key)
		}
	}
	if resolution == "resolved_exact" && sameCount != 0 && sameCount != len(nonIdentityRelationKeys) {
		return fmt.Errorf("resolved relations must be uniformly evaluated or not_evaluated")
	}
	return nil
}
