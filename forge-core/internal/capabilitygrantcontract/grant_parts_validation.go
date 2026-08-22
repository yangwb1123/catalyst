package capabilitygrantcontract

import (
	"encoding/base64"
	"fmt"
)

func validatePrincipal(node map[string]any) error {
	if err := requireKeys(node, "authority_domain", "principal_id", "principal_type"); err != nil {
		return err
	}
	for _, key := range []string{"authority_domain", "principal_id"} {
		value, err := stringValue(node, key)
		if err != nil || validateText(value, key, 160) != nil {
			return fmt.Errorf("%s must be non-empty bounded text", key)
		}
	}
	typeName, err := stringValue(node, "principal_type")
	if err != nil {
		return err
	}
	return validateEnum(typeName, "principal_type", "agent", "human", "operator", "service")
}

func validateIssuer(node map[string]any) error {
	if err := requireKeys(node, "authority_class", "authority_domain", "principal_id", "principal_type"); err != nil {
		return err
	}
	class, err := stringValue(node, "authority_class")
	if err != nil || validateEnum(class, "authority_class", "external_operator", "forgeos_kernel") != nil {
		return fmt.Errorf("issuer authority_class is unsupported")
	}
	principal := cloneNode(node)
	delete(principal, "authority_class")
	if err := validatePrincipal(principal); err != nil {
		return err
	}
	typeName, _ := stringValue(node, "principal_type")
	if class == "forgeos_kernel" && typeName != "service" {
		return fmt.Errorf("forgeos_kernel issuer must be a service principal")
	}
	if class == "external_operator" && typeName != "human" && typeName != "operator" {
		return fmt.Errorf("external_operator issuer must be a human or operator principal")
	}
	return nil
}

func validateAuthorityProof(node map[string]any) (map[string]any, error) {
	if err := requireKeys(node, "issuer", "key_id", "proof_base64url", "proof_profile_id",
		"proof_profile_sha256", "trust_domain", "trust_epoch"); err != nil {
		return nil, err
	}
	issuer, err := objectValue(node, "issuer")
	if err != nil || validateIssuer(issuer) != nil {
		return nil, fmt.Errorf("authority_proof issuer is invalid")
	}
	for _, key := range []string{"key_id", "proof_profile_id", "trust_domain"} {
		value, err := stringValue(node, key)
		if err != nil || validateText(value, key, 160) != nil {
			return nil, fmt.Errorf("%s must be non-empty bounded text", key)
		}
	}
	profileHash, err := stringValue(node, "proof_profile_sha256")
	if err != nil || validateHash(profileHash, "proof_profile_sha256") != nil {
		return nil, fmt.Errorf("proof_profile_sha256 is invalid")
	}
	if err := validateDeclaredProof(node); err != nil {
		return nil, err
	}
	return issuer, nil
}

func validateDeclaredProof(node map[string]any) error {
	proof, err := stringValue(node, "proof_base64url")
	if err != nil || len(proof) < 16 || len(proof) > 16384 {
		return fmt.Errorf("proof_base64url must be non-empty bounded base64url")
	}
	decoded, decodeErr := base64.RawURLEncoding.DecodeString(proof)
	if decodeErr != nil || base64.RawURLEncoding.EncodeToString(decoded) != proof {
		return fmt.Errorf("proof_base64url must be canonical unpadded base64url")
	}
	epoch, err := intValue(node, "trust_epoch")
	if err != nil || epoch < 0 {
		return fmt.Errorf("trust_epoch must be non-negative")
	}
	return nil
}

func validateBindings(node map[string]any) error {
	if err := requireKeys(node, "context_sha256", "grant_request_sha256", "impact_sha256", "plan_sha256",
		"policy_sha256", "risk_sha256", "source_revision", "source_tree_sha256"); err != nil {
		return err
	}
	for _, key := range []string{"context_sha256", "grant_request_sha256", "policy_sha256", "source_tree_sha256"} {
		value, err := stringValue(node, key)
		if err != nil || validateHash(value, key) != nil {
			return fmt.Errorf("%s must be a lowercase SHA-256", key)
		}
	}
	for _, key := range []string{"impact_sha256", "plan_sha256", "risk_sha256"} {
		value, err := nullableStringValue(node, key)
		if err != nil || (value != nil && validateHash(*value, key) != nil) {
			return fmt.Errorf("%s must be null or a lowercase SHA-256", key)
		}
	}
	revision, err := stringValue(node, "source_revision")
	if err != nil {
		return err
	}
	return validateText(revision, "source_revision", 160)
}

