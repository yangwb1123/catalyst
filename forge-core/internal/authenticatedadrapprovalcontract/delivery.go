package authenticatedadrapprovalcontract

import "fmt"

// StoredBundle constructs the exact stored-shaped bundle for a validated input,
// receipt, and complete ledger. The label does not attest persistence.
func StoredBundle(input *AuthorizationInput, receipt *Receipt, ledger *Ledger) (*Bundle, error) {
	if input == nil || receipt == nil || ledger == nil || input.root == nil {
		return nil, fmt.Errorf("stored bundle inputs must be non-nil")
	}
	if !sameRoot(input.root, receipt.root) || !sameRoot(input.root, ledger.root) {
		return nil, fmt.Errorf("stored bundle trust roots differ")
	}
	if err := validateReceiptForInput(input, receipt); err != nil {
		return nil, err
	}
	if _, err := validateLedger(ledger.document, input.root.document); err != nil {
		return nil, err
	}
	snapshot, err := inputSnapshot(input)
	if err != nil {
		return nil, err
	}
	encodedProposal, err := encodeProposalDocument(input.proposal)
	if err != nil {
		return nil, err
	}
	node := bundleNode(input.policy, encodedProposal, input.request, receipt.document,
		snapshot, ledger.document, input.root.document, "stored")
	return validatedOpaqueBundle(node)
}

// ExactReplayBundle projects one exact prior ledger entry by idempotency key.
// The label does not attest storage, retention, rollback resistance, or CAS.
func ExactReplayBundle(ledger *Ledger, idempotencyKey string) (*Bundle, error) {
	if ledger == nil || ledger.root == nil {
		return nil, fmt.Errorf("ledger or trust root is nil")
	}
	if _, err := validateLedger(ledger.document, ledger.root.document); err != nil {
		return nil, err
	}
	recordKey, err := recordKeySHA256(idempotencyKey)
	if err != nil {
		return nil, err
	}
	entry, err := entryForRecordKey(ledger.document, recordKey)
	if err != nil {
		return nil, err
	}
	request := entry["request"].(map[string]any)
	snapshots := ledgerSnapshotMaps(ledger.document)
	snapshot, err := snapshotForRequest(request, snapshots)
	if err != nil {
		return nil, err
	}
	node := bundleNode(entry["policy"].(map[string]any),
		entry["proposal_document_base64url"].(string), request,
		entry["receipt"].(map[string]any), snapshot, ledger.document,
		ledger.root.document, "exact_replay")
	return validatedOpaqueBundle(node)
}

func bundleNode(policy map[string]any, encodedProposal string, request, receipt,
	snapshot, ledger, root map[string]any, disposition string) map[string]any {
	result := map[string]any{"api_version": resultAPI, "canonicalization": canonicalization,
		"delivery_disposition": disposition,
		"kind":                 "ArchitectureDecisionApprovalAuthorizationResult",
		"receipt":              cloneValue(receipt)}
	return map[string]any{
		"authorization_ledger": cloneValue(ledger), "authorization_policy": cloneValue(policy),
		"authorization_receipt": cloneValue(receipt), "authorization_request": cloneValue(request),
		"authorization_result":        result,
		"proposal_binding":            cloneValue(policy["proposal_binding"]),
		"proposal_document_base64url": encodedProposal,
		"revocation_snapshot":         cloneValue(snapshot),
		"signature_profile":           signatureProfileDocument(), "trust_root": cloneValue(root),
	}
}

func validatedOpaqueBundle(value map[string]any) (*Bundle, error) {
	node, root, err := validateBundle(value)
	if err != nil {
		return nil, err
	}
	rootCopy := &TrustRoot{document: cloneValue(root).(map[string]any)}
	return &Bundle{document: cloneValue(node).(map[string]any), root: rootCopy}, nil
}

func entryForRecordKey(ledger map[string]any, recordKey string) (map[string]any, error) {
	var match map[string]any
	for _, item := range ledger["entries"].([]any) {
		entry := item.(map[string]any)
		receipt := entry["receipt"].(map[string]any)
		if receipt["record_key_sha256"] == recordKey {
			if match != nil {
				return nil, fmt.Errorf("ledger contains duplicate idempotency record keys")
			}
			match = entry
		}
	}
	if match == nil {
		return nil, fmt.Errorf("idempotency key is absent from ledger")
	}
	return match, nil
}

func ledgerSnapshotMaps(ledger map[string]any) []map[string]any {
	items := ledger["revocation_snapshots"].([]any)
	result := make([]map[string]any, len(items))
	for index, item := range items {
		result[index] = item.(map[string]any)
	}
	return result
}

// Position returns a detached view of the supplied complete ledger tip.
func (ledger *Ledger) Position() (ledgerPosition, error) {
	if ledger == nil || ledger.root == nil {
		return ledgerPosition{}, fmt.Errorf("ledger or trust root is nil")
	}
	if _, err := validateLedger(ledger.document, ledger.root.document); err != nil {
		return ledgerPosition{}, err
	}
	entries := ledger.document["entries"].([]any)
	lastReceipt := entries[len(entries)-1].(map[string]any)["receipt"].(map[string]any)
	return ledgerPosition{
		ClockHighWaterUnixMS:        ledger.document["clock_high_water_unix_ms"].(int64),
		LastReceiptSHA256:           lastReceipt["receipt_sha256"].(string),
		LedgerSHA256:                ledger.document["ledger_sha256"].(string),
		NextSequence:                int64(len(entries) + 1),
		RevocationHighWaterSHA256:   ledger.document["revocation_high_water_sha256"].(string),
		RevocationHighWaterSequence: ledger.document["revocation_high_water_sequence"].(int64),
	}, nil
}
