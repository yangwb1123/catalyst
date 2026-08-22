package bootstraprepoexecutionauthority

import "fmt"

var receiptKeys = []string{"api_version", "canonicalization", "effect_intent_receipt_sha256",
	"execution_policy_sha256", "execution_result_sha256", "execution_trust_epoch",
	"execution_trust_root_sha256", "grant_envelope_sha256", "grant_id",
	"grant_issuance_receipt_sha256", "grant_sha256", "idempotency_record_key_sha256",
	"invocation_id", "invocation_sha256", "issuance_trust_epoch", "issuance_trust_root_sha256",
	"kind", "ledger_sequence", "manifest_sha256", "prior_usage_receipt_sha256", "profile_id",
	"reason_code", "receipt_sha256", "recorded_at_unix_ms", "reservation_receipt_sha256",
	"requested_action_sha256", "result_metadata_sha256", "signature", "state"}

var failureReasons = []string{"content_mismatch", "cooperative_timeout_exceeded",
	"repository_identity_changed", "repository_read_failed"}
var quarantineReasons = []string{"effect_outcome_uncertain", "orphaned_effect_intent",
	"orphaned_reserved_no_repo_io"}

// Receipt is one authenticated durable usage-state transition.
type Receipt struct{ document map[string]any }

// IssueReceipt creates the next transition; it does not persist it.
func IssueReceipt(current *Ledger, state string, policy *Policy, invocation *Invocation,
	manifest *Manifest, metadata *Metadata, recordedAt int64, reasonCode string,
	signer *Signer) (*Receipt, error) {
	if policy == nil || invocation == nil || manifest == nil || signer == nil || signer.trust == nil {
		return nil, fmt.Errorf("complete authenticated inputs and signer are required")
	}
	if current != nil && current.trust.rootHash != signer.trust.rootHash {
		return nil, fmt.Errorf("Ledger and signer Trust differ")
	}
	if policy.grant == nil {
		return nil, fmt.Errorf("ExecutionPolicy lacks an authenticated issued Grant")
	}
	if err := validateTransitionInputs(policy, invocation, manifest, signer.trust,
		policy.grant); err != nil {
		return nil, err
	}
	transition, err := nextTransition(current, state, policy, invocation, manifest)
	if err != nil {
		return nil, err
	}
	if err = validateTransitionTime(state, policy, invocation, transition.prior, recordedAt); err != nil {
		return nil, err
	}
	document, err := buildReceiptDocument(state, policy, invocation, manifest, metadata,
		transition, recordedAt, reasonCode, signer.trust)
	if err != nil {
		return nil, err
	}
	digest, err := selfDigest(receiptDomain, document, "receipt_sha256", maxReceiptBytes,
		"BootstrapRepoReadUsageReceipt", true, "")
	if err != nil {
		return nil, err
	}
	document["receipt_sha256"] = digest
	signature := document["signature"].(map[string]any)
	signature["signature_base64url"], err = signer.sign(receiptSignatureDomain, digest)
	if err != nil {
		return nil, err
	}
	receipt := &Receipt{document}
	if err = validateReceipt(document, signer.trust); err != nil {
		return nil, err
	}
	if err = validateReceiptTransition(receipt, transition, policy, invocation, manifest, metadata); err != nil {
		return nil, err
	}
	return receipt, nil
}

func validateTransitionInputs(policy *Policy, invocation *Invocation, manifest *Manifest,
	trust *Trust, grant *issuedGrant) error {
	if err := validatePolicy(policy.document, trust, grant, manifest); err != nil {
		return err
	}
	return validateInvocation(invocation.document, trust, grant, manifest, policy)
}

func (receipt *Receipt) canonicalDocument() map[string]any { return cloneDocument(receipt.document) }

type transitionContext struct {
	sequence                   int64
	prior, reservation, intent *Receipt
}

