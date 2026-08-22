package authenticatedadrapprovalauthority

import (
	"bytes"
	"fmt"

	contract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

var excludedBootstrapADRIDs = map[string]bool{
	"ADR-0079": true,
	"ADR-0080": true,
	"ADR-0081": true,
}

var excludedBootstrapProposalBindings = map[string]bool{
	"d90111614eff92e843c246e56ff2d71cc453d54c91426b8d013a7702f88512c9": true,
	"0cdd000081f6098ac9a9138521893b1cf0b31a68fe98e5b7c0a7d2b133c13ed4": true,
	"fac7e74c96c41b09547d9fef6210b7a0bb7f5b2333dcffb20f66efcd0dd3fa1c": true,
}

func rejectExcludedProposal(input *contract.AuthorizationInput, config Config) error {
	facts, err := contract.Facts(input)
	if err != nil {
		return coded(codeInputRejected, err)
	}
	return rejectProposalIdentity(facts.ADRID, facts.ProposalBindingSHA256,
		config.ExtraExcludedProposalBindingSHA256s)
}

func rejectProposalIdentity(adrID, binding string, extra []string) error {
	if excludedBootstrapADRIDs[adrID] || excludedBootstrapProposalBindings[binding] {
		return coded(codeProposalExcluded, fmt.Errorf("bootstrap ADR is excluded"))
	}
	for _, digest := range extra {
		if constantTimeTextEqual(digest, binding) {
			return coded(codeProposalExcluded, fmt.Errorf("proposal binding is excluded"))
		}
	}
	return nil
}

func preflightNew(encoded contract.EncodedAuthorizationInput, input *contract.AuthorizationInput,
	view inputView, prior *contract.Ledger, priorView ledgerView,
	trust ExternalTrust) (*contract.ReceiptDraft, []byte, error) {
	if view.Request.RevocationSequence != view.Latest.Sequence ||
		!constantTimeTextEqual(view.Request.RevocationSHA256, view.Latest.SHA256) {
		return nil, nil, coded(codeRevocationRejected,
			fmt.Errorf("new request does not bind external revocation high-water"))
	}
	if err := requireCAS(view.Request, priorView, prior != nil); err != nil {
		return nil, nil, err
	}
	if err := requireCapacityAndPrefix(encoded, priorView, prior != nil); err != nil {
		return nil, nil, err
	}
	if err := requireProposalAvailable(input, view, prior); err != nil {
		return nil, nil, err
	}
	priorReceipt := priorReceiptSHA(priorView, prior != nil)
	draft, message, err := contract.NewReceiptDraft(input, trust.ObservedAtUnixMS, priorReceipt)
	if err != nil {
		return nil, nil, coded(codeAuthorizationNotCurrent, err)
	}
	return draft, message, nil
}

func preflightInputCurrent(input *contract.AuthorizationInput, view inputView,
	root *contract.TrustRoot, trust ExternalTrust) error {
	if err := requireCurrentWindow(view.Policy.Validity.NotBeforeUnixMS,
		view.Policy.Validity.ExpiresAtUnixMS, trust.ObservedAtUnixMS); err != nil {
		return coded(codeTimeRejected, err)
	}
	if err := requireCurrentWindow(view.Request.RequestedAtUnixMS,
		view.Request.ExpiresAtUnixMS, trust.ObservedAtUnixMS); err != nil {
		return coded(codeTimeRejected, err)
	}
	if err := requireStateKeyCurrent(root, view.Latest); err != nil {
		return err
	}
	if err := requireInputAuthoritiesCurrent(view); err != nil {
		return err
	}
	prior := provisionalPriorReceipt(view.Request.ExpectedNextSequence)
	if _, _, err := contract.NewReceiptDraft(input, trust.ObservedAtUnixMS, prior); err != nil {
		return coded(codeAuthorizationNotCurrent, err)
	}
	return nil
}

func requireInputAuthoritiesCurrent(view inputView) error {
	revokedApprovals := stringLookup(view.Latest.RevokedApprovalIDs)
	revokedKeys := stringLookup(view.Latest.RevokedKeyIDs)
	if revokedKeys[view.Policy.Signature.KeyID] || revokedKeys[view.Request.Signature.KeyID] {
		return coded(codeAuthorizationNotCurrent,
			fmt.Errorf("policy or request signing key is revoked"))
	}
	for _, approval := range view.Request.ApprovalRecords {
		if revokedApprovals[approval.ApprovalID] ||
			revokedKeys[approval.AuthorityProof.KeyID] {
			return coded(codeAuthorizationNotCurrent,
				fmt.Errorf("approval or its key is revoked"))
		}
	}
	return nil
}

func provisionalPriorReceipt(sequence int64) *string {
	if sequence == 1 {
		return nil
	}
	value := "0000000000000000000000000000000000000000000000000000000000000000"
	return &value
}

func requireStateKeyCurrent(root *contract.TrustRoot, latest revocationView) error {
	key, err := stateSigningKey(root)
	if err != nil {
		return coded(codeSignerKeyRejected, err)
	}
	if stringLookup(latest.RevokedKeyIDs)[key.KeyID] {
		return coded(codeAuthorizationNotCurrent, fmt.Errorf("state signing key is revoked"))
	}
	return nil
}

func requireCAS(request requestView, prior ledgerView, present bool) error {
	if !present {
		if request.ExpectedNextSequence != 1 || request.ExpectedLedgerSHA256 != nil {
			return coded(codeCASConflict, fmt.Errorf("genesis CAS position differs"))
		}
		return nil
	}
	expectedSequence := int64(len(prior.Entries) + 1)
	if request.ExpectedNextSequence != expectedSequence || request.ExpectedLedgerSHA256 == nil ||
		!constantTimeTextEqual(*request.ExpectedLedgerSHA256, prior.LedgerSHA256) {
		return coded(codeCASConflict, fmt.Errorf("request CAS position differs"))
	}
	return nil
}

func requireCapacityAndPrefix(encoded contract.EncodedAuthorizationInput,
	prior ledgerView, present bool) error {
	if present && len(prior.Entries) >= 64 {
		return coded(codeCapacityExhausted, fmt.Errorf("ledger entry capacity reached"))
	}
	if len(encoded.RevocationSnapshots) > 256 {
		return coded(codeCapacityExhausted, fmt.Errorf("revocation capacity reached"))
	}
	if !present {
		return nil
	}
	if len(prior.RevocationSnapshots) > len(encoded.RevocationSnapshots) {
		return coded(codeRevocationRejected, fmt.Errorf("revocation chain regressed"))
	}
	for index, snapshot := range prior.RevocationSnapshots {
		if !bytes.Equal(snapshot, encoded.RevocationSnapshots[index]) {
			return coded(codeRevocationRejected, fmt.Errorf("revocation prefix differs"))
		}
	}
	return nil
}

func requireProposalAvailable(input *contract.AuthorizationInput, view inputView,
	prior *contract.Ledger) error {
	if prior == nil || declaredDecision(view) != "acceptance_transition_authorized" {
		return nil
	}
	inputFacts, err := contract.Facts(input)
	if err != nil {
		return coded(codeInputRejected, err)
	}
	priorFacts, err := contract.Facts(prior)
	if err != nil {
		return coded(codeLedgerRejected, err)
	}
	for _, record := range priorFacts.ReplayRecords {
		if record.AuthorizationDecision == "acceptance_transition_authorized" &&
			constantTimeTextEqual(record.ProposalBindingSHA256,
				inputFacts.ProposalBindingSHA256) {
			return coded(codeProposalAlreadyAllowed, fmt.Errorf("proposal already authorized"))
		}
	}
	return nil
}

func declaredDecision(view inputView) string {
	approvals, rejected := int64(0), false
	for _, record := range view.Request.ApprovalRecords {
		if record.Decision == "approve" {
			approvals++
		}
		if record.Decision == "reject" {
			rejected = true
		}
	}
	if view.Policy.Disposition == "deny" || rejected || approvals < view.Policy.Threshold {
		return "acceptance_transition_not_authorized"
	}
	return "acceptance_transition_authorized"
}

func priorReceiptSHA(view ledgerView, present bool) *string {
	if !present || len(view.Entries) == 0 {
		return nil
	}
	value := view.Entries[len(view.Entries)-1].Receipt.ReceiptSHA256
	return &value
}
