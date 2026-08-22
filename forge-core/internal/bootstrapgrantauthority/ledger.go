package bootstrapgrantauthority

import "fmt"

var ledgerKeys = []string{
	"api_version", "canonicalization", "clock_high_water_unix_ms", "entries", "kind",
	"ledger_sha256", "profile_id", "signature", "trust_epoch", "trust_root_sha256",
}
var ledgerEntryKeys = []string{"grant", "policy", "receipt", "request", "sequence"}

// Ledger is a fully replayed and Ed25519-authenticated issuance snapshot.
type Ledger struct {
	document map[string]any
	records  map[string]*record
}

type record struct {
	grant   *Grant
	policy  *Policy
	receipt *Receipt
	request *Request
}

// DecodeLedger strictly replays every signature, digest, relation, and chain link.
func DecodeLedger(data []byte, trust *Trust) (*Ledger, error) {
	if trust == nil {
		return nil, fmt.Errorf("Trust is required")
	}
	document, err := decodeCanonical(data, maxLedgerBytes)
	if err != nil {
		return nil, err
	}
	return validateLedger(document, trust)
}

// AppendLedger returns a newly signed complete snapshot containing receipt.
func AppendLedger(current *Ledger, policy *Policy, request *Request, grant *Grant,
	receipt *Receipt, issuer *Issuer) (*Ledger, error) {
	if policy == nil || request == nil || receipt == nil || issuer == nil {
		return nil, fmt.Errorf("complete issuance entry and issuer are required")
	}
	entries := []any{}
	highWater := int64(0)
	if current != nil {
		currentEntries, _ := arrayValue(current.document, "entries")
		entries = cloneNode(currentEntries).([]any)
		highWater, _ = intValue(current.document, "clock_high_water_unix_ms")
	}
	if len(entries) >= maxLedgerItems || receipt.document["ledger_sequence"] != int64(len(entries)+1) {
		return nil, fmt.Errorf("receipt sequence cannot append to ledger")
	}
	entry := map[string]any{"grant": nullableGrantNode(grant), "policy": cloneNode(policy.document),
		"receipt": cloneNode(receipt.document), "request": cloneNode(request.document),
		"sequence": int64(len(entries) + 1)}
	entries = append(entries, entry)
	stored, _ := intValue(receipt.document, "stored_at_unix_ms")
	requested, _ := intValue(request.document, "requested_at_unix_ms")
	highWater = maxInt64(highWater, maxInt64(stored, requested))
	document := ledgerDocument(entries, highWater, issuer.trust)
	digest, err := selfDigest(ledgerDomain, document, "ledger_sha256", maxLedgerBytes,
		"GrantIssuanceLedger", true)
	if err != nil {
		return nil, err
	}
	document["ledger_sha256"] = digest
	signature, _ := objectValue(document, "signature")
	signature["signature_base64url"], err = issuer.sign(ledgerSignatureDomain, digest)
	if err != nil {
		return nil, err
	}
	return validateLedger(document, issuer.trust)
}

func ledgerDocument(entries []any, highWater int64, trust *Trust) map[string]any {
	return map[string]any{
		"api_version": ledgerAPI, "canonicalization": canonicalization,
		"clock_high_water_unix_ms": highWater, "entries": entries,
		"kind": "GrantIssuanceLedger", "ledger_sha256": "", "profile_id": contractProfileID,
		"signature": signaturePlaceholder(trust), "trust_epoch": trust.epoch,
		"trust_root_sha256": trust.rootHash,
	}
}

func validateLedger(document map[string]any, trust *Trust) (*Ledger, error) {
	if err := requireKeys(document, ledgerKeys...); err != nil {
		return nil, fmt.Errorf("GrantIssuanceLedger: %w", err)
	}
	if err := validateDocumentEnvelope(document, ledgerAPI, "GrantIssuanceLedger"); err != nil {
		return nil, err
	}
	if err := validateAuthorityBinding(document, trust, "Ledger"); err != nil {
		return nil, err
	}
	highWater, err := intValue(document, "clock_high_water_unix_ms")
	if err != nil || highWater < 0 {
		return nil, fmt.Errorf("Ledger clock high-water is invalid")
	}
	entries, err := arrayValue(document, "entries")
	if err != nil || len(entries) < 1 || len(entries) > maxLedgerItems {
		return nil, fmt.Errorf("Ledger entries must contain 1..%d records", maxLedgerItems)
	}
	records, err := validateLedgerEntries(entries, highWater, trust)
	if err != nil {
		return nil, err
	}
	if err = validateSignedDocument(document, "ledger_sha256", ledgerDomain,
		ledgerSignatureDomain, maxLedgerBytes, "Ledger", trust, "grant_issue"); err != nil {
		return nil, err
	}
	return &Ledger{document: document, records: records}, nil
}