func nextTransition(current *Ledger, state string, policy *Policy, invocation *Invocation,
	manifest *Manifest) (transitionContext, error) {
	context := transitionContext{sequence: 1}
	if state == "reserved_no_repo_io" && current != nil &&
		len(current.entries) > maxLedgerItems-3 {
		return transitionContext{}, fmt.Errorf("UsageLedger lacks capacity for a complete usage group")
	}
	if current != nil {
		if len(current.entries) == 0 {
			return transitionContext{}, fmt.Errorf("current UsageLedger is invalid")
		}
		context.sequence = int64(len(current.entries) + 1)
		context.prior = current.entries[len(current.entries)-1].receipt
	}
	active := activeGroup(current)
	if state == "reserved_no_repo_io" {
		if active != nil || ledgerIdentityUsed(current, invocation) {
			return transitionContext{}, fmt.Errorf("Grant or idempotency identity is active or already consumed")
		}
		if err := ensureReservationCapacity(current, policy, invocation, manifest); err != nil {
			return transitionContext{}, err
		}
		return context, nil
	}
	if active == nil || !sameInvocation(active.invocation, invocation) ||
		!sameCanonical(active.policy.document, policy.document) ||
		!sameCanonical(active.manifest.document, manifest.document) {
		return transitionContext{}, fmt.Errorf("transition does not continue the active usage group")
	}
	context.reservation, context.intent = active.reservation, active.intent
	if state == "effect_intent" && active.intent == nil {
		return context, nil
	}
	if state == "quarantined" && active.intent == nil {
		return context, nil
	}
	if oneOf(state, "completed", "failed_consumed", "quarantined") && active.intent != nil {
		return context, nil
	}
	return transitionContext{}, fmt.Errorf("usage state transition is invalid")
}

func ensureReservationCapacity(current *Ledger, policy *Policy, invocation *Invocation,
	manifest *Manifest) error {
	currentSize := 0
	if current != nil {
		encoded, err := canonicalJSON(current.document)
		if err != nil {
			return err
		}
		currentSize = len(encoded)
	}
	inputSize := 0
	for _, document := range []map[string]any{policy.document, invocation.document, manifest.document} {
		encoded, err := canonicalJSON(document)
		if err != nil {
			return err
		}
		inputSize += len(encoded)
	}
	worstCase := currentSize + inputSize + 3*maxReceiptBytes + maxMetadataBytes +
		reservationOverheadBytes
	if worstCase > maxLedgerBytes {
		return fmt.Errorf("UsageLedger lacks byte capacity for a complete usage group")
	}
	return nil
}

func validateTransitionTime(state string, policy *Policy, invocation *Invocation,
	prior *Receipt, recordedAt int64) error {
	if recordedAt < 0 {
		return fmt.Errorf("receipt durable time is negative")
	}
	if prior != nil {
		priorTime, _ := intValue(prior.document, "recorded_at_unix_ms")
		if recordedAt < priorTime {
			return fmt.Errorf("receipt durable time regresses")
		}
	}
	if state == "reserved_no_repo_io" {
		return ValidateExecutionTime(policy, invocation, recordedAt)
	}
	return nil
}

func buildReceiptDocument(state string, policy *Policy, invocation *Invocation,
	manifest *Manifest, metadata *Metadata, transition transitionContext, recordedAt int64,
	reason string, trust *Trust) (map[string]any, error) {
	reservationHash, intentHash := nullableReceiptHash(transition.reservation), nullableReceiptHash(transition.intent)
	resultHash, metadataHash := any(nil), any(nil)
	if metadata != nil {
		resultHash = metadata.document["execution_result_sha256"]
		metadataHash = metadata.document["metadata_sha256"]
	}
	var reasonValue any
	if reason != "" {
		reasonValue = reason
	}
	request := invocation.document
	return map[string]any{"api_version": receiptAPI, "canonicalization": canonicalization,
		"effect_intent_receipt_sha256": intentHash,
		"execution_policy_sha256":      policy.document["execution_policy_sha256"],
		"execution_result_sha256":      resultHash, "execution_trust_epoch": trust.epoch,
		"execution_trust_root_sha256": trust.rootHash,
		"grant_envelope_sha256":       request["grant_envelope_sha256"], "grant_id": request["grant_id"],
		"grant_issuance_receipt_sha256": request["grant_issuance_receipt_sha256"],
		"grant_sha256":                  request["grant_sha256"],
		"idempotency_record_key_sha256": recordKey(request["idempotency_key"].(string)),
		"invocation_id":                 request["invocation_id"], "invocation_sha256": request["invocation_sha256"],
		"issuance_trust_epoch": trust.issuanceEpoch, "issuance_trust_root_sha256": trust.issuanceRootHash,
		"kind": "BootstrapRepoReadUsageReceipt", "ledger_sequence": transition.sequence,
		"manifest_sha256":            manifest.document["manifest_sha256"],
		"prior_usage_receipt_sha256": nullableReceiptHash(transition.prior), "profile_id": profileID,
		"reason_code": reasonValue, "receipt_sha256": "", "recorded_at_unix_ms": recordedAt,
		"reservation_receipt_sha256": reservationHash,
		"requested_action_sha256":    request["requested_action_sha256"],
		"result_metadata_sha256":     metadataHash, "signature": signaturePlaceholder(trust), "state": state}, nil
}

