package authenticatedadrapprovalcontract

import "fmt"

// Facts returns detached structural facts for a TrustRoot, AuthorizationInput,
// Receipt, Ledger, or Bundle. These facts are not authentication or currentness
// claims.
func Facts(value any) (facts, error) {
	switch typed := value.(type) {
	case *TrustRoot:
		if typed == nil {
			return facts{}, fmt.Errorf("trust root is nil")
		}
		if _, err := validateTrustRoot(typed.document, signatureProfileSHA256Pin); err != nil {
			return facts{}, err
		}
		return rootFacts(typed.document), nil
	case *AuthorizationInput:
		return authorizationInputFacts(typed)
	case *Receipt:
		return receiptFacts(typed)
	case *Ledger:
		return ledgerFacts(typed)
	case *Bundle:
		return bundleFacts(typed)
	default:
		return facts{}, fmt.Errorf("facts do not support %T", value)
	}
}

func receiptFacts(receipt *Receipt) (facts, error) {
	if receipt == nil || receipt.root == nil {
		return facts{}, fmt.Errorf("receipt or trust root is nil")
	}
	node, err := validateReceipt(receipt.document, receipt.root.document)
	if err != nil {
		return facts{}, err
	}
	result := rootFacts(receipt.root.document)
	result.Kind = "receipt"
	populateReceiptFacts(&result, node)
	return result, nil
}

func rootFacts(root map[string]any) facts {
	items := root["keys"].([]any)
	keys := make([]RootKey, len(items))
	for index, item := range items {
		keys[index] = rootKeyView(item.(map[string]any))
	}
	return facts{Kind: "trust_root", TrustDomain: root["trust_domain"].(string),
		TrustRootSHA256: root["root_sha256"].(string), TrustEpoch: root["trust_epoch"].(int64),
		RootKeys: keys}
}

func authorizationInputFacts(input *AuthorizationInput) (facts, error) {
	if input == nil || input.root == nil {
		return facts{}, fmt.Errorf("authorization input or trust root is nil")
	}
	snapshot, err := inputSnapshot(input)
	if err != nil {
		return facts{}, err
	}
	if _, err = validateRequest(input.request, input.root.document, input.policy, snapshot); err != nil {
		return facts{}, err
	}
	result := rootFacts(input.root.document)
	result.Kind = "authorization_input"
	result.ADRID = input.metadata.ADRID
	result.ProposalBindingSHA256 = input.policy["proposal_binding"].(map[string]any)["proposal_binding_sha256"].(string)
	result.IdempotencyKey = input.request["idempotency_key"].(string)
	result.RecordKeySHA256, _ = recordKeySHA256(result.IdempotencyKey)
	result.ExpectedNextSequence = input.request["expected_next_sequence"].(int64)
	result.ExpectedLedgerSHA256 = optionalStringCopy(input.request["expected_ledger_sha256"])
	result.RequestSHA256 = input.request["request_sha256"].(string)
	result.RequestedAtUnixMS = input.request["requested_at_unix_ms"].(int64)
	result.RequestExpiresAtUnixMS = input.request["expires_at_unix_ms"].(int64)
	populateRevocationFacts(&result, snapshot)
	return result, nil
}

func ledgerFacts(ledger *Ledger) (facts, error) {
	if ledger == nil || ledger.root == nil {
		return facts{}, fmt.Errorf("ledger or trust root is nil")
	}
	if _, err := validateLedger(ledger.document, ledger.root.document); err != nil {
		return facts{}, err
	}
	result := rootFacts(ledger.root.document)
	result.Kind = "ledger"
	populateLedgerFacts(&result, ledger.document)
	return result, nil
}

func bundleFacts(bundle *Bundle) (facts, error) {
	if bundle == nil || bundle.root == nil {
		return facts{}, fmt.Errorf("bundle or trust root is nil")
	}
	if _, _, err := validateBundle(bundle.document); err != nil {
		return facts{}, err
	}
	result := rootFacts(bundle.root.document)
	result.Kind = "bundle"
	binding := bundle.document["proposal_binding"].(map[string]any)
	result.ADRID = binding["adr_id"].(string)
	receipt := bundle.document["authorization_receipt"].(map[string]any)
	populateReceiptFacts(&result, receipt)
	result.DeliveryDisposition = bundle.document["authorization_result"].(map[string]any)["delivery_disposition"].(string)
	populateLedgerFacts(&result, bundle.document["authorization_ledger"].(map[string]any))
	return result, nil
}

func populateReceiptFacts(result *facts, receipt map[string]any) {
	result.AuthorizationDecision = receipt["authorization_decision"].(string)
	result.AuthorizationExpiresAtUnixMS = receipt["authorization_expires_at_unix_ms"].(int64)
	result.AuthorizationReasonCodes = detachedStrings(receipt["reason_codes"].([]any))
	result.ProposalBindingSHA256 = receipt["proposal_binding_sha256"].(string)
	result.QualifyingApprovalIDs = detachedStrings(receipt["qualifying_approval_ids"].([]any))
	result.ReceiptEvaluatedAtUnixMS = receipt["evaluated_at_unix_ms"].(int64)
	result.ReceiptID = receipt["receipt_id"].(string)
	result.ReceiptLedgerSequence = receipt["ledger_sequence"].(int64)
	result.ReceiptPriorSHA256 = optionalStringCopy(receipt["prior_receipt_sha256"])
	result.ReceiptSHA256 = receipt["receipt_sha256"].(string)
	result.RecordKeySHA256 = receipt["record_key_sha256"].(string)
	result.RequestSHA256 = receipt["request_sha256"].(string)
	result.RevocationSequence = receipt["revocation_sequence"].(int64)
	result.RevocationSHA256 = receipt["revocation_sha256"].(string)
}

func populateRevocationFacts(result *facts, snapshot map[string]any) {
	result.RevocationSequence = snapshot["revocation_sequence"].(int64)
	result.RevocationSHA256 = snapshot["revocation_sha256"].(string)
	result.RevocationEffectiveAtUnixMS = snapshot["effective_at_unix_ms"].(int64)
	result.RevocationExpiresAtUnixMS = snapshot["expires_at_unix_ms"].(int64)
}

func populateLedgerFacts(result *facts, ledger map[string]any) {
	result.ClockHighWaterUnixMS = ledger["clock_high_water_unix_ms"].(int64)
	result.LedgerSHA256 = ledger["ledger_sha256"].(string)
	result.RevocationHighWaterSequence = ledger["revocation_high_water_sequence"].(int64)
	result.RevocationHighWaterSHA256 = ledger["revocation_high_water_sha256"].(string)
	entries := ledger["entries"].([]any)
	result.ReplayRecords = make([]replayFact, len(entries))
	for index, item := range entries {
		entry := item.(map[string]any)
		request := entry["request"].(map[string]any)
		receipt := entry["receipt"].(map[string]any)
		result.ReplayRecords[index] = replayFact{
			AuthorizationDecision: receipt["authorization_decision"].(string),
			ProposalBindingSHA256: receipt["proposal_binding_sha256"].(string),
			ReceiptSHA256:         receipt["receipt_sha256"].(string),
			RecordKeySHA256:       receipt["record_key_sha256"].(string),
			RequestSHA256:         request["request_sha256"].(string), Sequence: entry["sequence"].(int64)}
	}
}

func optionalStringCopy(value any) *string {
	if value == nil {
		return nil
	}
	text := value.(string)
	return &text
}

func detachedStrings(values []any) []string {
	result := make([]string, len(values))
	for index, value := range values {
		result[index] = value.(string)
	}
	return result
}