func validateLedgerEntries(entries []any, highWater int64,
	trust *Trust) (map[string]*record, error) {
	records := make(map[string]*record, len(entries))
	var prior any
	for index, value := range entries {
		entry, ok := value.(map[string]any)
		if !ok || requireKeys(entry, ledgerEntryKeys...) != nil || entry["sequence"] != int64(index+1) {
			return nil, fmt.Errorf("Ledger entry %d shape or sequence is invalid", index)
		}
		record, err := validateLedgerEntry(entry, highWater, prior, trust)
		if err != nil {
			return nil, fmt.Errorf("Ledger entry %d: %w", index, err)
		}
		key := record.receipt.document["record_key_sha256"].(string)
		if _, duplicate := records[key]; duplicate {
			return nil, fmt.Errorf("Ledger contains duplicate idempotency record key")
		}
		records[key] = record
		prior = record.receipt.document["receipt_sha256"]
	}
	return records, nil
}

func validateLedgerEntry(entry map[string]any, highWater int64, prior any,
	trust *Trust) (*record, error) {
	policyDocument, policyOK := entry["policy"].(map[string]any)
	requestDocument, requestOK := entry["request"].(map[string]any)
	receiptDocument, receiptOK := entry["receipt"].(map[string]any)
	if !policyOK || !requestOK || !receiptOK {
		return nil, fmt.Errorf("embedded Policy, Request, or Receipt is not an object")
	}
	if err := validatePolicy(policyDocument, trust); err != nil {
		return nil, err
	}
	if err := validateRequest(requestDocument, trust); err != nil {
		return nil, err
	}
	policy, request := &Policy{policyDocument}, &Request{requestDocument}
	if err := validatePolicyRequest(policyDocument, requestDocument); err != nil {
		return nil, err
	}
	if err := validateReceipt(receiptDocument, trust); err != nil {
		return nil, err
	}
	receipt := &Receipt{receiptDocument}
	grant, err := validateEmbeddedGrant(entry["grant"], policy, request, receipt, trust)
	if err != nil {
		return nil, err
	}
	if err = validateLedgerEntryRelations(entry, policy, request, grant, receipt, highWater, prior); err != nil {
		return nil, err
	}
	return &record{grant: grant, policy: policy, receipt: receipt, request: request}, nil
}

func validateEmbeddedGrant(value any, policy *Policy, request *Request,
	receipt *Receipt, trust *Trust) (*Grant, error) {
	if value == nil {
		if receipt.document["decision"] != "denied" {
			return nil, fmt.Errorf("only denied entry may omit Grant")
		}
		return nil, nil
	}
	document, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("embedded Grant is not an object")
	}
	stored, _ := intValue(receipt.document, "stored_at_unix_ms")
	grant := &Grant{document}
	if err := validateGrantRelations(grant, policy, request, stored, trust); err != nil {
		return nil, err
	}
	return grant, nil
}

func validateLedgerEntryRelations(entry map[string]any, policy *Policy, request *Request,
	grant *Grant, receipt *Receipt, highWater int64, prior any) error {
	if receipt.document["ledger_sequence"] != entry["sequence"] ||
		receipt.document["prior_receipt_sha256"] != prior {
		return fmt.Errorf("Receipt sequence or prior digest chain is invalid")
	}
	if err := validateReceiptRelations(receipt, policy, request, grant); err != nil {
		return err
	}
	stored, _ := intValue(receipt.document, "stored_at_unix_ms")
	requested, _ := intValue(request.document, "requested_at_unix_ms")
	if highWater < stored || highWater < requested {
		return fmt.Errorf("Ledger high-water is below an observed timestamp")
	}
	return validateIssuanceTime(policy.document, request.document, stored)
}

func nullableGrantNode(grant *Grant) any {
	if grant == nil {
		return nil
	}
	return cloneNode(grant.document)
}

func maxInt64(left, right int64) int64 {
	if left > right {
		return left
	}
	return right
}
