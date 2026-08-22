package bootstrapgrantauthority

import "fmt"

var receiptKeys = []string{
	"api_version", "canonicalization", "decision", "denial_reason", "grant_envelope_sha256",
	"grant_id", "grant_sha256", "kind", "ledger_sequence", "policy_sha256",
	"prior_receipt_sha256", "profile_id", "receipt_sha256", "record_key_sha256",
	"request_sha256", "signature", "stored_at_unix_ms", "trust_epoch", "trust_root_sha256",
}

// Receipt is a fully validated and authenticated issuance-only receipt.
type Receipt struct{ document map[string]any }

// IssueReceipt signs an issued or authenticated-policy-denied receipt.
func IssueReceipt(policy *Policy, request *Request, grant *Grant, sequence int64,
	priorReceiptSHA256 *string, storedAt int64, issuer *Issuer) (*Receipt, error) {
	if policy == nil || request == nil || issuer == nil {
		return nil, fmt.Errorf("Policy, Request, and issuer are required")
	}
	if err := validateSigningInputs(policy, request, issuer.trust); err != nil {
		return nil, err
	}
	if err := validateIssuanceTime(policy.document, request.document, storedAt); err != nil {
		return nil, err
	}
	if grant != nil {
		if err := validateGrantRelations(grant, policy, request, storedAt, issuer.trust); err != nil {
			return nil, err
		}
	}
	if sequence < 1 || storedAt < 0 {
		return nil, fmt.Errorf("receipt sequence or durable time is invalid")
	}
	document, err := buildReceipt(policy, request, grant, sequence, priorReceiptSHA256, storedAt, issuer.trust)
	if err != nil {
		return nil, err
	}
	digest, err := selfDigest(receiptDomain, document, "receipt_sha256", maxReceiptBytes,
		"GrantIssuanceReceipt", true)
	if err != nil {
		return nil, err
	}
	document["receipt_sha256"] = digest
	signature, _ := objectValue(document, "signature")
	signature["signature_base64url"], err = issuer.sign(receiptSignatureDomain, digest)
	if err != nil {
		return nil, err
	}
	receipt := &Receipt{document: document}
	if err = validateReceipt(receipt.document, issuer.trust); err != nil {
		return nil, err
	}
	if err = validateReceiptRelations(receipt, policy, request, grant); err != nil {
		return nil, err
	}
	return receipt, nil
}

func buildReceipt(policy *Policy, request *Request, grant *Grant, sequence int64,
	prior *string, storedAt int64, trust *Trust) (map[string]any, error) {
	decision := policy.document["disposition"].(string)
	denialReason, grantID, grantHash, envelopeHash := any(nil), any(nil), any(nil), any(nil)
	if decision == "allow" {
		if grant == nil {
			return nil, fmt.Errorf("allow Policy receipt requires a Grant")
		}
		decision = "issued"
		grantID, grantHash = grant.document["grant_id"], grant.document["grant_sha256"]
		computed, err := grantEnvelopeSHA256(grant.document)
		if err != nil {
			return nil, err
		}
		envelopeHash = computed
	} else {
		if grant != nil {
			return nil, fmt.Errorf("deny Policy receipt cannot include a Grant")
		}
		decision, denialReason = "denied", "policy_denied"
	}
	return receiptDocument(policy, request, sequence, prior, storedAt, trust,
		decision, denialReason, grantID, grantHash, envelopeHash), nil
}

func receiptDocument(policy *Policy, request *Request, sequence int64, prior *string,
	storedAt int64, trust *Trust, decision string, denialReason, grantID, grantHash,
	envelopeHash any) map[string]any {
	var priorValue any
	if prior != nil {
		priorValue = *prior
	}
	return map[string]any{
		"api_version": receiptAPI, "canonicalization": canonicalization,
		"decision": decision, "denial_reason": denialReason,
		"grant_envelope_sha256": envelopeHash, "grant_id": grantID, "grant_sha256": grantHash,
		"kind": "GrantIssuanceReceipt", "ledger_sequence": sequence,
		"policy_sha256": policy.document["policy_sha256"], "prior_receipt_sha256": priorValue,
		"profile_id": contractProfileID, "receipt_sha256": "",
		"record_key_sha256": recordKey(request.document["idempotency_key"].(string)),
		"request_sha256":    request.document["request_sha256"],
		"signature":         signaturePlaceholder(trust), "stored_at_unix_ms": storedAt,
		"trust_epoch": trust.epoch, "trust_root_sha256": trust.rootHash,
	}
}

