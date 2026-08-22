package capabilityregistry

import (
	"bytes"
	"fmt"
)

// Resolve performs exact declared-reference resolution against the frozen
// singleton registry. A result carries no implementation or effect authority.
func Resolve(registry, request map[string]any) (map[string]any, error) {
	if err := validateRegistry(registry); err != nil || registry["registry_sha256"] != pinnedRegistrySHA256 {
		return nil, fmt.Errorf("registry is not the frozen singleton v1 profile")
	}
	if err := validateRequest(request); err != nil {
		return nil, err
	}
	if request["registry_sha256"] != registry["registry_sha256"] {
		return nil, fmt.Errorf("request registry digest does not bind the supplied registry")
	}
	entry := registry["entries"].([]any)[0].(map[string]any)
	resolution, matched, err := resolveReference(entry, request)
	if err != nil {
		return nil, err
	}
	assessment := buildAssessment(registry, request, resolution, matched)
	digest, err := digestDocument(assessmentDigestDomain, assessment, "assessment_sha256")
	if err != nil {
		return nil, err
	}
	assessment["assessment_sha256"] = digest
	if err := validateAssessment(assessment); err != nil {
		return nil, fmt.Errorf("internal assessment reconstruction: %w", err)
	}
	return assessment, nil
}

func resolveReference(entry, request map[string]any) (string, bool, error) {
	contract := entry["contract"].(map[string]any)
	reference := request["expected_reference"].(map[string]any)
	if reference["capability_id"] != contract["capability_id"] {
		return "capability_id_not_found", false, nil
	}
	if reference["capability_version"] != contract["capability_version"] {
		return "capability_version_not_found", false, nil
	}
	if reference["capability_contract_sha256"] != contract["capability_contract_sha256"] {
		return "capability_contract_digest_mismatch", true, nil
	}
	if request["expected_contract"] != nil {
		expected := request["expected_contract"].(map[string]any)
		actualJSON, _ := canonicalJSON(contract)
		expectedJSON, _ := canonicalJSON(expected)
		if !bytes.Equal(actualJSON, expectedJSON) {
			return "", false, fmt.Errorf("equal contract digest has unequal canonical contract bytes")
		}
	}
	return "resolved_exact", true, nil
}

func buildAssessment(
	registry, request map[string]any, resolution string, matched bool,
) map[string]any {
	entry := registry["entries"].([]any)[0].(map[string]any)
	assessment := map[string]any{
		"api_version":       assessmentAPIVersion,
		"assessment_mode":   "authority_neutral_read_only_exact_declared_contract",
		"assessment_sha256": "", "authorization_decision": "none",
		"canonicalization": canonicalization, "effect_attestation": false,
		"gate_applicability_state":                "not_evaluated",
		"implementation_availability_attestation": false, "invocation_attestation": false,
		"kind":                       "CapabilityRegistryDeclaredResolution",
		"owner_authentication_state": "not_evaluated", "permission_attestation": false,
		"persistence_attestation": false, "proof_satisfaction_state": "not_evaluated",
		"reason_codes":                  assessmentReasons(resolution),
		"registry_authentication_state": "not_evaluated",
		"registry_sha256":               registry["registry_sha256"],
		"relations":                     assessmentRelations(resolution, request["expected_contract"] != nil),
		"request_sha256":                request["request_sha256"], "resolution": resolution,
		"result": assessmentResult, "rule_applicability_state": "not_evaluated",
		"runtime_routing_attestation": false, "test_pass_attestation": false,
		"transition_attestation": false,
	}
	assessment["matched_key_entry_id"], assessment["matched_key_entry_sha256"] = nil, nil
	if matched {
		assessment["matched_key_entry_id"] = entry["entry_id"]
		assessment["matched_key_entry_sha256"] = entry["entry_sha256"]
	}
	return assessment
}

func assessmentRelations(resolution string, hasContract bool) map[string]any {
	relations := make(map[string]any, len(relationKeys))
	identity := resolution
	if resolution == "resolved_exact" {
		identity = "same_declared_identity"
	}
	relations["identity"] = identity
	for _, key := range nonIdentityRelationKeys {
		relations[key] = "not_evaluated"
		if resolution == "resolved_exact" && hasContract {
			relations[key] = "same_declared_" + key
		}
	}
	return relations
}

func assessmentReasons(resolution string) []any {
	if resolution == "resolved_exact" {
		return []any{}
	}
	return []any{resolution}
}
