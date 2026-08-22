package bootstraprepoexecutionauthority

import (
	"fmt"
)

var invocationKeys = []string{"api_version", "bindings", "canonicalization", "capability",
	"execution_policy_sha256", "execution_trust_epoch", "execution_trust_root_sha256",
	"expires_at_unix_ms", "grant_envelope_sha256", "grant_id", "grant_issuance_ledger_sequence",
	"grant_issuance_receipt_sha256", "grant_policy_sha256", "grant_request_sha256", "grant_sha256",
	"idempotency_key", "invocation_id", "invocation_sha256", "issuance_trust_epoch",
	"issuance_trust_root_sha256", "kind", "manifest_sha256", "profile_id", "requested_action",
	"requested_action_sha256", "requested_at_unix_ms", "signature", "subject", "task_binding"}

// Invocation is a strict authenticated one-shot execution request.
type Invocation struct{ document map[string]any }

// DecodeInvocation authenticates every Policy, Grant, Manifest, and freshness relation.
func DecodeInvocation(data []byte, trust *Trust, manifest *Manifest,
	policy *Policy) (*Invocation, error) {
	if trust == nil || manifest == nil || policy == nil || policy.grant == nil {
		return nil, fmt.Errorf("Trust, Manifest, and authenticated Policy are required")
	}
	document, err := decodeCanonical(data, maxInvocationBytes)
	if err != nil {
		return nil, err
	}
	if err = validateInvocation(document, trust, policy.grant, manifest, policy); err != nil {
		return nil, err
	}
	return &Invocation{document}, nil
}

func (invocation *Invocation) canonicalDocument() map[string]any {
	return cloneDocument(invocation.document)
}

// ExecutionLimits returns the raw-byte ceiling and cooperative timeout.
func (invocation *Invocation) ExecutionLimits() (int64, int64) {
	if invocation == nil {
		return 0, 0
	}
	action := invocation.document["requested_action"].(map[string]any)
	usage := action["usage"].(map[string]any)
	output, _ := intValue(usage, "output_bytes")
	timeout, _ := intValue(usage, "timeout_ms")
	return output, timeout
}

func validateInvocation(document map[string]any, trust *Trust, grant *issuedGrant,
	manifest *Manifest, policy *Policy) error {
	if err := validateReplayInvocation(document, trust, manifest, policy); err != nil {
		return err
	}
	return validateInvocationGrantRelations(document, grant.document)
}

func validateReplayInvocation(document map[string]any, trust *Trust, manifest *Manifest,
	policy *Policy) error {
	if err := requireKeys(document, invocationKeys...); err != nil {
		return fmt.Errorf("BootstrapRepoReadInvocation: %w", err)
	}
	if err := validateInvocationShape(document, trust); err != nil {
		return err
	}
	if policy.document["disposition"] != "allow" || policy.document["activation"] != "activate_once" {
		return fmt.Errorf("non-activating ExecutionPolicy cannot authenticate an Invocation")
	}
	if err := validateInvocationPolicyRelations(document, manifest.document, policy.document); err != nil {
		return err
	}
	return validateSigned(document, "invocation_sha256", invocationDomain,
		invocationSignatureDomain, maxInvocationBytes, "Invocation", trust,
		"execution_request_auth", "invocation_id")
}

func validateInvocationShape(document map[string]any, trust *Trust) error {
	if err := validateEnvelope(document, invocationAPI, "BootstrapRepoReadInvocation"); err != nil {
		return err
	}
	if document["profile_id"] != profileID {
		return fmt.Errorf("Invocation profile is invalid")
	}
	key, err := stringValue(document, "idempotency_key")
	if err != nil || !idempotencyPattern.MatchString(key) {
		return fmt.Errorf("Invocation idempotency_key is invalid")
	}
	if err := validateInvocationParts(document); err != nil {
		return err
	}
	if err := validateAuthorityBinding(document, trust, "Invocation"); err != nil {
		return err
	}
	return validateInvocationIdentity(document)
}

func validateInvocationParts(document map[string]any) error {
	validators := []func() error{
		func() error { return validateBindings(document["bindings"], "Invocation bindings") },
		func() error { return validateCapability(document["capability"], "Invocation capability") },
		func() error { _, e := validatePrincipal(document["subject"], "Invocation subject"); return e },
		func() error { return validateTask(document["task_binding"], "Invocation task_binding") },
		func() error {
			return validateRequestedAction(document["requested_action"], document["requested_action_sha256"])
		},
	}
	for _, validator := range validators {
		if err := validator(); err != nil {
			return err
		}
	}
	for _, field := range []string{"execution_policy_sha256", "execution_trust_root_sha256",
		"grant_envelope_sha256", "grant_issuance_receipt_sha256", "grant_policy_sha256",
		"grant_request_sha256", "grant_sha256", "invocation_sha256", "issuance_trust_root_sha256",
		"manifest_sha256", "requested_action_sha256"} {
		if err := validateHashField(document, field, "Invocation "+field); err != nil {
			return err
		}
	}
	return validateInvocationWindow(document)
}