func nullableReceiptHash(receipt *Receipt) any {
	if receipt == nil {
		return nil
	}
	return receipt.document["receipt_sha256"]
}

func signaturePlaceholder(trust *Trust) map[string]any {
	key := trust.keys["execution_receipt_sign"]
	return map[string]any{"key_id": key.id, "profile_id": signatureProfile,
		"profile_sha256": trust.profileHash, "signature_base64url": ""}
}

func validateReceipt(document map[string]any, trust *Trust) error {
	if err := requireKeys(document, receiptKeys...); err != nil {
		return fmt.Errorf("BootstrapRepoReadUsageReceipt: %w", err)
	}
	if err := validateReceiptShape(document, trust); err != nil {
		return err
	}
	return validateSigned(document, "receipt_sha256", receiptDomain, receiptSignatureDomain,
		maxReceiptBytes, "UsageReceipt", trust, "execution_receipt_sign", "")
}

func validateReceiptShape(document map[string]any, trust *Trust) error {
	if err := validateEnvelope(document, receiptAPI, "BootstrapRepoReadUsageReceipt"); err != nil {
		return err
	}
	if document["profile_id"] != profileID {
		return fmt.Errorf("UsageReceipt profile is invalid")
	}
	if err := validateAuthorityBinding(document, trust, "UsageReceipt"); err != nil {
		return err
	}
	for _, field := range []string{"execution_policy_sha256", "execution_trust_root_sha256",
		"grant_envelope_sha256", "grant_issuance_receipt_sha256", "grant_sha256",
		"idempotency_record_key_sha256", "invocation_sha256", "issuance_trust_root_sha256",
		"manifest_sha256", "receipt_sha256", "requested_action_sha256"} {
		if err := validateHashField(document, field, "UsageReceipt "+field); err != nil {
			return err
		}
	}
	return validateReceiptStateFields(document)
}

func validateReceiptStateFields(document map[string]any) error {
	state, err := stringValue(document, "state")
	if err != nil || !oneOf(state, "reserved_no_repo_io", "effect_intent", "completed",
		"failed_consumed", "quarantined") {
		return fmt.Errorf("UsageReceipt state is invalid")
	}
	sequence, sequenceErr := intValue(document, "ledger_sequence")
	recorded, recordedErr := intValue(document, "recorded_at_unix_ms")
	if sequenceErr != nil || sequence < 1 || sequence > maxLedgerItems || recordedErr != nil || recorded < 0 {
		return fmt.Errorf("UsageReceipt sequence or durable time is invalid")
	}
	for _, field := range []string{"prior_usage_receipt_sha256", "reservation_receipt_sha256",
		"effect_intent_receipt_sha256", "execution_result_sha256", "result_metadata_sha256"} {
		if document[field] != nil {
			if err := validateHashField(document, field, "UsageReceipt "+field); err != nil {
				return err
			}
		}
	}
	return validateStateCombination(document, state)
}

