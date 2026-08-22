package authenticatedadrapprovalcontract

import (
	"encoding/base64"
	"fmt"
)

var placeholderSignature = base64.RawURLEncoding.EncodeToString(make([]byte, 64))

// NewReceiptDraft deterministically builds a structurally related receipt
// preimage. The returned message still requires external signature verification
// and signing authority; this function grants neither.
func NewReceiptDraft(input *AuthorizationInput, evaluatedAtUnixMS int64,
	priorReceiptSHA256 *string) (*ReceiptDraft, []byte, error) {
	if input == nil || input.root == nil {
		return nil, nil, fmt.Errorf("authorization input or trust root is nil")
	}
	snapshot, err := inputSnapshot(input)
	if err != nil {
		return nil, nil, err
	}
	records, err := validateApprovalRecords(input.request["approval_records"], input.policy,
		input.root.document, snapshot, evaluatedAtUnixMS)
	if err != nil {
		return nil, nil, err
	}
	node, err := receiptDraftNode(input, snapshot, records, evaluatedAtUnixMS, priorReceiptSHA256)
	if err != nil {
		return nil, nil, err
	}
	if _, err = validateReceipt(node, input.root.document); err != nil {
		return nil, nil, err
	}
	if err = validateReceiptRelations(input.policy, input.request, snapshot, node, input.root.document); err != nil {
		return nil, nil, err
	}
	message, err := signatureMessage(receiptSignatureDomain, node["receipt_sha256"].(string))
	if err != nil {
		return nil, nil, err
	}
	return &ReceiptDraft{document: cloneValue(node).(map[string]any), input: input}, message, nil
}

func receiptDraftNode(input *AuthorizationInput, snapshot map[string]any,
	records []map[string]any, evaluatedAt int64, prior *string) (map[string]any, error) {
	decision, approvals := declaredOutcome(input.policy, records)
	recordKey, err := recordKeySHA256(input.request["idempotency_key"].(string))
	if err != nil {
		return nil, err
	}
	stateKey, err := keyNodeForUsage(input.root.document, "approval_authorization_state_sign")
	if err != nil {
		return nil, err
	}
	binding := input.policy["proposal_binding"].(map[string]any)
	node := map[string]any{
		"api_version": receiptAPI, "authorization_decision": decision,
		"authorization_expires_at_unix_ms": receiptExpiry(input.policy, input.request, snapshot, records),
		"canonicalization":                 canonicalization, "evaluated_at_unix_ms": evaluatedAt,
		"kind":            "ArchitectureDecisionApprovalAuthorizationReceipt",
		"ledger_sequence": input.request["expected_next_sequence"],
		"policy_sha256":   input.policy["policy_sha256"], "prior_receipt_sha256": nullableString(prior),
		"profile_id": profileID, "proposal_binding_sha256": binding["proposal_binding_sha256"],
		"qualifying_approval_ids": stringsToAny(approvals),
		"reason_codes":            stringsToAny(declaredReasonCodes(input.policy, records)),
		"receipt_id":              "", "receipt_sha256": "", "record_key_sha256": recordKey,
		"request_sha256":      input.request["request_sha256"],
		"revocation_sequence": snapshot["revocation_sequence"],
		"revocation_sha256":   snapshot["revocation_sha256"],
		"signature":           signatureNode(stateKey["key_id"].(string), placeholderSignature),
		"trust_epoch":         input.root.document["trust_epoch"],
		"trust_root_sha256":   input.root.document["root_sha256"],
	}
	digest, err := receiptSHA256(node)
	if err != nil {
		return nil, err
	}
	node["receipt_id"], node["receipt_sha256"] = "architecture-decision-approval-receipt-"+digest, digest
	return node, nil
}

func receiptExpiry(policy, request, snapshot map[string]any, records []map[string]any) int64 {
	expires := policy["validity"].(map[string]any)["expires_at_unix_ms"].(int64)
	expires = minimumInt64(expires, request["expires_at_unix_ms"].(int64))
	expires = minimumInt64(expires, snapshot["expires_at_unix_ms"].(int64))
	for _, record := range records {
		expires = minimumInt64(expires, record["validity"].(map[string]any)["expires_at_unix_ms"].(int64))
	}
	return expires
}

