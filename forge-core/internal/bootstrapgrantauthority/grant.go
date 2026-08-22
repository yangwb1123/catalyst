package bootstrapgrantauthority

import (
	"fmt"
	"math"

	"forgeos/forge-core/internal/capabilitygrantcontract"
)

// Grant is a fully validated and Ed25519-authenticated bootstrap Grant.
type Grant struct{ document map[string]any }

// IssueGrant creates and authenticates the only allowed bootstrap Grant.
func IssueGrant(policy *Policy, request *Request, storedAt int64, issuer *Issuer) (*Grant, error) {
	if policy == nil || request == nil || issuer == nil || issuer.trust == nil {
		return nil, fmt.Errorf("Policy, Request, and issuer are required")
	}
	if err := validateSigningInputs(policy, request, issuer.trust); err != nil {
		return nil, err
	}
	if policy.document["disposition"] != "allow" {
		return nil, fmt.Errorf("deny Policy cannot issue a Grant")
	}
	if err := validateIssuanceTime(policy.document, request.document, storedAt); err != nil {
		return nil, err
	}
	candidate, err := buildGrantCandidate(policy.document, request.document, storedAt, issuer.trust)
	if err != nil {
		return nil, err
	}
	prepared, digest, err := capabilitygrantcontract.PrepareGrantForSigning(candidate)
	if err != nil {
		return nil, err
	}
	proof, err := issuer.sign(grantSignatureDomain, digest)
	if err != nil {
		return nil, err
	}
	document, err := capabilitygrantcontract.FinalizeSignedGrant(prepared, proof)
	if err != nil {
		return nil, err
	}
	grant := &Grant{document: document}
	if err = validateGrantRelations(grant, policy, request, storedAt, issuer.trust); err != nil {
		return nil, err
	}
	return grant, nil
}

func validateSigningInputs(policy *Policy, request *Request, trust *Trust) error {
	if err := validateAuthorityBinding(policy.document, trust, "Policy"); err != nil {
		return err
	}
	if err := validateAuthorityBinding(request.document, trust, "Request"); err != nil {
		return err
	}
	return validatePolicyRequest(policy.document, request.document)
}

func buildGrantCandidate(policy, request map[string]any, storedAt int64,
	trust *Trust) (map[string]any, error) {
	ttl, _ := intValue(request, "requested_ttl_ms")
	if ttl > math.MaxInt64-storedAt {
		return nil, fmt.Errorf("Grant expiration overflows signed int64")
	}
	requestBindings, _ := objectValue(request, "bindings")
	return map[string]any{
		"api_version": "forgeos.capability-grant/v1", "approval_refs": []any{},
		"authority_proof": buildGrantProof(trust), "bindings": map[string]any{
			"context_sha256":       requestBindings["context_sha256"],
			"grant_request_sha256": request["request_sha256"], "impact_sha256": nil,
			"plan_sha256": nil, "policy_sha256": policy["policy_sha256"], "risk_sha256": nil,
			"source_revision":    requestBindings["source_revision"],
			"source_tree_sha256": requestBindings["source_tree_sha256"],
		},
		"budget": cloneNode(request["budget"]), "canonicalization": canonicalization,
		"capability":               cloneNode(request["capability"]),
		"effect_vocabulary_sha256": effectVocabularySHA256, "grant_id": "", "grant_sha256": "",
		"issuance_phase": "bootstrap_planning", "kind": "CapabilityGrant",
		"scope": cloneNode(request["scope"]), "separation_of_duty": buildSeparation(trust),
		"subject": cloneNode(request["subject"]), "task_binding": cloneNode(request["task_binding"]),
		"usage_policy": buildUsagePolicy(), "validity": map[string]any{
			"expires_at_unix_ms": storedAt + ttl, "issued_at_unix_ms": storedAt,
			"not_before_unix_ms": storedAt, "transferable": false,
		},
	}, nil
}

func buildGrantProof(trust *Trust) map[string]any {
	key := trust.keys["grant_issue"]
	issuer := cloneNode(key.principal).(map[string]any)
	issuer["authority_class"] = "forgeos_kernel"
	return map[string]any{
		"issuer": issuer, "key_id": key.id, "proof_base64url": "",
		"proof_profile_id": signatureProfile, "proof_profile_sha256": trust.profileHash,
		"trust_domain": trust.domain, "trust_epoch": trust.epoch,
	}
}

func buildSeparation(trust *Trust) map[string]any {
	return map[string]any{
		"requester":             cloneNode(trust.keys["request_auth"].principal),
		"required_distinctions": []any{"issuer_not_requester", "issuer_not_subject"},
	}
}

func buildUsagePolicy() map[string]any {
	return map[string]any{
		"atomic_reservation_required": true, "concurrent_use": "forbidden",
		"consumption_mode": "single_use", "replay": "receipt_only_no_reexecute",
		"uncertain_effect": "quarantine", "usage_ledger_required": true,
	}
}

func validateIssuanceTime(policy, request map[string]any, storedAt int64) error {
	if storedAt < 0 {
		return fmt.Errorf("durable decision time is negative")
	}
	policyValidity, _ := objectValue(policy, "validity")
	policyStart, _ := intValue(policyValidity, "not_before_unix_ms")
	policyEnd, _ := intValue(policyValidity, "expires_at_unix_ms")
	requestStart, _ := intValue(request, "requested_at_unix_ms")
	requestEnd, _ := intValue(request, "expires_at_unix_ms")
	ttl, _ := intValue(request, "requested_ttl_ms")
	if storedAt < policyStart || storedAt >= policyEnd || storedAt < requestStart || storedAt >= requestEnd {
		return fmt.Errorf("durable decision time is outside Policy or Request validity")
	}
	if policy["disposition"] == "allow" &&
		(ttl > math.MaxInt64-storedAt || storedAt+ttl > policyEnd) {
		return fmt.Errorf("Grant validity would exceed Policy validity")
	}
	return nil
}

