package authenticatedadrapprovalauthority

import (
	"fmt"

	contract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

// VerifyBundle authenticates an exact canonical bundle relative to explicit
// caller trust. It verifies no persistence property and performs no ADR change.
func VerifyBundle(canonical []byte, trust ExternalTrust) (*VerifiedBundle, error) {
	canonical = cloneBytes(canonical)
	bundle, view, root, err := decodeBundleForVerification(canonical)
	if err != nil {
		return nil, coded(codeInputRejected, err)
	}
	if err = validateExternalTrust(trust); err != nil {
		return nil, coded(codeTrustRootRejected, err)
	}
	if err = authenticateRoot(root, trust); err != nil {
		return nil, err
	}
	checks, err := contract.SignatureChecks(bundle)
	if err != nil {
		return nil, coded(codeInputRejected, err)
	}
	if err = verifySignatureChecks(checks); err != nil {
		return nil, coded(codeSignatureRejected, err)
	}
	facts, err := contract.Facts(bundle)
	if err != nil {
		return nil, coded(codeInputRejected, err)
	}
	if err = rejectProposalIdentity(facts.ADRID, facts.ProposalBindingSHA256,
		nil); err != nil {
		return nil, err
	}
	if err = verifyBundleCurrent(view, trust); err != nil {
		return nil, err
	}
	return newVerifiedBundle(canonical, view.Receipt), nil
}

func decodeBundleForVerification(canonical []byte) (*contract.Bundle, bundleView,
	*contract.TrustRoot, error) {
	var view bundleView
	bundle, err := contract.DecodeCanonicalBundle(canonical)
	if err != nil {
		return nil, view, nil, err
	}
	if err = decodeJSONView(canonical, &view, "bundle"); err != nil {
		return nil, view, nil, err
	}
	root, err := contract.DecodeCanonicalTrustRoot(view.Root)
	if err != nil {
		return nil, view, nil, err
	}
	return bundle, view, root, nil
}

func authenticateRoot(root *contract.TrustRoot, trust ExternalTrust) error {
	rootSHA, epoch := root.Identity()
	if !constantTimeTextEqual(rootSHA, trust.PinnedTrustRootSHA256) ||
		epoch != trust.PinnedTrustEpoch {
		return coded(codeTrustRootRejected, fmt.Errorf("root pin or epoch differs"))
	}
	facts, err := contract.Facts(root)
	if err != nil {
		return coded(codeTrustRootRejected, err)
	}
	if rootSHA == knownFixtureRootSHA256 {
		return coded(codeFixtureAuthority, fmt.Errorf("known fixture root is forbidden"))
	}
	for _, key := range facts.RootKeys {
		if fixtureKeyIdentity(key) {
			return coded(codeFixtureAuthority, fmt.Errorf("fixture key identity is forbidden"))
		}
	}
	if fixtureNamespace(facts.TrustDomain) {
		return coded(codeFixtureAuthority, fmt.Errorf("fixture trust namespace is forbidden"))
	}
	return nil
}

func verifyBundleCurrent(view bundleView, trust ExternalTrust) error {
	latest, err := parseLatestRevocation(view.Ledger)
	if err != nil {
		return coded(codeRevocationRejected, err)
	}
	if err = requireExternalHighWater(latest, trust); err != nil {
		return err
	}
	if trust.ObservedAtUnixMS < view.Ledger.ClockHighWaterUnixMS {
		return coded(codeTimeRejected, fmt.Errorf("trusted time regresses below ledger clock"))
	}
	if err = requireCurrentWindow(latest.EffectiveAtUnixMS,
		latest.ExpiresAtUnixMS, trust.ObservedAtUnixMS); err != nil {
		return coded(codeRevocationRejected, err)
	}
	if view.Receipt.AuthorizationDecision != "acceptance_transition_authorized" {
		return nil
	}
	if trust.ObservedAtUnixMS >= view.Receipt.AuthorizationExpiresAtUnixMS {
		return coded(codeAuthorizationNotCurrent, fmt.Errorf("authorization expired"))
	}
	return requireActiveReceiptUnrevoked(view, latest)
}

func requireExternalHighWater(latest revocationView, trust ExternalTrust) error {
	if latest.Sequence != trust.RevocationHighWaterSequence ||
		!constantTimeTextEqual(latest.SHA256, trust.RevocationHighWaterSHA256) {
		return coded(codeRevocationRejected, fmt.Errorf("revocation high-water differs"))
	}
	return nil
}

func requireCurrentWindow(start, end, observed int64) error {
	if observed < start || observed >= end {
		return fmt.Errorf("trusted observation lies outside half-open validity window")
	}
	return nil
}

func requireActiveReceiptUnrevoked(view bundleView, latest revocationView) error {
	revokedApprovals := stringLookup(latest.RevokedApprovalIDs)
	for _, approvalID := range view.Receipt.QualifyingApprovalIDs {
		if revokedApprovals[approvalID] {
			return coded(codeAuthorizationNotCurrent, fmt.Errorf("qualifying approval is revoked"))
		}
	}
	revokedKeys := stringLookup(latest.RevokedKeyIDs)
	for _, keyID := range activeReceiptKeyIDs(view) {
		if revokedKeys[keyID] {
			return coded(codeAuthorizationNotCurrent, fmt.Errorf("active authorization key is revoked"))
		}
	}
	return nil
}

func activeReceiptKeyIDs(view bundleView) []string {
	result := []string{view.Policy.Signature.KeyID, view.Request.Signature.KeyID,
		view.Receipt.Signature.KeyID, view.Ledger.Signature.KeyID}
	qualifying := stringLookup(view.Receipt.QualifyingApprovalIDs)
	for _, approval := range view.Request.ApprovalRecords {
		if qualifying[approval.ApprovalID] {
			result = append(result, approval.AuthorityProof.KeyID)
		}
	}
	return result
}

func stringLookup(values []string) map[string]bool {
	result := make(map[string]bool, len(values))
	for _, value := range values {
		result[value] = true
	}
	return result
}

func newVerifiedBundle(canonical []byte, receipt receiptView) *VerifiedBundle {
	return &VerifiedBundle{canonical: cloneBytes(canonical),
		authorizationDecision:        receipt.AuthorizationDecision,
		authorizationExpiresAtUnixMS: receipt.AuthorizationExpiresAtUnixMS,
		proposalBindingSHA256:        receipt.ProposalBindingSHA256,
		receiptSHA256:                receipt.ReceiptSHA256}
}