func validateInvocationIdentity(document map[string]any) error {
	digest, _ := stringValue(document, "invocation_sha256")
	identifier, err := stringValue(document, "invocation_id")
	if err != nil || identifier != "bootstrap-repo-read-invocation-"+digest {
		return fmt.Errorf("Invocation identity is invalid")
	}
	for _, field := range []string{"execution_trust_epoch", "grant_issuance_ledger_sequence", "issuance_trust_epoch"} {
		value, valueErr := intValue(document, field)
		if valueErr != nil || value < 1 || (field == "grant_issuance_ledger_sequence" && value > 256) {
			return fmt.Errorf("Invocation %s is invalid", field)
		}
	}
	return nil
}

func validateInvocationWindow(document map[string]any) error {
	requested, requestedErr := intValue(document, "requested_at_unix_ms")
	expires, expiresErr := intValue(document, "expires_at_unix_ms")
	if requestedErr != nil || expiresErr != nil || requested < 0 || expires <= requested ||
		expires-requested > maxFreshnessMillis {
		return fmt.Errorf("Invocation freshness interval is invalid")
	}
	return nil
}

func validateInvocationPolicyRelations(invocation, manifest, policy map[string]any) error {
	policyFields := []string{"bindings", "capability", "execution_trust_epoch",
		"execution_trust_root_sha256", "grant_envelope_sha256", "grant_id",
		"grant_issuance_ledger_sequence", "grant_issuance_receipt_sha256", "grant_policy_sha256",
		"grant_request_sha256", "grant_sha256", "idempotency_key", "issuance_trust_epoch",
		"issuance_trust_root_sha256", "manifest_sha256", "profile_id", "requested_action",
		"requested_action_sha256", "subject", "task_binding"}
	for _, field := range policyFields {
		if !sameCanonical(invocation[field], policy[field]) {
			return fmt.Errorf("Invocation field %s differs from ExecutionPolicy", field)
		}
	}
	if invocation["execution_policy_sha256"] != policy["execution_policy_sha256"] ||
		invocation["manifest_sha256"] != manifest["manifest_sha256"] {
		return fmt.Errorf("Invocation does not bind exact Policy and Manifest")
	}
	return validateInvocationPolicyWindow(invocation, policy)
}

func validateInvocationGrantRelations(invocation, grant map[string]any) error {
	fields := []string{"bindings", "capability", "grant_envelope_sha256", "grant_id",
		"grant_issuance_ledger_sequence", "grant_issuance_receipt_sha256", "grant_policy_sha256",
		"grant_request_sha256", "grant_sha256", "issuance_trust_epoch", "issuance_trust_root_sha256",
		"subject", "task_binding"}
	for _, field := range fields {
		if !sameCanonical(invocation[field], grant[field]) {
			return fmt.Errorf("Invocation field %s differs from issued Grant", field)
		}
	}
	return nil
}

func validateInvocationPolicyWindow(invocation, policy map[string]any) error {
	requested, _ := intValue(invocation, "requested_at_unix_ms")
	expires, _ := intValue(invocation, "expires_at_unix_ms")
	policyValidity := policy["validity"].(map[string]any)
	policyStart, _ := intValue(policyValidity, "not_before_unix_ms")
	policyEnd, _ := intValue(policyValidity, "expires_at_unix_ms")
	if requested < policyStart || requested >= policyEnd || expires > policyEnd {
		return fmt.Errorf("Invocation freshness exceeds Policy validity")
	}
	return nil
}

// ValidateExecutionTime checks a caller-observed clock without reading it.
func ValidateExecutionTime(policy *Policy, invocation *Invocation, observedAt int64) error {
	if policy == nil || invocation == nil || observedAt < 0 {
		return fmt.Errorf("Policy, Invocation, and non-negative observed time are required")
	}
	validity := policy.document["validity"].(map[string]any)
	start, _ := intValue(validity, "not_before_unix_ms")
	policyEnd, _ := intValue(validity, "expires_at_unix_ms")
	requestStart, _ := intValue(invocation.document, "requested_at_unix_ms")
	requestEnd, _ := intValue(invocation.document, "expires_at_unix_ms")
	if observedAt < start || observedAt >= policyEnd || observedAt < requestStart || observedAt >= requestEnd {
		return fmt.Errorf("observed time is outside Policy or Invocation validity")
	}
	return nil
}
