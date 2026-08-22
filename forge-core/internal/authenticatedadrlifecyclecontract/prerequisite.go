package authenticatedadrlifecyclecontract

import (
	"fmt"

	approvalcontract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

type approvalReceiptFacts struct {
	AuthorizationDecision  string
	AuthorizationExpiresAt int64
	EvaluatedAt            int64
	LedgerSequence         int64
	PriorReceiptSHA256     *string
	ProposalBindingSHA256  string
	QualifyingApprovalIDs  []string
	ReasonCodes            []string
	ReceiptSHA256          string
	RevocationSequence     int64
	RevocationSHA256       string
}

func validatePrerequisite(value any, profileHash string,
	approvalRoot *approvalcontract.TrustRoot) (map[string]any, approvalReceiptFacts, error) {
	label := "ArchitectureDecisionAcceptancePrerequisite"
	fields := []string{"api_version", "approval_trust_epoch", "approval_trust_root_sha256",
		"authorization_ledger_clock_high_water_unix_ms", "authorization_ledger_last_sequence",
		"authorization_ledger_sha256", "authorization_ledger_signature", "authorization_receipt",
		"authorization_receipt_physical_sha256", "canonicalization", "kind", "observed_at_unix_ms",
		"prerequisite_sha256", "profile_id", "proposal_binding",
		"revocation_high_water_sequence", "revocation_high_water_sha256"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, approvalReceiptFacts{}, err
	}
	if _, err = boundedCanonicalJSON(node, maxRequestBytes, label); err != nil {
		return nil, approvalReceiptFacts{}, err
	}
	if node["api_version"] != prerequisiteAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != label || node["profile_id"] != profileID {
		return nil, approvalReceiptFacts{}, fmt.Errorf("%s envelope drifted from v1", label)
	}
	binding, err := validateProposalBinding(node["proposal_binding"])
	if err != nil {
		return nil, approvalReceiptFacts{}, err
	}
	receiptFacts, err := decodeApprovalReceipt(node["authorization_receipt"], approvalRoot)
	if err != nil {
		return nil, approvalReceiptFacts{}, err
	}
	if err = validatePrerequisiteRelations(node, binding, receiptFacts, profileHash, approvalRoot); err != nil {
		return nil, approvalReceiptFacts{}, err
	}
	digest, err := prerequisiteSHA256(node)
	if err != nil || node["prerequisite_sha256"] != digest {
		return nil, approvalReceiptFacts{}, fmt.Errorf("acceptance prerequisite self digest does not match")
	}
	return node, receiptFacts, nil
}

func decodeApprovalReceipt(value any,
	root *approvalcontract.TrustRoot) (approvalReceiptFacts, error) {
	raw, err := boundedCanonicalJSON(value, maxAcceptanceBytes, "embedded authorization receipt")
	if err != nil {
		return approvalReceiptFacts{}, err
	}
	receipt, err := approvalcontract.DecodeCanonicalReceipt(raw, root)
	if err != nil {
		return approvalReceiptFacts{}, fmt.Errorf("embedded authorization receipt: %w", err)
	}
	facts, err := approvalcontract.Facts(receipt)
	if err != nil {
		return approvalReceiptFacts{}, err
	}
	return approvalReceiptFacts{
		AuthorizationDecision:  facts.AuthorizationDecision,
		AuthorizationExpiresAt: facts.AuthorizationExpiresAtUnixMS,
		EvaluatedAt:            facts.ReceiptEvaluatedAtUnixMS,
		LedgerSequence:         facts.ReceiptLedgerSequence,
		PriorReceiptSHA256:     copyStringPointer(facts.ReceiptPriorSHA256),
		ProposalBindingSHA256:  facts.ProposalBindingSHA256,
		QualifyingApprovalIDs:  append([]string(nil), facts.QualifyingApprovalIDs...),
		ReasonCodes:            append([]string(nil), facts.AuthorizationReasonCodes...),
		ReceiptSHA256:          facts.ReceiptSHA256,
		RevocationSequence:     facts.RevocationSequence,
		RevocationSHA256:       facts.RevocationSHA256,
	}, nil
}

func validatePrerequisiteRelations(node, binding map[string]any, receipt approvalReceiptFacts,
	profileHash string, root *approvalcontract.TrustRoot) error {
	if err := validatePrerequisitePins(node, root); err != nil {
		return err
	}
	if err := validatePrerequisiteReceipt(node, binding, receipt); err != nil {
		return err
	}
	return validateApprovalLedgerBinding(node, receipt, profileHash, root)
}

func validatePrerequisitePins(node map[string]any, root *approvalcontract.TrustRoot) error {
	rootSHA, epoch := root.Identity()
	if node["approval_trust_root_sha256"] != rootSHA || node["approval_trust_epoch"] != epoch {
		return fmt.Errorf("acceptance prerequisite does not bind the approval root")
	}
	if _, err := intValue(node["approval_trust_epoch"], "prerequisite.approval_trust_epoch", 1, maxInt64); err != nil {
		return err
	}
	for _, field := range []string{"authorization_ledger_sha256",
		"authorization_receipt_physical_sha256", "revocation_high_water_sha256"} {
		if _, err := shaValue(node[field], "prerequisite."+field); err != nil {
			return err
		}
	}
	return nil
}

func validatePrerequisiteReceipt(node, binding map[string]any,
	receipt approvalReceiptFacts) error {
	if receipt.AuthorizationDecision != "acceptance_transition_authorized" ||
		len(receipt.ReasonCodes) != 0 || len(receipt.QualifyingApprovalIDs) == 0 {
		return fmt.Errorf("acceptance prerequisite requires an authorized-shaped receipt")
	}
	if receipt.ProposalBindingSHA256 != binding["proposal_binding_sha256"] {
		return fmt.Errorf("approval receipt does not bind the proposed ADR")
	}
	raw, err := boundedCanonicalJSON(node["authorization_receipt"], maxAcceptanceBytes,
		"embedded authorization receipt")
	if err != nil || node["authorization_receipt_physical_sha256"] != sha256Bytes(raw) {
		return fmt.Errorf("acceptance prerequisite does not bind exact receipt bytes")
	}
	observed, err := intValue(node["observed_at_unix_ms"], "prerequisite.observed_at_unix_ms", 0, maxInt64)
	if err != nil || observed < receipt.EvaluatedAt || observed >= receipt.AuthorizationExpiresAt {
		return fmt.Errorf("declared prerequisite observation lies outside receipt validity")
	}
	return nil
}

func validateApprovalLedgerBinding(node map[string]any, receipt approvalReceiptFacts,
	profileHash string, root *approvalcontract.TrustRoot) error {
	last, err := intValue(node["authorization_ledger_last_sequence"],
		"prerequisite.authorization_ledger_last_sequence", 1, maxApprovalEntries)
	if err != nil {
		return err
	}
	clock, err := intValue(node["authorization_ledger_clock_high_water_unix_ms"],
		"prerequisite.authorization_ledger_clock_high_water_unix_ms", 0, maxInt64)
	if err != nil {
		return err
	}
	revocation, err := intValue(node["revocation_high_water_sequence"],
		"prerequisite.revocation_high_water_sequence", 1, maxApprovalSnapshots)
	if err != nil {
		return err
	}
	if err = validateApprovalHighWater(node, receipt, last, clock, revocation); err != nil {
		return err
	}
	return validateApprovalLedgerSignature(node, profileHash, root)
}

func validateApprovalHighWater(node map[string]any, receipt approvalReceiptFacts,
	last, clock, revocation int64) error {
	observed := node["observed_at_unix_ms"].(int64)
	if last < receipt.LedgerSequence || clock < receipt.EvaluatedAt || clock > observed {
		return fmt.Errorf("approval ledger sequence or clock high-water is impossible")
	}
	if (receipt.LedgerSequence == 1) != (receipt.PriorReceiptSHA256 == nil) {
		return fmt.Errorf("approval receipt prior link differs from its own ledger sequence")
	}
	if revocation < receipt.RevocationSequence {
		return fmt.Errorf("approval revocation high-water regresses below the receipt")
	}
	if revocation == receipt.RevocationSequence &&
		node["revocation_high_water_sha256"] != receipt.RevocationSHA256 {
		return fmt.Errorf("equal revocation sequence requires the exact receipt digest")
	}
	return nil
}

func validateApprovalLedgerSignature(node map[string]any, profileHash string,
	root *approvalcontract.TrustRoot) error {
	facts, err := approvalcontract.Facts(root)
	if err != nil {
		return err
	}
	var keyID string
	for _, key := range facts.RootKeys {
		if key.Usage == "approval_authorization_state_sign" {
			if keyID != "" {
				return fmt.Errorf("approval root repeats its state-signing key")
			}
			keyID = key.KeyID
		}
	}
	if keyID == "" {
		return fmt.Errorf("approval root lacks its state-signing key")
	}
	return validateApprovalSignature(node["authorization_ledger_signature"],
		"prerequisite.authorization_ledger_signature", profileHash, keyID)
}

func validateApprovalSignature(value any, label, profileHash, expectedKey string) error {
	node, err := requireKeys(value, label, "key_id", "profile_id", "profile_sha256",
		"signature_base64url")
	if err != nil {
		return err
	}
	keyID, err := textValue(node["key_id"], label+".key_id", 160)
	if err != nil || keyID != expectedKey {
		return fmt.Errorf("%s uses the wrong approval trust-root key", label)
	}
	if node["profile_id"] != signatureProfileID || node["profile_sha256"] != profileHash {
		return fmt.Errorf("%s does not bind the signature profile", label)
	}
	if _, err = shaValue(node["profile_sha256"], label+".profile_sha256"); err != nil {
		return err
	}
	_, err = fixedBase64URL(node["signature_base64url"], label+".signature_base64url", 64)
	return err
}

func copyStringPointer(value *string) *string {
	if value == nil {
		return nil
	}
	result := *value
	return &result
}