func validateCapability(node map[string]any) error {
	if err := requireKeys(node, "capability_contract_sha256", "capability_id", "capability_version"); err != nil {
		return err
	}
	hash, err := stringValue(node, "capability_contract_sha256")
	if err != nil || validateHash(hash, "capability_contract_sha256") != nil {
		return fmt.Errorf("capability_contract_sha256 is invalid")
	}
	for _, key := range []string{"capability_id", "capability_version"} {
		value, err := stringValue(node, key)
		if err != nil || validateText(value, key, 160) != nil {
			return fmt.Errorf("%s must be non-empty bounded text", key)
		}
	}
	return nil
}

func validateTaskBinding(node map[string]any) error {
	if err := requireKeys(node, "attempt_id", "change_id", "environment_class", "environment_id", "node_id",
		"project_id", "role", "run_id", "target_id", "task_id"); err != nil {
		return err
	}
	for _, key := range []string{"change_id", "environment_id", "node_id", "project_id", "role", "run_id", "task_id"} {
		value, err := stringValue(node, key)
		if err != nil || validateText(value, key, 160) != nil {
			return fmt.Errorf("%s must be non-empty bounded text", key)
		}
	}
	for _, key := range []string{"attempt_id", "target_id"} {
		value, err := nullableStringValue(node, key)
		if err != nil || (value != nil && validateText(*value, key, 160) != nil) {
			return fmt.Errorf("%s must be null or non-empty bounded text", key)
		}
	}
	class, err := stringValue(node, "environment_class")
	if err != nil {
		return err
	}
	return validateEnum(class, "environment_class", "development", "local", "production", "staging", "test")
}

func validateBudget(node map[string]any) error {
	keys := []string{"max_calls", "max_cost_usd_micros", "max_input_tokens", "max_network_bytes",
		"max_output_bytes", "max_output_tokens", "timeout_ms"}
	if err := requireKeys(node, keys...); err != nil {
		return err
	}
	bounds := map[string]int64{
		"max_calls": 1000000000, "max_cost_usd_micros": 1000000000000000,
		"max_input_tokens": 1000000000, "max_network_bytes": 1073741824,
		"max_output_bytes": 1073741824, "max_output_tokens": 1000000000,
		"timeout_ms": 86400000,
	}
	for _, key := range keys {
		value, err := intValue(node, key)
		minimum := int64(0)
		if key == "max_calls" || key == "timeout_ms" {
			minimum = 1
		}
		if err != nil || value < minimum || value > bounds[key] {
			return fmt.Errorf("%s is outside its explicit v1 ceiling", key)
		}
	}
	return nil
}

func validateValidity(node map[string]any) error {
	if err := requireKeys(node, "expires_at_unix_ms", "issued_at_unix_ms", "not_before_unix_ms",
		"transferable"); err != nil {
		return err
	}
	issued, issuedErr := intValue(node, "issued_at_unix_ms")
	notBefore, beforeErr := intValue(node, "not_before_unix_ms")
	expires, expiresErr := intValue(node, "expires_at_unix_ms")
	transferable, transferErr := boolValue(node, "transferable")
	if issuedErr != nil || beforeErr != nil || expiresErr != nil || transferErr != nil {
		return fmt.Errorf("validity fields have invalid types")
	}
	if issued < 0 || notBefore < issued || expires <= notBefore || expires-issued > 86400000 {
		return fmt.Errorf("validity must satisfy 0 <= issued <= not_before < expires")
	}
	if transferable {
		return fmt.Errorf("CapabilityGrant v1 is never transferable")
	}
	return nil
}
