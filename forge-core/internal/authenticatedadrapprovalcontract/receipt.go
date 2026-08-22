package authenticatedadrapprovalcontract

import "fmt"

func validateReceipt(value any, root map[string]any) (map[string]any, error) {
	label := "ArchitectureDecisionApprovalAuthorizationReceipt"
	fields := []string{"api_version", "authorization_decision",
		"authorization_expires_at_unix_ms", "canonicalization", "evaluated_at_unix_ms",
		"kind", "ledger_sequence", "policy_sha256", "prior_receipt_sha256",
		"profile_id", "proposal_binding_sha256", "qualifying_approval_ids",
		"reason_codes", "receipt_id", "receipt_sha256", "record_key_sha256",
		"request_sha256", "revocation_sequence", "revocation_sha256", "signature",
		"trust_epoch", "trust_root_sha256"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, err
	}
	if _, err = boundedCanonicalJSON(node, maxReceiptBytes, label); err != nil {
		return nil, err
	}
	if node["api_version"] != receiptAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != label || node["profile_id"] != profileID {
		return nil, fmt.Errorf("%s envelope drifted from v1", label)
	}
	if _, err = enumValue(node["authorization_decision"], "receipt.authorization_decision",
		"acceptance_transition_authorized", "acceptance_transition_not_authorized"); err != nil {
		return nil, err
	}
	if err = validateReceiptScalars(node); err != nil {
		return nil, err
	}
	if err = validateReceiptAuthority(node, root); err != nil {
		return nil, err
	}
	if err = validateReceiptIdentity(node); err != nil {
		return nil, err
	}
	return node, nil
}

func validateReceiptScalars(node map[string]any) error {
	for field, minimum := range map[string]int64{"evaluated_at_unix_ms": 0,
		"authorization_expires_at_unix_ms": 0, "ledger_sequence": 1, "revocation_sequence": 1} {
		if _, err := intValue(node[field], "receipt."+field, minimum, maxInt64); err != nil {
			return err
		}
	}
	for _, field := range []string{"policy_sha256", "proposal_binding_sha256", "record_key_sha256",
		"request_sha256", "revocation_sha256", "trust_root_sha256"} {
		if _, err := shaValue(node[field], "receipt."+field); err != nil {
			return err
		}
	}
	if node["prior_receipt_sha256"] != nil {
		if _, err := shaValue(node["prior_receipt_sha256"], "receipt.prior_receipt_sha256"); err != nil {
			return err
		}
	}
	approvals, err := sortedUniqueStringValues(node["qualifying_approval_ids"], "receipt.qualifying_approval_ids", 0, 16)
	if err != nil {
		return err
	}
	for _, approvalID := range approvals {
		if !approvalIDPattern.MatchString(approvalID) {
			return fmt.Errorf("receipt qualifying approval ID is malformed")
		}
	}
	reasons, err := sortedUniqueStringValues(node["reason_codes"], "receipt.reason_codes", 0, 1)
	if err != nil {
		return err
	}
	for index, reason := range reasons {
		if _, err = stableID(reason, fmt.Sprintf("receipt.reason_codes[%d]", index)); err != nil {
			return err
		}
	}
	return nil
}

func validateReceiptAuthority(node, root map[string]any) error {
	if node["trust_root_sha256"] != root["root_sha256"] || node["trust_epoch"] != root["trust_epoch"] {
		return fmt.Errorf("receipt does not bind the supplied trust root")
	}
	if _, err := intValue(node["trust_epoch"], "receipt.trust_epoch", 1, maxInt64); err != nil {
		return err
	}
	signature, err := validateSignature(node["signature"], "receipt.signature", signatureProfileSHA256Pin)
	if err != nil {
		return err
	}
	key, err := keyNodeForUsage(root, "approval_authorization_state_sign")
	if err != nil || signature["key_id"] != key["key_id"] {
		return fmt.Errorf("receipt signature uses the wrong root key usage")
	}
	return nil
}

func validateReceiptIdentity(node map[string]any) error {
	if _, err := shaValue(node["receipt_sha256"], "receipt.receipt_sha256"); err != nil {
		return err
	}
	digest, err := receiptSHA256(node)
	if err != nil || node["receipt_sha256"] != digest {
		return fmt.Errorf("receipt self digest does not match")
	}
	if node["receipt_id"] != "architecture-decision-approval-receipt-"+digest {
		return fmt.Errorf("receipt ID does not match its digest")
	}
	return nil
}

