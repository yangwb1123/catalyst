package authenticatedadrlifecyclecontract

import (
	"fmt"

	approvalcontract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

func validateRequest(value any, profileHash string, lifecycleRoot map[string]any,
	approvalRoot *approvalcontract.TrustRoot) (map[string]any, proposalMetadata, error) {
	label := "ArchitectureDecisionLifecycleTransitionRequest"
	fields := []string{"acceptance_prerequisite", "api_version", "canonicalization",
		"expected_current_head_set_sha256", "expected_ledger_sha256", "expected_next_sequence",
		"expires_at_unix_ms", "idempotency_key", "kind", "profile_id",
		"proposal_document_base64url", "request_id", "request_sha256", "requested_at_unix_ms",
		"signature", "supersession_targets", "trust_epoch", "trust_root_sha256"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, proposalMetadata{}, err
	}
	if _, err = boundedCanonicalJSON(node, maxRequestBytes, label); err != nil {
		return nil, proposalMetadata{}, err
	}
	if node["api_version"] != requestAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != label || node["profile_id"] != profileID {
		return nil, proposalMetadata{}, fmt.Errorf("%s envelope drifted from v1", label)
	}
	prerequisite, receipt, err := validatePrerequisite(node["acceptance_prerequisite"],
		profileHash, approvalRoot)
	if err != nil {
		return nil, proposalMetadata{}, err
	}
	_, metadata, err := decodeProposalDocument(node["proposal_document_base64url"],
		prerequisite["proposal_binding"].(map[string]any), "request.proposal_document_base64url")
	if err != nil {
		return nil, proposalMetadata{}, err
	}
	if err = validateRequestScalars(node, prerequisite, receipt, metadata); err != nil {
		return nil, proposalMetadata{}, err
	}
	if err = validateRequestAuthority(node, profileHash, lifecycleRoot); err != nil {
		return nil, proposalMetadata{}, err
	}
	if err = validateRequestIdentity(node); err != nil {
		return nil, proposalMetadata{}, err
	}
	return node, metadata, nil
}

func validateRequestScalars(node, prerequisite map[string]any,
	receipt approvalReceiptFacts, metadata proposalMetadata) error {
	requested, err := intValue(node["requested_at_unix_ms"], "request.requested_at_unix_ms", 0, maxInt64)
	if err != nil {
		return err
	}
	expires, err := intValue(node["expires_at_unix_ms"], "request.expires_at_unix_ms", 0, maxInt64)
	if err != nil {
		return err
	}
	if requested != prerequisite["observed_at_unix_ms"] || expires <= requested ||
		expires-requested > maxRequestValidityMS || expires > receipt.AuthorizationExpiresAt {
		return fmt.Errorf("request time must exactly consume the observed prerequisite window")
	}
	if err = validateRequestCAS(node); err != nil {
		return err
	}
	targets, err := validateTargets(node["supersession_targets"])
	if err != nil {
		return err
	}
	return validateRequestTargets(node, prerequisite, metadata, targets)
}

func validateRequestCAS(node map[string]any) error {
	sequence, err := intValue(node["expected_next_sequence"], "request.expected_next_sequence", 1, maxInt64)
	if err != nil {
		return err
	}
	if sequence == 1 && node["expected_ledger_sha256"] != nil {
		return fmt.Errorf("genesis request requires null expected ledger digest")
	}
	if sequence > 1 {
		if _, err = shaValue(node["expected_ledger_sha256"], "request.expected_ledger_sha256"); err != nil {
			return err
		}
	}
	_, err = shaValue(node["expected_current_head_set_sha256"],
		"request.expected_current_head_set_sha256")
	return err
}

func validateTargets(value any) ([]map[string]any, error) {
	items, err := arrayValue(value, "request.supersession_targets", 0, maxSupersessions)
	if err != nil {
		return nil, err
	}
	targets := make([]map[string]any, len(items))
	for index, item := range items {
		targets[index], err = validateTarget(item, index)
		if err != nil {
			return nil, err
		}
		if index > 0 && targets[index-1]["adr_id"].(string) >= targets[index]["adr_id"].(string) {
			return nil, fmt.Errorf("request supersession_targets must be sorted and unique")
		}
	}
	return targets, nil
}

func validateTarget(value any, index int) (map[string]any, error) {
	label := fmt.Sprintf("request.supersession_targets[%d]", index)
	node, err := requireKeys(value, label, "acceptance_id", "acceptance_sha256", "adr_id",
		"proposal_binding_sha256")
	if err != nil {
		return nil, err
	}
	if _, err = adrIDValue(node["adr_id"], label+".adr_id"); err != nil {
		return nil, err
	}
	if _, err = textValue(node["acceptance_id"], label+".acceptance_id", 160); err != nil {
		return nil, err
	}
	for _, field := range []string{"acceptance_sha256", "proposal_binding_sha256"} {
		if _, err = shaValue(node[field], label+"."+field); err != nil {
			return nil, err
		}
	}
	return node, nil
}

func validateRequestTargets(node, prerequisite map[string]any, metadata proposalMetadata,
	targets []map[string]any) error {
	identifiers := make([]string, len(targets))
	for index, target := range targets {
		identifiers[index] = target["adr_id"].(string)
	}
	if !equalStrings(identifiers, metadata.Supersedes) {
		return fmt.Errorf("request targets differ from immutable proposal supersedes")
	}
	binding := prerequisite["proposal_binding"].(map[string]any)
	if containsString(identifiers, binding["adr_id"].(string)) {
		return fmt.Errorf("request cannot supersede its own immutable proposal")
	}
	idempotency, err := textValue(node["idempotency_key"], "request.idempotency_key", 160)
	if err != nil || !idempotencyPattern.MatchString(idempotency) {
		return fmt.Errorf("request.idempotency_key does not match its closed grammar")
	}
	return nil
}

func validateRequestAuthority(node map[string]any, profileHash string,
	root map[string]any) error {
	if node["trust_root_sha256"] != root["root_sha256"] ||
		node["trust_epoch"] != root["trust_epoch"] {
		return fmt.Errorf("request does not bind the lifecycle trust root")
	}
	if _, err := intValue(node["trust_epoch"], "request.trust_epoch", 1, maxInt64); err != nil {
		return err
	}
	key, err := lifecycleKey(root, requestKeyUsage)
	if err != nil {
		return err
	}
	_, err = validateSignature(node["signature"], "request.signature", profileHash,
		key["key_id"].(string))
	return err
}

func validateRequestIdentity(node map[string]any) error {
	digest, err := requestSHA256(node)
	if err != nil || node["request_sha256"] != digest {
		return fmt.Errorf("lifecycle request self digest does not match")
	}
	if node["request_id"] != "architecture-decision-lifecycle-request-"+digest {
		return fmt.Errorf("lifecycle request ID does not match its digest")
	}
	return nil
}

func equalStrings(left, right []string) bool {
	if len(left) != len(right) {
		return false
	}
	for index := range left {
		if left[index] != right[index] {
			return false
		}
	}
	return true
}
