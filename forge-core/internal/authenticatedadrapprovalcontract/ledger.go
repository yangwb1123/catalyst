package authenticatedadrapprovalcontract

import "fmt"

func validateLedger(value any, root map[string]any) (map[string]any, error) {
	label := "ArchitectureDecisionApprovalAuthorizationLedger"
	fields := []string{"api_version", "canonicalization", "clock_high_water_unix_ms",
		"entries", "kind", "ledger_sha256", "profile_id",
		"revocation_high_water_sequence", "revocation_high_water_sha256",
		"revocation_snapshots", "signature", "trust_epoch", "trust_root_sha256"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, err
	}
	if _, err = boundedCanonicalJSON(node, maxLedgerBytes, label); err != nil {
		return nil, err
	}
	if node["api_version"] != ledgerAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != label || node["profile_id"] != profileID {
		return nil, fmt.Errorf("%s envelope drifted from v1", label)
	}
	if err = validateLedgerAuthority(node, root); err != nil {
		return nil, err
	}
	snapshots, err := validateRevocationChain(node["revocation_snapshots"], root)
	if err != nil {
		return nil, err
	}
	if err = validateLedgerHighWater(node, snapshots); err != nil {
		return nil, err
	}
	entries, err := arrayValue(node["entries"], "ledger.entries", 1, maxLedgerEntries)
	if err != nil {
		return nil, err
	}
	if err = validateLedgerEntries(entries, snapshots, root, node["clock_high_water_unix_ms"].(int64)); err != nil {
		return nil, err
	}
	digest, err := ledgerSHA256(node)
	if err != nil || node["ledger_sha256"] != digest {
		return nil, fmt.Errorf("ledger self digest does not match")
	}
	return node, nil
}

func validateLedgerAuthority(node, root map[string]any) error {
	if node["trust_root_sha256"] != root["root_sha256"] || node["trust_epoch"] != root["trust_epoch"] {
		return fmt.Errorf("ledger does not bind the supplied trust root")
	}
	if _, err := intValue(node["trust_epoch"], "ledger.trust_epoch", 1, maxInt64); err != nil {
		return err
	}
	if _, err := intValue(node["clock_high_water_unix_ms"], "ledger.clock_high_water_unix_ms", 0, maxInt64); err != nil {
		return err
	}
	signature, err := validateSignature(node["signature"], "ledger.signature", signatureProfileSHA256Pin)
	if err != nil {
		return err
	}
	key, err := keyNodeForUsage(root, "approval_authorization_state_sign")
	if err != nil || signature["key_id"] != key["key_id"] {
		return fmt.Errorf("ledger signature uses the wrong root key usage")
	}
	return nil
}

func validateLedgerHighWater(node map[string]any, snapshots []map[string]any) error {
	latest := snapshots[len(snapshots)-1]
	if node["revocation_high_water_sequence"] != latest["revocation_sequence"] ||
		node["revocation_high_water_sha256"] != latest["revocation_sha256"] {
		return fmt.Errorf("ledger revocation high-water differs from complete snapshot chain")
	}
	clock := node["clock_high_water_unix_ms"].(int64)
	if clock < latest["effective_at_unix_ms"].(int64) {
		return fmt.Errorf("ledger clock high-water is below revocation high-water time")
	}
	keyID := node["signature"].(map[string]any)["key_id"].(string)
	if arrayContains(latest["revoked_key_ids"].([]any), keyID) {
		return fmt.Errorf("ledger signing key is revoked at revocation high-water")
	}
	return nil
}

type ledgerValidationState struct {
	priorReceipt            any
	recordKeys              map[string]bool
	authorizedProposals     map[string]bool
	priorRevocationSequence int64
}

func validateLedgerEntries(entries []any, snapshots []map[string]any, root map[string]any,
	clockHighWater int64) error {
	state := ledgerValidationState{recordKeys: map[string]bool{}, authorizedProposals: map[string]bool{}}
	for index, value := range entries {
		entry, err := requireKeys(value, fmt.Sprintf("ledger.entries[%d]", index),
			"policy", "proposal_document_base64url", "receipt", "request", "sequence")
		if err != nil {
			return err
		}
		if entry["sequence"] != int64(index+1) {
			return fmt.Errorf("ledger entry sequence must start at one and be contiguous")
		}
		receipt, err := validateLedgerEntry(entry, snapshots, root)
		if err != nil {
			return err
		}
		if err = validateEntryChain(entry, receipt, &state, clockHighWater); err != nil {
			return err
		}
	}
	return nil
}

func validateLedgerEntry(entry map[string]any, snapshots []map[string]any,
	root map[string]any) (map[string]any, error) {
	requestValue, ok := entry["request"].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("ledger request must be an object")
	}
	binding, err := validateProposalBinding(requestValue["proposal_binding"])
	if err != nil {
		return nil, err
	}
	_, metadata, err := decodeProposalDocument(entry["proposal_document_base64url"], binding, "ledger proposal document")
	if err != nil {
		return nil, err
	}
	policy, err := validatePolicy(entry["policy"], root, metadata)
	if err != nil {
		return nil, err
	}
	if !canonicalEqual(policy["proposal_binding"], binding) {
		return nil, fmt.Errorf("ledger policy differs from exact proposal binding")
	}
	snapshot, err := snapshotForRequest(requestValue, snapshots)
	if err != nil {
		return nil, err
	}
	request, err := validateRequest(requestValue, root, policy, snapshot)
	if err != nil {
		return nil, err
	}
	receipt, err := validateReceipt(entry["receipt"], root)
	if err != nil {
		return nil, err
	}
	if err = validateReceiptRelations(policy, request, snapshot, receipt, root); err != nil {
		return nil, err
	}
	return receipt, nil
}

func validateEntryChain(entry, receipt map[string]any, state *ledgerValidationState,
	clockHighWater int64) error {
	sequence := entry["sequence"].(int64)
	request := entry["request"].(map[string]any)
	if receipt["ledger_sequence"] != sequence || request["expected_next_sequence"] != sequence {
		return fmt.Errorf("ledger entry, request, and receipt sequence differ")
	}
	if receipt["prior_receipt_sha256"] != state.priorReceipt {
		return fmt.Errorf("ledger receipt prior digest chain is not contiguous")
	}
	recordKey := receipt["record_key_sha256"].(string)
	if state.recordKeys[recordKey] {
		return fmt.Errorf("ledger contains a duplicate idempotency record key")
	}
	state.recordKeys[recordKey] = true
	if err := validateAuthorizedProposalUniqueness(receipt, state); err != nil {
		return err
	}
	revocationSequence := request["revocation_sequence"].(int64)
	if revocationSequence < state.priorRevocationSequence {
		return fmt.Errorf("ledger request revocation sequences must be nondecreasing")
	}
	observed := request["requested_at_unix_ms"].(int64)
	if receipt["evaluated_at_unix_ms"].(int64) > observed {
		observed = receipt["evaluated_at_unix_ms"].(int64)
	}
	if clockHighWater < observed {
		return fmt.Errorf("ledger clock high-water is below an observed declared timestamp")
	}
	state.priorReceipt = receipt["receipt_sha256"]
	state.priorRevocationSequence = revocationSequence
	return nil
}

func validateAuthorizedProposalUniqueness(receipt map[string]any, state *ledgerValidationState) error {
	if receipt["authorization_decision"] != "acceptance_transition_authorized" {
		return nil
	}
	proposal := receipt["proposal_binding_sha256"].(string)
	if state.authorizedProposals[proposal] {
		return fmt.Errorf("ledger contains two authorized receipts for one proposal")
	}
	state.authorizedProposals[proposal] = true
	return nil
}
