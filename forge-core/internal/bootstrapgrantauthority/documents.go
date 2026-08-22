package bootstrapgrantauthority

import (
	"crypto/sha256"
	"fmt"
)

var policyKeys = []string{
	"api_version", "budget", "canonicalization", "capability", "disposition", "effect_id",
	"kind", "max_ttl_ms", "policy_id", "policy_sha256", "profile_id", "scope", "signature",
	"subject", "task_binding", "trust_epoch", "trust_root_sha256", "validity",
}

var requestKeys = []string{
	"api_version", "bindings", "budget", "canonicalization", "capability", "effect_id",
	"expires_at_unix_ms", "idempotency_key", "kind", "policy_sha256", "profile_id",
	"request_sha256", "requested_at_unix_ms", "requested_ttl_ms", "scope", "signature",
	"subject", "task_binding", "trust_epoch", "trust_root_sha256",
}

// Policy is a structurally valid and cryptographically authenticated Policy.
type Policy struct{ document map[string]any }

// Request is a structurally valid and cryptographically authenticated request.
type Request struct{ document map[string]any }

// DecodePolicy authenticates a strict canonical Policy against the pinned root.
func DecodePolicy(data []byte, trust *Trust) (*Policy, error) {
	if trust == nil {
		return nil, fmt.Errorf("Trust is required")
	}
	document, err := decodeCanonical(data, maxPolicyBytes)
	if err != nil {
		return nil, err
	}
	if err = validatePolicy(document, trust); err != nil {
		return nil, err
	}
	return &Policy{document: document}, nil
}

// DecodeRequest authenticates a strict request and its exact Policy relations.
func DecodeRequest(data []byte, trust *Trust, policy *Policy) (*Request, error) {
	if trust == nil || policy == nil {
		return nil, fmt.Errorf("Trust and Policy are required")
	}
	document, err := decodeCanonical(data, maxRequestBytes)
	if err != nil {
		return nil, err
	}
	if err = validateRequest(document, trust); err != nil {
		return nil, err
	}
	if err = validatePolicyRequest(policy.document, document); err != nil {
		return nil, err
	}
	return &Request{document: document}, nil
}

func validatePolicy(document map[string]any, trust *Trust) error {
	if err := requireKeys(document, policyKeys...); err != nil {
		return fmt.Errorf("BootstrapGrantPolicy: %w", err)
	}
	if err := validatePolicyShape(document); err != nil {
		return err
	}
	if err := validateAuthorityBinding(document, trust, "Policy"); err != nil {
		return err
	}
	return validateSignedDocument(document, "policy_sha256", policyDomain,
		policySignatureDomain, maxPolicyBytes, "Policy", trust, "policy_sign")
}

func validatePolicyShape(document map[string]any) error {
	if err := validateDocumentEnvelope(document, policyAPI, "BootstrapGrantPolicy"); err != nil {
		return err
	}
	if err := validateTextField(document, "policy_id", 160); err != nil {
		return err
	}
	disposition, err := stringValue(document, "disposition")
	if err != nil || !oneOf(disposition, "allow", "deny") || document["effect_id"] != "repo.read" {
		return fmt.Errorf("Policy disposition or effect is invalid")
	}
	if _, err = validateCapabilityNode(document["capability"], "Policy capability"); err != nil {
		return err
	}
	if _, err = validatePrincipalNode(document["subject"], "Policy subject"); err != nil {
		return err
	}
	if _, err = validateTaskNode(document["task_binding"], "Policy task_binding"); err != nil {
		return err
	}
	if _, err = validateScopeNode(document["scope"], "Policy scope"); err != nil {
		return err
	}
	if _, err = validateBudgetNode(document["budget"], "Policy budget"); err != nil {
		return err
	}
	return validatePolicyWindow(document)
}

func validatePolicyWindow(document map[string]any) error {
	maximum, err := intValue(document, "max_ttl_ms")
	if err != nil || validateRange(maximum, "Policy max_ttl_ms", 1, maxTTLMillis) != nil {
		return fmt.Errorf("Policy max_ttl_ms is invalid")
	}
	validity, err := objectValue(document, "validity")
	if err != nil || requireKeys(validity, "expires_at_unix_ms", "not_before_unix_ms") != nil {
		return fmt.Errorf("Policy validity fields are invalid")
	}
	start, startErr := intValue(validity, "not_before_unix_ms")
	end, endErr := intValue(validity, "expires_at_unix_ms")
	if startErr != nil || endErr != nil || start < 0 || end <= start || end-start > maxPolicyWindow {
		return fmt.Errorf("Policy validity must be ordered within 24 hours")
	}
	return nil
}

func validateRequest(document map[string]any, trust *Trust) error {
	if err := requireKeys(document, requestKeys...); err != nil {
		return fmt.Errorf("BootstrapGrantRequest: %w", err)
	}
	if err := validateRequestShape(document); err != nil {
		return err
	}
	if err := validateAuthorityBinding(document, trust, "Request"); err != nil {
		return err
	}
	if equal, _ := sameCanonical(document["subject"], trust.keys["request_auth"].principal); !equal {
		return fmt.Errorf("request-auth principal must equal Request subject")
	}
	return validateSignedDocument(document, "request_sha256", requestDomain,
		requestSignatureDomain, maxRequestBytes, "Request", trust, "request_auth")
}

