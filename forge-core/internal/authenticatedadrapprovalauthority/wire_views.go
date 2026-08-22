package authenticatedadrapprovalauthority

import (
	"encoding/base64"
	"encoding/json"
	"fmt"
)

type signatureView struct {
	KeyID string `json:"key_id"`
}

type revocationView struct {
	EffectiveAtUnixMS  int64         `json:"effective_at_unix_ms"`
	ExpiresAtUnixMS    int64         `json:"expires_at_unix_ms"`
	RevokedApprovalIDs []string      `json:"revoked_approval_ids"`
	RevokedKeyIDs      []string      `json:"revoked_key_ids"`
	Sequence           int64         `json:"revocation_sequence"`
	SHA256             string        `json:"revocation_sha256"`
	Signature          signatureView `json:"signature"`
}

type approvalView struct {
	ApprovalID     string `json:"approval_id"`
	Decision       string `json:"decision"`
	AuthorityProof struct {
		KeyID string `json:"key_id"`
	} `json:"authority_proof"`
}

type requestView struct {
	ApprovalRecords      []approvalView `json:"approval_records"`
	ExpectedLedgerSHA256 *string        `json:"expected_ledger_sha256"`
	ExpectedNextSequence int64          `json:"expected_next_sequence"`
	ExpiresAtUnixMS      int64          `json:"expires_at_unix_ms"`
	IdempotencyKey       string         `json:"idempotency_key"`
	RequestedAtUnixMS    int64          `json:"requested_at_unix_ms"`
	RevocationSequence   int64          `json:"revocation_sequence"`
	RevocationSHA256     string         `json:"revocation_sha256"`
	Signature            signatureView  `json:"signature"`
}

type policyView struct {
	Disposition string        `json:"disposition"`
	Signature   signatureView `json:"signature"`
	Threshold   int64         `json:"threshold"`
	Validity    struct {
		ExpiresAtUnixMS int64 `json:"expires_at_unix_ms"`
		NotBeforeUnixMS int64 `json:"not_before_unix_ms"`
	} `json:"validity"`
}

type receiptView struct {
	AuthorizationDecision        string        `json:"authorization_decision"`
	AuthorizationExpiresAtUnixMS int64         `json:"authorization_expires_at_unix_ms"`
	EvaluatedAtUnixMS            int64         `json:"evaluated_at_unix_ms"`
	ProposalBindingSHA256        string        `json:"proposal_binding_sha256"`
	QualifyingApprovalIDs        []string      `json:"qualifying_approval_ids"`
	ReceiptSHA256                string        `json:"receipt_sha256"`
	Signature                    signatureView `json:"signature"`
}

type ledgerEntryView struct {
	Policy                    json.RawMessage `json:"policy"`
	ProposalDocumentBase64URL string          `json:"proposal_document_base64url"`
	Receipt                   receiptView     `json:"receipt"`
	Request                   json.RawMessage `json:"request"`
}

type ledgerView struct {
	ClockHighWaterUnixMS        int64             `json:"clock_high_water_unix_ms"`
	Entries                     []ledgerEntryView `json:"entries"`
	LedgerSHA256                string            `json:"ledger_sha256"`
	RevocationHighWaterSequence int64             `json:"revocation_high_water_sequence"`
	RevocationHighWaterSHA256   string            `json:"revocation_high_water_sha256"`
	RevocationSnapshots         []json.RawMessage `json:"revocation_snapshots"`
	Signature                   signatureView     `json:"signature"`
}

type bundleView struct {
	Ledger  ledgerView      `json:"authorization_ledger"`
	Policy  policyView      `json:"authorization_policy"`
	Receipt receiptView     `json:"authorization_receipt"`
	Request requestView     `json:"authorization_request"`
	Root    json.RawMessage `json:"trust_root"`
}

type inputView struct {
	Latest  revocationView
	Policy  policyView
	Request requestView
}

func decodeJSONView(raw []byte, target any, label string) error {
	if err := json.Unmarshal(raw, target); err != nil {
		return fmt.Errorf("decode %s view: %w", label, err)
	}
	return nil
}

func parseInputView(policy, request []byte, snapshots [][]byte) (inputView, error) {
	var result inputView
	if len(snapshots) == 0 {
		return result, fmt.Errorf("authorization input has no revocation snapshots")
	}
	if err := decodeJSONView(policy, &result.Policy, "policy"); err != nil {
		return result, err
	}
	if err := decodeJSONView(request, &result.Request, "request"); err != nil {
		return result, err
	}
	if err := decodeJSONView(snapshots[len(snapshots)-1], &result.Latest,
		"latest revocation"); err != nil {
		return result, err
	}
	return result, nil
}

func parseLatestRevocation(ledger ledgerView) (revocationView, error) {
	var result revocationView
	if len(ledger.RevocationSnapshots) == 0 {
		return result, fmt.Errorf("ledger has no revocation snapshots")
	}
	err := decodeJSONView(ledger.RevocationSnapshots[len(ledger.RevocationSnapshots)-1],
		&result, "ledger latest revocation")
	return result, err
}

func decodeProposalView(encoded string) ([]byte, error) {
	value, err := base64.RawURLEncoding.DecodeString(encoded)
	if err != nil || base64.RawURLEncoding.EncodeToString(value) != encoded {
		return nil, fmt.Errorf("ledger proposal encoding is not canonical base64url")
	}
	return value, nil
}