func validateStateCombination(document map[string]any, state string) error {
	reservation, intent := document["reservation_receipt_sha256"], document["effect_intent_receipt_sha256"]
	result, metadata := document["execution_result_sha256"], document["result_metadata_sha256"]
	reason := document["reason_code"]
	switch state {
	case "reserved_no_repo_io":
		if reservation != nil || intent != nil || result != nil || metadata != nil || reason != nil {
			return fmt.Errorf("reserved receipt fields are invalid")
		}
	case "effect_intent":
		if reservation == nil || intent != nil || result != nil || metadata != nil || reason != nil {
			return fmt.Errorf("effect-intent receipt fields are invalid")
		}
	case "completed":
		if reservation == nil || intent == nil || result == nil || metadata == nil || reason != nil {
			return fmt.Errorf("completed receipt fields are invalid")
		}
	case "failed_consumed":
		if reservation == nil || intent == nil || result != nil || metadata != nil || !stringIn(reason, failureReasons) {
			return fmt.Errorf("failed-consumed receipt fields are invalid")
		}
	case "quarantined":
		if reservation == nil || result != nil || metadata != nil || !stringIn(reason, quarantineReasons) {
			return fmt.Errorf("quarantined receipt fields are invalid")
		}
	}
	return validateQuarantineCombination(document, state)
}

func validateQuarantineCombination(document map[string]any, state string) error {
	if state != "quarantined" {
		return nil
	}
	reason, _ := stringValue(document, "reason_code")
	hasIntent := document["effect_intent_receipt_sha256"] != nil
	if reason == "orphaned_reserved_no_repo_io" && hasIntent {
		return fmt.Errorf("reserved orphan quarantine cannot bind effect intent")
	}
	if reason != "orphaned_reserved_no_repo_io" && !hasIntent {
		return fmt.Errorf("post-intent quarantine must bind effect intent")
	}
	return nil
}

func stringIn(value any, allowed []string) bool {
	text, ok := value.(string)
	if !ok {
		return false
	}
	return oneOf(text, allowed...)
}

func validateReceiptTransition(receipt *Receipt, transition transitionContext, policy *Policy,
	invocation *Invocation, manifest *Manifest, metadata *Metadata) error {
	document, request := receipt.document, invocation.document
	expected := map[string]any{"execution_policy_sha256": policy.document["execution_policy_sha256"],
		"grant_envelope_sha256": request["grant_envelope_sha256"], "grant_id": request["grant_id"],
		"grant_issuance_receipt_sha256": request["grant_issuance_receipt_sha256"],
		"grant_sha256":                  request["grant_sha256"], "invocation_id": request["invocation_id"],
		"invocation_sha256": request["invocation_sha256"], "manifest_sha256": manifest.document["manifest_sha256"],
		"requested_action_sha256": request["requested_action_sha256"]}
	for field, value := range expected {
		if !sameCanonical(document[field], value) {
			return fmt.Errorf("UsageReceipt field %s differs from authenticated inputs", field)
		}
	}
	if document["idempotency_record_key_sha256"] != recordKey(request["idempotency_key"].(string)) ||
		document["ledger_sequence"] != transition.sequence ||
		document["prior_usage_receipt_sha256"] != nullableReceiptHash(transition.prior) ||
		document["reservation_receipt_sha256"] != nullableReceiptHash(transition.reservation) ||
		document["effect_intent_receipt_sha256"] != nullableReceiptHash(transition.intent) {
		return fmt.Errorf("UsageReceipt chain or state anchors are invalid")
	}
	return validateReceiptMetadata(document, metadata)
}

func validateReceiptMetadata(document map[string]any, metadata *Metadata) error {
	if document["state"] != "completed" {
		if metadata != nil {
			return fmt.Errorf("only completed transition accepts ResultMetadata")
		}
		return nil
	}
	if metadata == nil || document["execution_result_sha256"] != metadata.document["execution_result_sha256"] ||
		document["result_metadata_sha256"] != metadata.document["metadata_sha256"] ||
		document["manifest_sha256"] != metadata.document["manifest_sha256"] {
		return fmt.Errorf("completed UsageReceipt does not bind exact ResultMetadata")
	}
	return nil
}