func signaturePlaceholder(trust *Trust) map[string]any {
	return map[string]any{
		"key_id": trust.keys["grant_issue"].id, "profile_id": signatureProfile,
		"profile_sha256": trust.profileHash, "signature_base64url": "",
	}
}

func validateReceipt(document map[string]any, trust *Trust) error {
	if err := requireKeys(document, receiptKeys...); err != nil {
		return fmt.Errorf("GrantIssuanceReceipt: %w", err)
	}
	if err := validateDocumentEnvelope(document, receiptAPI, "GrantIssuanceReceipt"); err != nil {
		return err
	}
	if err := validateReceiptFields(document); err != nil {
		return err
	}
	if err := validateAuthorityBinding(document, trust, "Receipt"); err != nil {
		return err
	}
	return validateSignedDocument(document, "receipt_sha256", receiptDomain,
		receiptSignatureDomain, maxReceiptBytes, "Receipt", trust, "grant_issue")
}

func validateReceiptFields(document map[string]any) error {
	decision, err := stringValue(document, "decision")
	if err != nil || !oneOf(decision, "denied", "issued") {
		return fmt.Errorf("Receipt decision is invalid")
	}
	sequence, sequenceErr := intValue(document, "ledger_sequence")
	stored, storedErr := intValue(document, "stored_at_unix_ms")
	if sequenceErr != nil || sequence < 1 || storedErr != nil || stored < 0 {
		return fmt.Errorf("Receipt sequence or durable time is invalid")
	}
	for _, field := range []string{"policy_sha256", "receipt_sha256", "record_key_sha256",
		"request_sha256", "trust_root_sha256"} {
		if err := validateHashField(document, field, "Receipt "+field); err != nil {
			return err
		}
	}
	if err := validateNullableHash(document, "prior_receipt_sha256"); err != nil {
		return err
	}
	return validateReceiptDecision(document, decision)
}

func validateReceiptDecision(document map[string]any, decision string) error {
	fields := []string{"grant_envelope_sha256", "grant_sha256"}
	if decision == "denied" {
		if document["denial_reason"] != "policy_denied" || document["grant_id"] != nil ||
			document[fields[0]] != nil || document[fields[1]] != nil {
			return fmt.Errorf("denied Receipt Grant fields are invalid")
		}
		return nil
	}
	if document["denial_reason"] != nil {
		return fmt.Errorf("issued Receipt cannot contain a denial reason")
	}
	for _, field := range fields {
		if err := validateHashField(document, field, "Receipt "+field); err != nil {
			return err
		}
	}
	grantID, err := stringValue(document, "grant_id")
	grantHash, hashErr := stringValue(document, "grant_sha256")
	if err != nil || hashErr != nil || grantID != "capability-grant-"+grantHash {
		return fmt.Errorf("issued Receipt Grant identity is invalid")
	}
	return nil
}

func validateReceiptRelations(receipt *Receipt, policy *Policy, request *Request,
	grant *Grant) error {
	document := receipt.document
	expected := "issued"
	if policy.document["disposition"] == "deny" {
		expected = "denied"
	}
	if document["decision"] != expected || document["policy_sha256"] != policy.document["policy_sha256"] ||
		document["request_sha256"] != request.document["request_sha256"] ||
		document["trust_root_sha256"] != request.document["trust_root_sha256"] ||
		document["trust_epoch"] != request.document["trust_epoch"] ||
		document["record_key_sha256"] != recordKey(request.document["idempotency_key"].(string)) {
		return fmt.Errorf("Receipt does not bind Policy, Request, record key, and root")
	}
	stored, _ := intValue(document, "stored_at_unix_ms")
	start, _ := intValue(request.document, "requested_at_unix_ms")
	end, _ := intValue(request.document, "expires_at_unix_ms")
	if stored < start || stored >= end {
		return fmt.Errorf("Receipt durable time is outside Request freshness")
	}
	return validateReceiptGrant(document, grant)
}

func validateReceiptGrant(receipt map[string]any, grant *Grant) error {
	if grant == nil {
		if receipt["decision"] != "denied" {
			return fmt.Errorf("only denied Receipt may omit Grant")
		}
		return nil
	}
	envelope, err := grantEnvelopeSHA256(grant.document)
	if err != nil || receipt["grant_id"] != grant.document["grant_id"] ||
		receipt["grant_sha256"] != grant.document["grant_sha256"] ||
		receipt["grant_envelope_sha256"] != envelope {
		return fmt.Errorf("Receipt does not bind the complete Grant")
	}
	return nil
}

func validateNullableHash(document map[string]any, field string) error {
	if document[field] == nil {
		return nil
	}
	return validateHashField(document, field, "Receipt "+field)
}