func validateReceiptRelations(policy, request, snapshot, receipt, root map[string]any) error {
	evaluated := receipt["evaluated_at_unix_ms"].(int64)
	if evaluated < request["requested_at_unix_ms"].(int64) || evaluated >= request["expires_at_unix_ms"].(int64) {
		return fmt.Errorf("receipt evaluation time lies outside request validity")
	}
	if evaluated < snapshot["effective_at_unix_ms"].(int64) || evaluated >= snapshot["expires_at_unix_ms"].(int64) {
		return fmt.Errorf("receipt evaluation time lies outside revocation validity")
	}
	records, err := validateApprovalRecords(request["approval_records"], policy, root, snapshot, evaluated)
	if err != nil {
		return err
	}
	keyID := receipt["signature"].(map[string]any)["key_id"].(string)
	if arrayContains(snapshot["revoked_key_ids"].([]any), keyID) {
		return fmt.Errorf("receipt signing key is revoked")
	}
	if err = validateReceiptBindings(policy, request, snapshot, receipt); err != nil {
		return err
	}
	if err = validateReceiptOutcome(policy, records, receipt); err != nil {
		return err
	}
	return validateReceiptExpiry(policy, request, snapshot, records, receipt)
}

func validateReceiptBindings(policy, request, snapshot, receipt map[string]any) error {
	recordKey, err := recordKeySHA256(request["idempotency_key"].(string))
	if err != nil {
		return err
	}
	binding := policy["proposal_binding"].(map[string]any)
	expected := map[string]any{"ledger_sequence": request["expected_next_sequence"],
		"policy_sha256": policy["policy_sha256"], "proposal_binding_sha256": binding["proposal_binding_sha256"],
		"record_key_sha256": recordKey, "request_sha256": request["request_sha256"],
		"revocation_sequence": snapshot["revocation_sequence"], "revocation_sha256": snapshot["revocation_sha256"],
		"trust_epoch": request["trust_epoch"], "trust_root_sha256": request["trust_root_sha256"]}
	for field, value := range expected {
		if receipt[field] != value {
			return fmt.Errorf("receipt does not bind request, policy, proposal, revocation, or root")
		}
	}
	sequence := receipt["ledger_sequence"].(int64)
	if (sequence == 1 && receipt["prior_receipt_sha256"] != nil) ||
		(sequence > 1 && receipt["prior_receipt_sha256"] == nil) {
		return fmt.Errorf("receipt prior digest shape differs from ledger sequence")
	}
	return nil
}

func validateReceiptOutcome(policy map[string]any, records []map[string]any, receipt map[string]any) error {
	decision, approvals := declaredOutcome(policy, records)
	reasons := declaredReasonCodes(policy, records)
	if receipt["authorization_decision"] != decision ||
		!stringArrayEquals(receipt["qualifying_approval_ids"].([]any), approvals) ||
		!stringArrayEquals(receipt["reason_codes"].([]any), reasons) {
		return fmt.Errorf("receipt differs from declared policy/ApprovalRecord relations")
	}
	return nil
}

func validateReceiptExpiry(policy, request, snapshot map[string]any, records []map[string]any,
	receipt map[string]any) error {
	expected := policy["validity"].(map[string]any)["expires_at_unix_ms"].(int64)
	expected = minimumInt64(expected, request["expires_at_unix_ms"].(int64))
	expected = minimumInt64(expected, snapshot["expires_at_unix_ms"].(int64))
	for _, record := range records {
		expires := record["validity"].(map[string]any)["expires_at_unix_ms"].(int64)
		expected = minimumInt64(expected, expires)
	}
	if receipt["authorization_expires_at_unix_ms"] != expected {
		return fmt.Errorf("receipt expiry is not the minimum declared validity bound")
	}
	if receipt["evaluated_at_unix_ms"].(int64) >= expected {
		return fmt.Errorf("receipt authorization window is already closed")
	}
	return nil
}

func minimumInt64(left, right int64) int64 {
	if left < right {
		return left
	}
	return right
}
