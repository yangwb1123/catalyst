package authenticatedadrapprovalauthority

import (
	"bytes"
	"crypto/sha256"
	"encoding/base64"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"hash"

	contract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

const storedAuthorizationSealDomain = "forgeos.authenticated-architecture-decision-approval.stored-authorization.internal.v1\x00"

type prerequisiteBundleProjection struct {
	Ledger                    json.RawMessage `json:"authorization_ledger"`
	Receipt                   json.RawMessage `json:"authorization_receipt"`
	ProposalBinding           json.RawMessage `json:"proposal_binding"`
	ProposalDocumentBase64URL string          `json:"proposal_document_base64url"`
	SignatureProfile          json.RawMessage `json:"signature_profile"`
	TrustRoot                 json.RawMessage `json:"trust_root"`
}

type prerequisiteLedgerProjection struct {
	ClockHighWaterUnixMS        int64             `json:"clock_high_water_unix_ms"`
	Entries                     []json.RawMessage `json:"entries"`
	LedgerSHA256                string            `json:"ledger_sha256"`
	RevocationHighWaterSequence int64             `json:"revocation_high_water_sequence"`
	RevocationHighWaterSHA256   string            `json:"revocation_high_water_sha256"`
	Signature                   json.RawMessage   `json:"signature"`
}

func newStoredAuthorization(verified *VerifiedBundle, ledgerCanonical []byte,
	trust ExternalTrust) (*StoredAuthorization, error) {
	if verified == nil || len(ledgerCanonical) == 0 {
		return nil, fmt.Errorf("verified bundle or reopened ledger is absent")
	}
	reverified, err := VerifyBundle(verified.CanonicalJSON(), trust)
	if err != nil {
		return nil, fmt.Errorf("stored bundle is not current: %w", err)
	}
	if err = requireExactStoredLedger(reverified.canonical, ledgerCanonical); err != nil {
		return nil, err
	}
	result := &StoredAuthorization{verified: *cloneVerifiedBundle(reverified),
		ledgerCanonical: cloneBytes(ledgerCanonical), trust: trust}
	result.seal = storedAuthorizationSeal(result)
	return result, nil
}

func requireExactStoredLedger(bundleCanonical, ledgerCanonical []byte) error {
	var bundle prerequisiteBundleProjection
	if err := decodeJSONView(bundleCanonical, &bundle, "stored authorization bundle"); err != nil {
		return err
	}
	if len(bundle.Ledger) == 0 || !bytes.Equal(bundle.Ledger, ledgerCanonical) {
		return fmt.Errorf("bundle ledger differs from locked reopened state")
	}
	return nil
}

// AcceptancePrerequisite returns a detached source projection only for an
// authorized, current StoredAuthorization. The returned data is not itself a
// storage proof and must not replace this capability at a mutation boundary.
func (s *StoredAuthorization) AcceptancePrerequisite() (AcceptancePrerequisiteSource, error) {
	if !validStoredAuthorization(s) {
		return AcceptancePrerequisiteSource{}, coded(codeStateRejected,
			fmt.Errorf("stored authorization capability is invalid"))
	}
	verified, err := VerifyBundle(s.verified.CanonicalJSON(), s.trust)
	if err != nil {
		return AcceptancePrerequisiteSource{}, err
	}
	if verified.AuthorizationDecision() != "acceptance_transition_authorized" {
		return AcceptancePrerequisiteSource{}, coded(codeAuthorizationNotCurrent,
			fmt.Errorf("stored outcome does not authorize acceptance"))
	}
	source, err := projectAcceptancePrerequisite(verified, s.ledgerCanonical, s.trust)
	if err != nil {
		return AcceptancePrerequisiteSource{}, coded(codeStateRejected, err)
	}
	return cloneAcceptancePrerequisiteSource(source), nil
}

func projectAcceptancePrerequisite(verified *VerifiedBundle, ledgerCanonical []byte,
	trust ExternalTrust) (AcceptancePrerequisiteSource, error) {
	var result AcceptancePrerequisiteSource
	if verified == nil {
		return result, fmt.Errorf("verified bundle is nil")
	}
	bundle, _, root, err := decodeBundleForVerification(verified.CanonicalJSON())
	if err != nil {
		return result, err
	}
	var fragments prerequisiteBundleProjection
	if err = decodeJSONView(verified.canonical, &fragments,
		"acceptance prerequisite source"); err != nil {
		return result, err
	}
	if !bytes.Equal(fragments.Ledger, ledgerCanonical) {
		return result, fmt.Errorf("prerequisite ledger differs from stored state")
	}
	var ledger prerequisiteLedgerProjection
	if err = decodeJSONView(fragments.Ledger, &ledger, "prerequisite ledger"); err != nil {
		return result, err
	}
	facts, err := authenticatePrerequisiteFacts(bundle, root, fragments.Receipt)
	if err != nil {
		return result, err
	}
	if err = requirePrerequisiteRelations(facts.Relations, ledger, trust); err != nil {
		return result, err
	}
	proposal, err := decodeStoredProposal(fragments.ProposalDocumentBase64URL)
	if err != nil {
		return result, err
	}
	return prerequisiteSource(fragments, ledger, facts, proposal, trust), nil
}

type prerequisiteAuthenticatedFacts struct {
	Relations             prerequisiteRelationFacts
	ProposalBindingSHA256 string
	ReceiptSHA256         string
	TrustRootSHA256       string
	TrustEpoch            int64
}

func authenticatePrerequisiteFacts(bundle *contract.Bundle, root *contract.TrustRoot,
	receiptJSON []byte) (prerequisiteAuthenticatedFacts, error) {
	var result prerequisiteAuthenticatedFacts
	rootFacts, err := contract.Facts(root)
	if err != nil {
		return result, err
	}
	receipt, err := contract.DecodeCanonicalReceipt(receiptJSON, root)
	if err != nil {
		return result, err
	}
	receiptChecks, err := contract.SignatureChecks(receipt)
	if err != nil || verifySignatureChecks(receiptChecks) != nil {
		return result, fmt.Errorf("standalone receipt signature rejected")
	}
	receiptFacts, err := contract.Facts(receipt)
	if err != nil {
		return result, err
	}
	bundleFacts, err := contract.Facts(bundle)
	if err != nil {
		return result, err
	}
	result = prerequisiteAuthenticatedFacts{
		ProposalBindingSHA256: receiptFacts.ProposalBindingSHA256,
		ReceiptSHA256:         receiptFacts.ReceiptSHA256,
		TrustRootSHA256:       rootFacts.TrustRootSHA256,
		TrustEpoch:            rootFacts.TrustEpoch,
		Relations: prerequisiteRelationFacts{
			AuthorizationDecision:        receiptFacts.AuthorizationDecision,
			AuthorizationExpiresAtUnixMS: receiptFacts.AuthorizationExpiresAtUnixMS,
			AuthorizationReasonCodes:     append([]string(nil), receiptFacts.AuthorizationReasonCodes...),
			QualifyingApprovalIDs:        append([]string(nil), receiptFacts.QualifyingApprovalIDs...),
			ReceiptEvaluatedAtUnixMS:     receiptFacts.ReceiptEvaluatedAtUnixMS,
			ReceiptLedgerSequence:        receiptFacts.ReceiptLedgerSequence,
			ReceiptProposalBindingSHA256: receiptFacts.ProposalBindingSHA256,
			BundleProposalBindingSHA256:  bundleFacts.ProposalBindingSHA256,
			ReceiptSHA256:                receiptFacts.ReceiptSHA256,
			BundleReceiptSHA256:          bundleFacts.ReceiptSHA256,
			ReceiptRevocationSequence:    receiptFacts.RevocationSequence,
			ReceiptRevocationSHA256:      receiptFacts.RevocationSHA256,
			TrustRootSHA256:              rootFacts.TrustRootSHA256, TrustEpoch: rootFacts.TrustEpoch,
		},
	}
	return result, nil
}

func prerequisiteSource(fragments prerequisiteBundleProjection,
	ledger prerequisiteLedgerProjection, facts prerequisiteAuthenticatedFacts,
	proposal []byte, trust ExternalTrust) AcceptancePrerequisiteSource {
	physical := sha256.Sum256(fragments.Receipt)
	return AcceptancePrerequisiteSource{
		SignatureProfileJSON:                    cloneBytes(fragments.SignatureProfile),
		ApprovalTrustRootJSON:                   cloneBytes(fragments.TrustRoot),
		ProposalDocument:                        proposal,
		ProposalBindingJSON:                     cloneBytes(fragments.ProposalBinding),
		ProposalBindingSHA256:                   facts.ProposalBindingSHA256,
		AuthorizationReceiptJSON:                cloneBytes(fragments.Receipt),
		AuthorizationReceiptSHA256:              facts.ReceiptSHA256,
		AuthorizationReceiptPhysicalSHA256:      hex.EncodeToString(physical[:]),
		ApprovalTrustRootSHA256:                 facts.TrustRootSHA256,
		ApprovalTrustEpoch:                      facts.TrustEpoch,
		AuthorizationLedgerClockHighWaterUnixMS: ledger.ClockHighWaterUnixMS,
		AuthorizationLedgerLastSequence:         int64(len(ledger.Entries)),
		AuthorizationLedgerSHA256:               ledger.LedgerSHA256,
		AuthorizationLedgerSignatureJSON:        cloneBytes(ledger.Signature),
		ObservedAtUnixMS:                        trust.ObservedAtUnixMS,
		RevocationHighWaterSequence:             ledger.RevocationHighWaterSequence,
		RevocationHighWaterSHA256:               ledger.RevocationHighWaterSHA256,
	}
}

type prerequisiteRelationFacts struct {
	AuthorizationDecision        string
	AuthorizationExpiresAtUnixMS int64
	AuthorizationReasonCodes     []string
	QualifyingApprovalIDs        []string
	ReceiptEvaluatedAtUnixMS     int64
	ReceiptLedgerSequence        int64
	ReceiptProposalBindingSHA256 string
	BundleProposalBindingSHA256  string
	ReceiptSHA256                string
	BundleReceiptSHA256          string
	ReceiptRevocationSequence    int64
	ReceiptRevocationSHA256      string
	TrustRootSHA256              string
	TrustEpoch                   int64
}

func requirePrerequisiteRelations(facts prerequisiteRelationFacts,
	ledger prerequisiteLedgerProjection, trust ExternalTrust) error {
	if facts.AuthorizationDecision != "acceptance_transition_authorized" ||
		len(facts.AuthorizationReasonCodes) != 0 || len(facts.QualifyingApprovalIDs) == 0 {
		return fmt.Errorf("receipt is not authorized-shaped")
	}
	if !constantTimeTextEqual(facts.ReceiptProposalBindingSHA256,
		facts.BundleProposalBindingSHA256) ||
		!constantTimeTextEqual(facts.ReceiptSHA256, facts.BundleReceiptSHA256) {
		return fmt.Errorf("receipt differs from bundle proposal or receipt identity")
	}
	if !constantTimeTextEqual(facts.TrustRootSHA256, trust.PinnedTrustRootSHA256) ||
		facts.TrustEpoch != trust.PinnedTrustEpoch {
		return fmt.Errorf("approval root differs from external trust")
	}
	if trust.ObservedAtUnixMS < facts.ReceiptEvaluatedAtUnixMS ||
		trust.ObservedAtUnixMS >= facts.AuthorizationExpiresAtUnixMS {
		return fmt.Errorf("observation lies outside receipt authorization window")
	}
	if int64(len(ledger.Entries)) < facts.ReceiptLedgerSequence ||
		ledger.ClockHighWaterUnixMS < facts.ReceiptEvaluatedAtUnixMS ||
		ledger.ClockHighWaterUnixMS > trust.ObservedAtUnixMS {
		return fmt.Errorf("approval ledger sequence or clock is impossible")
	}
	if ledger.RevocationHighWaterSequence < facts.ReceiptRevocationSequence ||
		ledger.RevocationHighWaterSequence != trust.RevocationHighWaterSequence ||
		!constantTimeTextEqual(ledger.RevocationHighWaterSHA256,
			trust.RevocationHighWaterSHA256) {
		return fmt.Errorf("approval revocation high-water differs")
	}
	if ledger.RevocationHighWaterSequence == facts.ReceiptRevocationSequence &&
		!constantTimeTextEqual(ledger.RevocationHighWaterSHA256,
			facts.ReceiptRevocationSHA256) {
		return fmt.Errorf("equal revocation sequence has a different digest")
	}
	return nil
}

func decodeStoredProposal(encoded string) ([]byte, error) {
	proposal, err := base64.RawURLEncoding.DecodeString(encoded)
	if err != nil || base64.RawURLEncoding.EncodeToString(proposal) != encoded {
		return nil, fmt.Errorf("proposal is not canonical base64url")
	}
	return cloneBytes(proposal), nil
}

func cloneVerifiedBundle(value *VerifiedBundle) *VerifiedBundle {
	if value == nil {
		return nil
	}
	return &VerifiedBundle{canonical: cloneBytes(value.canonical),
		authorizationDecision:        value.authorizationDecision,
		authorizationExpiresAtUnixMS: value.authorizationExpiresAtUnixMS,
		proposalBindingSHA256:        value.proposalBindingSHA256,
		receiptSHA256:                value.receiptSHA256}
}

func cloneAcceptancePrerequisiteSource(value AcceptancePrerequisiteSource) AcceptancePrerequisiteSource {
	value.SignatureProfileJSON = cloneBytes(value.SignatureProfileJSON)
	value.ApprovalTrustRootJSON = cloneBytes(value.ApprovalTrustRootJSON)
	value.ProposalDocument = cloneBytes(value.ProposalDocument)
	value.ProposalBindingJSON = cloneBytes(value.ProposalBindingJSON)
	value.AuthorizationReceiptJSON = cloneBytes(value.AuthorizationReceiptJSON)
	value.AuthorizationLedgerSignatureJSON = cloneBytes(value.AuthorizationLedgerSignatureJSON)
	return value
}

func validStoredAuthorization(value *StoredAuthorization) bool {
	if value == nil || len(value.verified.canonical) == 0 || len(value.ledgerCanonical) == 0 {
		return false
	}
	return value.seal == storedAuthorizationSeal(value)
}

func storedAuthorizationSeal(value *StoredAuthorization) [32]byte {
	if value == nil {
		return [32]byte{}
	}
	hasher := sha256.New()
	writeSealPart(hasher, []byte(storedAuthorizationSealDomain))
	writeSealPart(hasher, value.verified.canonical)
	writeSealPart(hasher, value.ledgerCanonical)
	writeSealPart(hasher, []byte(value.verified.authorizationDecision))
	writeSealInt64(hasher, value.verified.authorizationExpiresAtUnixMS)
	writeSealPart(hasher, []byte(value.verified.proposalBindingSHA256))
	writeSealPart(hasher, []byte(value.verified.receiptSHA256))
	writeSealPart(hasher, []byte(value.trust.PinnedTrustRootSHA256))
	writeSealInt64(hasher, value.trust.PinnedTrustEpoch)
	writeSealInt64(hasher, value.trust.ObservedAtUnixMS)
	writeSealInt64(hasher, value.trust.RevocationHighWaterSequence)
	writeSealPart(hasher, []byte(value.trust.RevocationHighWaterSHA256))
	var result [32]byte
	copy(result[:], hasher.Sum(nil))
	return result
}

func writeSealPart(hasher hash.Hash, value []byte) {
	var length [8]byte
	binary.BigEndian.PutUint64(length[:], uint64(len(value)))
	_, _ = hasher.Write(length[:])
	_, _ = hasher.Write(value)
}

func writeSealInt64(hasher hash.Hash, value int64) {
	var encoded [8]byte
	binary.BigEndian.PutUint64(encoded[:], uint64(value))
	_, _ = hasher.Write(encoded[:])
}