// SealReceipt attaches one canonical proof-shaped signature and revalidates the
// exact receipt. It does not verify that signature or claim authorization.
func SealReceipt(draft *ReceiptDraft, signatureBase64URL string) (*Receipt, error) {
	if draft == nil || draft.input == nil || draft.input.root == nil {
		return nil, fmt.Errorf("receipt draft or authorization input is nil")
	}
	if _, err := fixedBase64URL(signatureBase64URL, "receipt signature", 64); err != nil {
		return nil, err
	}
	node := cloneValue(draft.document).(map[string]any)
	node["signature"].(map[string]any)["signature_base64url"] = signatureBase64URL
	receipt, err := validateReceipt(node, draft.input.root.document)
	if err != nil {
		return nil, err
	}
	snapshot, err := inputSnapshot(draft.input)
	if err != nil {
		return nil, err
	}
	if err = validateReceiptRelations(draft.input.policy, draft.input.request, snapshot,
		receipt, draft.input.root.document); err != nil {
		return nil, err
	}
	return &Receipt{document: cloneValue(receipt).(map[string]any), root: draft.input.root,
		input: draft.input}, nil
}

// NewLedgerDraft deterministically appends one exact input/receipt pair to a
// supplied prior ledger (or creates genesis) and returns the ledger signature message.
func NewLedgerDraft(input *AuthorizationInput, receipt *Receipt, prior *Ledger,
	clockHighWaterUnixMS int64) (*LedgerDraft, []byte, error) {
	if input == nil || input.root == nil || receipt == nil {
		return nil, nil, fmt.Errorf("authorization input, root, or receipt is nil")
	}
	if !sameRoot(input.root, receipt.root) {
		return nil, nil, fmt.Errorf("receipt and input trust roots differ")
	}
	if err := validateReceiptForInput(input, receipt); err != nil {
		return nil, nil, err
	}
	node, err := ledgerDraftNode(input, receipt, prior, clockHighWaterUnixMS)
	if err != nil {
		return nil, nil, err
	}
	if _, err = validateLedger(node, input.root.document); err != nil {
		return nil, nil, err
	}
	message, err := signatureMessage(ledgerSignatureDomain, node["ledger_sha256"].(string))
	if err != nil {
		return nil, nil, err
	}
	return &LedgerDraft{document: cloneValue(node).(map[string]any), root: input.root}, message, nil
}

func validateReceiptForInput(input *AuthorizationInput, receipt *Receipt) error {
	validated, err := validateReceipt(receipt.document, input.root.document)
	if err != nil {
		return err
	}
	snapshot, err := inputSnapshot(input)
	if err != nil {
		return err
	}
	return validateReceiptRelations(input.policy, input.request, snapshot, validated, input.root.document)
}

func ledgerDraftNode(input *AuthorizationInput, receipt *Receipt, prior *Ledger,
	clockHighWater int64) (map[string]any, error) {
	entries, snapshots, err := appendLedgerState(input, receipt, prior)
	if err != nil {
		return nil, err
	}
	latest := snapshots[len(snapshots)-1].(map[string]any)
	stateKey, err := keyNodeForUsage(input.root.document, "approval_authorization_state_sign")
	if err != nil {
		return nil, err
	}
	node := map[string]any{
		"api_version": ledgerAPI, "canonicalization": canonicalization,
		"clock_high_water_unix_ms": clockHighWater, "entries": entries,
		"kind": "ArchitectureDecisionApprovalAuthorizationLedger", "ledger_sha256": "",
		"profile_id": profileID, "revocation_high_water_sequence": latest["revocation_sequence"],
		"revocation_high_water_sha256": latest["revocation_sha256"],
		"revocation_snapshots":         snapshots,
		"signature":                    signatureNode(stateKey["key_id"].(string), placeholderSignature),
		"trust_epoch":                  input.root.document["trust_epoch"],
		"trust_root_sha256":            input.root.document["root_sha256"],
	}
	digest, err := ledgerSHA256(node)
	if err != nil {
		return nil, err
	}
	node["ledger_sha256"] = digest
	return node, nil
}

