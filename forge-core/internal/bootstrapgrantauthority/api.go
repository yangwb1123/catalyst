package bootstrapgrantauthority

import (
	"errors"
	"fmt"
)

var errIdempotencyConflict = errors.New("idempotency key already binds different Policy or Request")

type result struct{ document map[string]any }

// FindRecord resolves an authenticated Request without changing the Ledger.
func (ledger *Ledger) FindRecord(policy *Policy, request *Request) (*record, bool, error) {
	if ledger == nil || policy == nil || request == nil {
		return nil, false, nil
	}
	key := recordKey(request.document["idempotency_key"].(string))
	record, found := ledger.records[key]
	if !found {
		return nil, false, nil
	}
	policyEqual, policyErr := sameCanonical(record.policy.document, policy.document)
	requestEqual, requestErr := sameCanonical(record.request.document, request.document)
	if policyErr != nil || requestErr != nil || !policyEqual || !requestEqual {
		return nil, true, errIdempotencyConflict
	}
	return record, true, nil
}

// NextSequence returns one for an absent Ledger and otherwise the next value.
func (ledger *Ledger) NextSequence() int64 {
	if ledger == nil {
		return 1
	}
	entries, _ := arrayValue(ledger.document, "entries")
	return int64(len(entries) + 1)
}

// PriorReceiptSHA256 returns nil only for the first Ledger entry.
func (ledger *Ledger) PriorReceiptSHA256() *string {
	if ledger == nil {
		return nil
	}
	entries, _ := arrayValue(ledger.document, "entries")
	last := entries[len(entries)-1].(map[string]any)
	receipt := last["receipt"].(map[string]any)
	digest := receipt["receipt_sha256"].(string)
	return &digest
}

// ClockHighWater returns zero for an absent Ledger.
func (ledger *Ledger) ClockHighWater() int64 {
	if ledger == nil {
		return 0
	}
	value, _ := intValue(ledger.document, "clock_high_water_unix_ms")
	return value
}

// Result builds a byte-stable replay output from a validated record.
func (record *record) Result() (*result, error) {
	if record == nil {
		return nil, fmt.Errorf("issuance record is required")
	}
	return buildResult(record.grant, record.receipt, "exact_replay")
}

// StoredResult builds the output for a newly durably stored decision.
func StoredResult(grant *Grant, receipt *Receipt) (*result, error) {
	return buildResult(grant, receipt, "stored")
}

func buildResult(grant *Grant, receipt *Receipt, disposition string) (*result, error) {
	if receipt == nil || !oneOf(disposition, "exact_replay", "stored") {
		return nil, fmt.Errorf("Receipt and supported delivery disposition are required")
	}
	if err := validateReceiptGrant(receipt.document, grant); err != nil {
		return nil, err
	}
	document := map[string]any{
		"api_version": resultAPI, "canonicalization": canonicalization,
		"delivery_disposition": disposition, "grant": nullableGrantNode(grant),
		"kind": "BootstrapGrantIssuanceResult", "receipt": cloneNode(receipt.document),
	}
	if err := validateResultShape(document); err != nil {
		return nil, err
	}
	return &result{document: document}, nil
}

func validateResultShape(document map[string]any) error {
	if err := requireKeys(document, "api_version", "canonicalization", "delivery_disposition",
		"grant", "kind", "receipt"); err != nil {
		return fmt.Errorf("BootstrapGrantIssuanceResult: %w", err)
	}
	if err := requireLiteral(document, "api_version", resultAPI); err != nil {
		return err
	}
	if err := requireLiteral(document, "canonicalization", canonicalization); err != nil {
		return err
	}
	if err := requireLiteral(document, "kind", "BootstrapGrantIssuanceResult"); err != nil {
		return err
	}
	disposition, err := stringValue(document, "delivery_disposition")
	if err != nil || !oneOf(disposition, "exact_replay", "stored") {
		return fmt.Errorf("result delivery disposition is invalid")
	}
	canonical, err := canonicalJSON(document)
	if err != nil || len(canonical) > maxResultBytes {
		return fmt.Errorf("result canonical bytes exceed limit")
	}
	return nil
}

// CanonicalLedgerJSON returns exact bytes after the Ledger has been authenticated.
func CanonicalLedgerJSON(ledger *Ledger) ([]byte, error) {
	if ledger == nil {
		return nil, fmt.Errorf("Ledger is required")
	}
	encoded, err := canonicalJSON(ledger.document)
	if err != nil || len(encoded) > maxLedgerBytes {
		return nil, fmt.Errorf("Ledger canonical bytes exceed limit")
	}
	return encoded, nil
}

// CanonicalResultJSON returns the closed issuance-only output bytes.
func CanonicalResultJSON(result *result) ([]byte, error) {
	if result == nil {
		return nil, fmt.Errorf("Result is required")
	}
	if err := validateResultShape(result.document); err != nil {
		return nil, err
	}
	return canonicalJSON(result.document)
}

// CanonicalProfileJSON returns the built-in frozen signature profile.
func canonicalProfileJSON() ([]byte, error) {
	return canonicalJSON(frozenSignatureProfile())
}

// ValidateIssuanceTime rejects invalid or expired inputs before key access.
func ValidateIssuanceTime(policy *Policy, request *Request, storedAt int64) error {
	if policy == nil || request == nil {
		return fmt.Errorf("Policy and Request are required")
	}
	return validateIssuanceTime(policy.document, request.document, storedAt)
}

// RootSHA256 is the externally pinned root identity.
func (trust *Trust) rootSHA256() string {
	if trust == nil {
		return ""
	}
	return trust.rootHash
}

// PolicyDisposition is allow or deny after signature authentication.
func (policy *Policy) PolicyDisposition() string {
	if policy == nil {
		return ""
	}
	return policy.document["disposition"].(string)
}

func (request *Request) requestWindow() (int64, int64) {
	if request == nil {
		return 0, 0
	}
	start, _ := intValue(request.document, "requested_at_unix_ms")
	end, _ := intValue(request.document, "expires_at_unix_ms")
	return start, end
}
