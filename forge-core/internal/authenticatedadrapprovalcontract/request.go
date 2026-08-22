package authenticatedadrapprovalcontract

import (
	"fmt"
	"regexp"
)

var idempotencyPattern = regexp.MustCompile(`^[A-Za-z0-9._:@+\-]{16,128}$`)

func validateRequest(value any, root, policy, snapshot map[string]any) (map[string]any, error) {
	label := "ArchitectureDecisionApprovalAuthorizationRequest"
	fields := []string{"api_version", "approval_records", "canonicalization",
		"expected_ledger_sha256", "expected_next_sequence", "expires_at_unix_ms",
		"idempotency_key", "kind", "policy_sha256", "profile_id", "proposal_binding",
		"request_id", "request_sha256", "requested_at_unix_ms", "requester",
		"revocation_sequence", "revocation_sha256", "signature", "trust_epoch",
		"trust_root_sha256"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, err
	}
	if _, err = boundedCanonicalJSON(node, maxRequestBytes, label); err != nil {
		return nil, err
	}
	if node["api_version"] != requestAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != label || node["profile_id"] != profileID {
		return nil, fmt.Errorf("%s envelope drifted from v1", label)
	}
	if err = validateRequestTimeAndCAS(node, policy, snapshot); err != nil {
		return nil, err
	}
	if err = validateRequestAuthority(node, root, policy); err != nil {
		return nil, err
	}
	if err = validateRequestRelations(node, policy, snapshot); err != nil {
		return nil, err
	}
	requestedAt := node["requested_at_unix_ms"].(int64)
	if _, err = validateApprovalRecords(node["approval_records"], policy, root, snapshot, requestedAt); err != nil {
		return nil, err
	}
	if err = validateRequestIdentity(node); err != nil {
		return nil, err
	}
	return node, nil
}

func validateRequestTimeAndCAS(node, policy, snapshot map[string]any) error {
	start, err := intValue(node["requested_at_unix_ms"], "request.requested_at_unix_ms", 0, maxInt64)
	if err != nil {
		return err
	}
	expires, err := intValue(node["expires_at_unix_ms"], "request.expires_at_unix_ms", 0, maxInt64)
	if err != nil {
		return err
	}
	maximum := policy["max_request_validity_ms"].(int64)
	if start >= expires || expires-start > maximum {
		return fmt.Errorf("request validity exceeds the signed policy maximum")
	}
	validity := policy["validity"].(map[string]any)
	if start < validity["not_before_unix_ms"].(int64) || expires > validity["expires_at_unix_ms"].(int64) {
		return fmt.Errorf("request validity lies outside policy validity")
	}
	if start < snapshot["effective_at_unix_ms"].(int64) || start >= snapshot["expires_at_unix_ms"].(int64) {
		return fmt.Errorf("request declared time lies outside revocation snapshot validity")
	}
	sequence, err := intValue(node["expected_next_sequence"], "request.expected_next_sequence", 1, maxInt64)
	if err != nil {
		return err
	}
	if sequence == 1 && node["expected_ledger_sha256"] != nil {
		return fmt.Errorf("genesis request requires null expected ledger digest")
	}
	if sequence > 1 {
		_, err = shaValue(node["expected_ledger_sha256"], "request.expected_ledger_sha256")
	}
	return err
}

func validateRequestAuthority(node, root, policy map[string]any) error {
	requester, err := validatePrincipal(node["requester"], "request.requester", "agent", "human", "operator", "service")
	if err != nil {
		return err
	}
	requestKey, err := keyNodeForUsage(root, "approval_request_auth")
	if err != nil {
		return err
	}
	policyRequester := policy["roles"].(map[string]any)["requester"]
	if !canonicalEqual(requester, policyRequester) || !canonicalEqual(requester, requestKey["principal"]) {
		return fmt.Errorf("requester differs from signed policy and request-auth key")
	}
	if node["trust_root_sha256"] != root["root_sha256"] || node["trust_epoch"] != root["trust_epoch"] {
		return fmt.Errorf("request does not bind the supplied trust root")
	}
	if _, err = intValue(node["trust_epoch"], "request.trust_epoch", 1, maxInt64); err != nil {
		return err
	}
	signature, err := validateSignature(node["signature"], "request.signature", signatureProfileSHA256Pin)
	if err != nil || signature["key_id"] != requestKey["key_id"] {
		return fmt.Errorf("request signature uses the wrong root key usage")
	}
	return nil
}

func validateRequestRelations(node, policy, snapshot map[string]any) error {
	if node["policy_sha256"] != policy["policy_sha256"] ||
		!canonicalEqual(node["proposal_binding"], policy["proposal_binding"]) {
		return fmt.Errorf("request does not bind exact policy and proposal")
	}
	if node["revocation_sequence"] != snapshot["revocation_sequence"] ||
		node["revocation_sha256"] != snapshot["revocation_sha256"] {
		return fmt.Errorf("request does not bind exact revocation snapshot")
	}
	revoked := snapshot["revoked_key_ids"].([]any)
	requestKey := node["signature"].(map[string]any)["key_id"].(string)
	policyKey := policy["signature"].(map[string]any)["key_id"].(string)
	if arrayContains(revoked, requestKey) || arrayContains(revoked, policyKey) {
		return fmt.Errorf("request or policy signing key is revoked")
	}
	key, ok := node["idempotency_key"].(string)
	if !ok || !idempotencyPattern.MatchString(key) {
		return fmt.Errorf("request idempotency key must be 16..128 closed visible ASCII bytes")
	}
	return nil
}

func validateRequestIdentity(node map[string]any) error {
	if _, err := shaValue(node["request_sha256"], "request.request_sha256"); err != nil {
		return err
	}
	digest, err := requestSHA256(node)
	if err != nil || node["request_sha256"] != digest {
		return fmt.Errorf("request self digest does not match")
	}
	if node["request_id"] != "architecture-decision-approval-request-"+digest {
		return fmt.Errorf("request ID does not match its digest")
	}
	return nil
}

func snapshotForRequest(request map[string]any, snapshots []map[string]any) (map[string]any, error) {
	sequence, sequenceOK := request["revocation_sequence"]
	digest, digestOK := request["revocation_sha256"]
	if !sequenceOK || !digestOK {
		return nil, fmt.Errorf("request has no revocation snapshot reference")
	}
	var match map[string]any
	for _, snapshot := range snapshots {
		if snapshot["revocation_sequence"] == sequence && snapshot["revocation_sha256"] == digest {
			if match != nil {
				return nil, fmt.Errorf("request revocation reference is not unique")
			}
			match = snapshot
		}
	}
	if match == nil {
		return nil, fmt.Errorf("request revocation reference is absent from complete ledger state")
	}
	return match, nil
}