func appendLedgerState(input *AuthorizationInput, receipt *Receipt,
	prior *Ledger) ([]any, []any, error) {
	entries := []any{}
	if prior != nil {
		if !sameRoot(input.root, prior.root) {
			return nil, nil, fmt.Errorf("prior ledger and input trust roots differ")
		}
		if _, err := validateLedger(prior.document, prior.root.document); err != nil {
			return nil, nil, err
		}
		if err := validatePriorPosition(input, receipt, prior); err != nil {
			return nil, nil, err
		}
		entries = cloneValue(prior.document["entries"]).([]any)
	}
	encodedProposal, err := encodeProposalDocument(input.proposal)
	if err != nil {
		return nil, nil, err
	}
	entry := map[string]any{"policy": cloneValue(input.policy),
		"proposal_document_base64url": encodedProposal, "receipt": cloneValue(receipt.document),
		"request": cloneValue(input.request), "sequence": input.request["expected_next_sequence"]}
	entries = append(entries, entry)
	snapshots := cloneValue(input.snapshots).([]any)
	if prior != nil && !snapshotPrefixMatches(prior.document["revocation_snapshots"].([]any), snapshots) {
		return nil, nil, fmt.Errorf("input revocation chain does not extend prior complete state")
	}
	return entries, snapshots, nil
}

func validatePriorPosition(input *AuthorizationInput, receipt *Receipt, prior *Ledger) error {
	entries := prior.document["entries"].([]any)
	expectedSequence := int64(len(entries) + 1)
	if input.request["expected_next_sequence"] != expectedSequence ||
		input.request["expected_ledger_sha256"] != prior.document["ledger_sha256"] {
		return fmt.Errorf("request CAS position differs from supplied prior ledger")
	}
	lastReceipt := entries[len(entries)-1].(map[string]any)["receipt"].(map[string]any)
	if receipt.document["prior_receipt_sha256"] != lastReceipt["receipt_sha256"] {
		return fmt.Errorf("receipt prior digest differs from supplied prior ledger")
	}
	return nil
}

func snapshotPrefixMatches(prior, current []any) bool {
	if len(prior) > len(current) {
		return false
	}
	for index := range prior {
		if !canonicalEqual(prior[index], current[index]) {
			return false
		}
	}
	return true
}

// SealLedger attaches one canonical proof-shaped signature and revalidates the
// complete ledger. It does not persist or authenticate the result.
func SealLedger(draft *LedgerDraft, signatureBase64URL string) (*Ledger, error) {
	if draft == nil || draft.root == nil {
		return nil, fmt.Errorf("ledger draft or trust root is nil")
	}
	if _, err := fixedBase64URL(signatureBase64URL, "ledger signature", 64); err != nil {
		return nil, err
	}
	node := cloneValue(draft.document).(map[string]any)
	node["signature"].(map[string]any)["signature_base64url"] = signatureBase64URL
	ledger, err := validateLedger(node, draft.root.document)
	if err != nil {
		return nil, err
	}
	return &Ledger{document: cloneValue(ledger).(map[string]any), root: draft.root}, nil
}

func signatureNode(keyID, signature string) map[string]any {
	return map[string]any{"key_id": keyID, "profile_id": signatureProfileID,
		"profile_sha256": signatureProfileSHA256Pin, "signature_base64url": signature}
}

func nullableString(value *string) any {
	if value == nil {
		return nil
	}
	return *value
}

func stringsToAny(values []string) []any {
	result := make([]any, len(values))
	for index, value := range values {
		result[index] = value
	}
	return result
}

func sameRoot(left, right *TrustRoot) bool {
	return left != nil && right != nil && canonicalEqual(left.document, right.document)
}