func validateGrantRelations(grant *Grant, policy *Policy, request *Request,
	storedAt int64, trust *Trust) error {
	bytes, err := capabilitygrantcontract.CanonicalGrantJSON(grant.document)
	if err != nil || len(bytes) > maxGrantBytes {
		return fmt.Errorf("CapabilityGrant is not valid ADR-0056 canonical data")
	}
	if err = validateBootstrapGrantShape(grant.document, trust); err != nil {
		return err
	}
	if err = validateGrantRequestRelations(grant.document, policy.document, request.document); err != nil {
		return err
	}
	return validateGrantTime(grant.document, request.document, storedAt)
}

func validateBootstrapGrantShape(document map[string]any, trust *Trust) error {
	if document["issuance_phase"] != "bootstrap_planning" {
		return fmt.Errorf("Grant issuance phase is not bootstrap_planning")
	}
	approvals, err := arrayValue(document, "approval_refs")
	if err != nil || len(approvals) != 0 {
		return fmt.Errorf("bootstrap Grant approval_refs must be empty")
	}
	bindings, _ := objectValue(document, "bindings")
	if bindings["impact_sha256"] != nil || bindings["plan_sha256"] != nil || bindings["risk_sha256"] != nil {
		return fmt.Errorf("bootstrap Grant impact, plan, and risk bindings must be null")
	}
	if err := validateGrantProof(document, trust); err != nil {
		return err
	}
	return validateGrantUsageAndSeparation(document, trust)
}

func validateGrantUsageAndSeparation(document map[string]any, trust *Trust) error {
	expectedUsage := buildUsagePolicy()
	usageEqual, usageErr := sameCanonical(document["usage_policy"], expectedUsage)
	separation, separationErr := objectValue(document, "separation_of_duty")
	if usageErr != nil || !usageEqual || separationErr != nil {
		return fmt.Errorf("bootstrap Grant usage policy or separation is invalid")
	}
	expected := buildSeparation(trust)
	separationEqual, err := sameCanonical(separation, expected)
	if err != nil || !separationEqual {
		return fmt.Errorf("bootstrap Grant separation-of-duty declarations drifted")
	}
	return nil
}

func validateGrantProof(document map[string]any, trust *Trust) error {
	proof, err := objectValue(document, "authority_proof")
	if err != nil || proof["key_id"] != trust.keys["grant_issue"].id ||
		proof["proof_profile_id"] != signatureProfile || proof["proof_profile_sha256"] != trust.profileHash ||
		proof["trust_domain"] != trust.domain || proof["trust_epoch"] != trust.epoch {
		return fmt.Errorf("Grant authority proof binding is invalid")
	}
	expectedIssuer := cloneNode(trust.keys["grant_issue"].principal).(map[string]any)
	expectedIssuer["authority_class"] = "forgeos_kernel"
	equal, _ := sameCanonical(proof["issuer"], expectedIssuer)
	grantHash, _ := stringValue(document, "grant_sha256")
	proofText, _ := stringValue(proof, "proof_base64url")
	if !equal {
		return fmt.Errorf("Grant issuer does not match the pinned grant_issue principal")
	}
	return verifyDigest(trust.keys["grant_issue"].publicKey, grantSignatureDomain, grantHash, proofText)
}

func validateGrantRequestRelations(grant, policy, request map[string]any) error {
	for _, field := range []string{"budget", "capability", "scope", "subject", "task_binding"} {
		equal, err := sameCanonical(grant[field], request[field])
		if err != nil || !equal {
			return fmt.Errorf("Grant field %s differs from Request", field)
		}
	}
	bindings, _ := objectValue(grant, "bindings")
	requestBindings, _ := objectValue(request, "bindings")
	expected := map[string]any{
		"context_sha256": requestBindings["context_sha256"], "grant_request_sha256": request["request_sha256"],
		"impact_sha256": nil, "plan_sha256": nil, "policy_sha256": policy["policy_sha256"],
		"risk_sha256": nil, "source_revision": requestBindings["source_revision"],
		"source_tree_sha256": requestBindings["source_tree_sha256"],
	}
	equal, _ := sameCanonical(bindings, expected)
	if !equal {
		return fmt.Errorf("Grant bindings differ from Policy or Request")
	}
	return nil
}

func validateGrantTime(grant, request map[string]any, storedAt int64) error {
	validity, _ := objectValue(grant, "validity")
	expires, _ := intValue(validity, "expires_at_unix_ms")
	issued, _ := intValue(validity, "issued_at_unix_ms")
	notBefore, _ := intValue(validity, "not_before_unix_ms")
	ttl, _ := intValue(request, "requested_ttl_ms")
	if issued != storedAt || notBefore != storedAt || expires != storedAt+ttl || validity["transferable"] != false {
		return fmt.Errorf("Grant validity differs from durable decision time and Request TTL")
	}
	return nil
}

func grantEnvelopeSHA256(document map[string]any) (string, error) {
	return digestNode(grantEnvelopeDomain, document, maxGrantBytes, "CapabilityGrant envelope")
}