func validateRequestShape(document map[string]any) error {
	if err := validateDocumentEnvelope(document, requestAPI, "BootstrapGrantRequest"); err != nil {
		return err
	}
	if document["effect_id"] != "repo.read" {
		return fmt.Errorf("Request effect must be repo.read")
	}
	validators := []func() error{
		func() error {
			_, err := validateCapabilityNode(document["capability"], "Request capability")
			return err
		},
		func() error { _, err := validatePrincipalNode(document["subject"], "Request subject"); return err },
		func() error { _, err := validateTaskNode(document["task_binding"], "Request task_binding"); return err },
		func() error { _, err := validateScopeNode(document["scope"], "Request scope"); return err },
		func() error { _, err := validateBudgetNode(document["budget"], "Request budget"); return err },
		func() error { return validateRequestBindings(document["bindings"]) },
		func() error { return validateRequestWindow(document) },
	}
	for _, validator := range validators {
		if err := validator(); err != nil {
			return err
		}
	}
	return validateHashField(document, "policy_sha256", "Request policy_sha256")
}

func validateRequestBindings(value any) error {
	node, ok := value.(map[string]any)
	if !ok || requireKeys(node, "context_sha256", "source_revision", "source_tree_sha256") != nil {
		return fmt.Errorf("Request bindings fields are invalid")
	}
	if err := validateHashField(node, "context_sha256", "Request context_sha256"); err != nil {
		return err
	}
	if err := validateHashField(node, "source_tree_sha256", "Request source_tree_sha256"); err != nil {
		return err
	}
	return validateTextField(node, "source_revision", 160)
}

func validateRequestWindow(document map[string]any) error {
	key, err := stringValue(document, "idempotency_key")
	if err != nil || !idempotencyPattern.MatchString(key) {
		return fmt.Errorf("Request idempotency_key is invalid")
	}
	start, startErr := intValue(document, "requested_at_unix_ms")
	end, endErr := intValue(document, "expires_at_unix_ms")
	ttl, ttlErr := intValue(document, "requested_ttl_ms")
	if startErr != nil || endErr != nil || start < 0 || end <= start || end-start > maxRequestAge {
		return fmt.Errorf("Request freshness window is invalid")
	}
	if ttlErr != nil || validateRange(ttl, "requested_ttl_ms", 1, maxTTLMillis) != nil {
		return fmt.Errorf("Request TTL is invalid")
	}
	return nil
}

func validatePolicyRequest(policy, request map[string]any) error {
	fields := []string{"capability", "effect_id", "profile_id", "scope", "subject",
		"task_binding", "trust_epoch", "trust_root_sha256"}
	for _, field := range fields {
		equal, err := sameCanonical(policy[field], request[field])
		if err != nil || !equal {
			return fmt.Errorf("Policy and Request exact field %s differs", field)
		}
	}
	if request["policy_sha256"] != policy["policy_sha256"] {
		return fmt.Errorf("Request does not bind the exact Policy")
	}
	policyBudget, _ := objectValue(policy, "budget")
	requestBudget, _ := objectValue(request, "budget")
	if !budgetCovers(policyBudget, requestBudget) {
		return fmt.Errorf("Request budget exceeds Policy budget")
	}
	return validatePolicyRequestTime(policy, request)
}

func validatePolicyRequestTime(policy, request map[string]any) error {
	policyTTL, _ := intValue(policy, "max_ttl_ms")
	requestTTL, _ := intValue(request, "requested_ttl_ms")
	requested, _ := intValue(request, "requested_at_unix_ms")
	expires, _ := intValue(request, "expires_at_unix_ms")
	validity, _ := objectValue(policy, "validity")
	notBefore, _ := intValue(validity, "not_before_unix_ms")
	policyExpires, _ := intValue(validity, "expires_at_unix_ms")
	if requestTTL > policyTTL || requested < notBefore || expires > policyExpires {
		return fmt.Errorf("Request TTL or freshness exceeds Policy")
	}
	return nil
}

func validateAuthorityBinding(document map[string]any, trust *Trust, label string) error {
	if document["profile_id"] != contractProfileID || document["trust_root_sha256"] != trust.rootHash ||
		document["trust_epoch"] != trust.epoch {
		return fmt.Errorf("%s authority binding is invalid", label)
	}
	return nil
}

func validateSignedDocument(document map[string]any, digestField string, digestDomain,
	signatureDomain []byte, maximum int, label string, trust *Trust, usage string) error {
	claimed, err := stringValue(document, digestField)
	if err != nil || validateHash(claimed, label+" digest") != nil {
		return fmt.Errorf("%s self digest is invalid", label)
	}
	computed, err := selfDigest(digestDomain, document, digestField, maximum, label, true)
	if err != nil || computed != claimed {
		return fmt.Errorf("%s self digest does not match", label)
	}
	signature, err := validateSignatureNode(document["signature"], label, trust.profileHash)
	if err != nil || signature["key_id"] != trust.keys[usage].id {
		return fmt.Errorf("%s signature key binding is invalid", label)
	}
	return verifyDigest(trust.keys[usage].publicKey, signatureDomain, claimed,
		signature["signature_base64url"].(string))
}

func validateDocumentEnvelope(document map[string]any, api, kind string) error {
	for key, expected := range map[string]string{"api_version": api,
		"canonicalization": canonicalization, "kind": kind, "profile_id": contractProfileID} {
		if err := requireLiteral(document, key, expected); err != nil {
			return err
		}
	}
	return nil
}

func validateHashField(document map[string]any, field, label string) error {
	value, err := stringValue(document, field)
	if err != nil {
		return err
	}
	return validateHash(value, label)
}

func recordKey(idempotencyKey string) string {
	hash := sha256.New()
	_, _ = hash.Write(recordKeyDomain)
	_, _ = hash.Write([]byte(idempotencyKey))
	return fmt.Sprintf("%x", hash.Sum(nil))
}
