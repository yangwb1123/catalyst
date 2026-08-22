package authenticatedadrapprovalcontract

import "fmt"

func validateBundle(value any) (map[string]any, map[string]any, error) {
	fields := []string{"authorization_ledger", "authorization_policy", "authorization_receipt",
		"authorization_request", "authorization_result", "proposal_binding",
		"proposal_document_base64url", "revocation_snapshot", "signature_profile", "trust_root"}
	node, err := requireKeys(value, "ADR approval candidate bundle", fields...)
	if err != nil {
		return nil, nil, err
	}
	if _, err = boundedCanonicalJSON(node, maxBundleBytes, "ADR approval candidate bundle"); err != nil {
		return nil, nil, err
	}
	profile, err := validateSignatureProfile(node["signature_profile"])
	if err != nil {
		return nil, nil, err
	}
	root, err := validateTrustRoot(node["trust_root"], profile["profile_sha256"].(string))
	if err != nil {
		return nil, nil, err
	}
	if err = validateBundleDocuments(node, root); err != nil {
		return nil, nil, err
	}
	return node, root, nil
}

func validateBundleDocuments(node, root map[string]any) error {
	binding, err := validateProposalBinding(node["proposal_binding"])
	if err != nil {
		return err
	}
	_, metadata, err := decodeProposalDocument(node["proposal_document_base64url"], binding,
		"proposal_document_base64url")
	if err != nil {
		return err
	}
	policy, err := validatePolicy(node["authorization_policy"], root, metadata)
	if err != nil {
		return err
	}
	if !canonicalEqual(policy["proposal_binding"], binding) {
		return fmt.Errorf("top-level policy differs from exact ProposalBinding")
	}
	snapshot, err := validateRevocation(node["revocation_snapshot"], root)
	if err != nil {
		return err
	}
	request, err := validateRequest(node["authorization_request"], root, policy, snapshot)
	if err != nil {
		return err
	}
	receipt, err := validateReceipt(node["authorization_receipt"], root)
	if err != nil {
		return err
	}
	if err = validateReceiptRelations(policy, request, snapshot, receipt, root); err != nil {
		return err
	}
	result, err := validateResult(node["authorization_result"], root)
	if err != nil {
		return err
	}
	ledger, err := validateLedger(node["authorization_ledger"], root)
	if err != nil {
		return err
	}
	return validateTopRelations(node, result, ledger)
}

func validateTopRelations(node, result, ledger map[string]any) error {
	receipt := node["authorization_receipt"]
	if !canonicalEqual(result["receipt"], receipt) {
		return fmt.Errorf("authorization result does not bind exact top-level receipt")
	}
	snapshotMatches := equalItemCount(ledger["revocation_snapshots"].([]any), node["revocation_snapshot"])
	if snapshotMatches != 1 {
		return fmt.Errorf("top-level revocation snapshot is not unique in complete ledger")
	}
	entries := ledger["entries"].([]any)
	matchIndex, matches := -1, 0
	for index, item := range entries {
		entry := item.(map[string]any)
		if entryMatchesTop(entry, node) {
			matchIndex, matches = index, matches+1
		}
	}
	if matches != 1 {
		return fmt.Errorf("top-level artifacts do not identify one complete ledger entry")
	}
	if result["delivery_disposition"] == "stored" && matchIndex != len(entries)-1 {
		return fmt.Errorf("stored result must identify the final appended ledger entry")
	}
	snapshots := ledger["revocation_snapshots"].([]any)
	if result["delivery_disposition"] == "stored" && !canonicalEqual(node["revocation_snapshot"], snapshots[len(snapshots)-1]) {
		return fmt.Errorf("stored result must bind the revocation high-water snapshot")
	}
	return nil
}

func entryMatchesTop(entry, node map[string]any) bool {
	return canonicalEqual(entry["policy"], node["authorization_policy"]) &&
		entry["proposal_document_base64url"] == node["proposal_document_base64url"] &&
		canonicalEqual(entry["request"], node["authorization_request"]) &&
		canonicalEqual(entry["receipt"], node["authorization_receipt"])
}

func equalItemCount(items []any, target any) int {
	count := 0
	for _, item := range items {
		if canonicalEqual(item, target) {
			count++
		}
	}
	return count
}
