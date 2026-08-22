package authenticatedadrapprovalauthority

import (
	"bytes"
	"fmt"

	contract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

func replayIfExact(prior *contract.Ledger, priorView ledgerView,
	encoded contract.EncodedAuthorizationInput, input inputView,
	ledgerCanonical []byte, trust ExternalTrust) (*StoredAuthorization, bool, error) {
	if prior == nil {
		return nil, false, nil
	}
	entry, found, err := replayEntry(priorView, input.Request.IdempotencyKey)
	if err != nil {
		return nil, true, coded(codeLedgerRejected, err)
	}
	if !found {
		return nil, false, nil
	}
	if !bytes.Equal(entry.Policy, encoded.Policy) ||
		!bytes.Equal(entry.Request, encoded.Request) {
		return nil, true, coded(codeIdempotencyConflict,
			fmt.Errorf("idempotency key reuses different policy or request bytes"))
	}
	proposal, err := decodeProposalView(entry.ProposalDocumentBase64URL)
	if err != nil || !bytes.Equal(proposal, encoded.ProposalDocument) {
		return nil, true, coded(codeIdempotencyConflict,
			fmt.Errorf("idempotency key reuses different proposal bytes"))
	}
	bundle, err := contract.ExactReplayBundle(prior, input.Request.IdempotencyKey)
	if err != nil {
		return nil, true, coded(codeLedgerRejected, err)
	}
	canonical, err := contract.CanonicalBundleJSON(bundle)
	if err != nil {
		return nil, true, coded(codeLedgerRejected, err)
	}
	verified, err := VerifyBundle(canonical, trust)
	if err != nil {
		return nil, true, err
	}
	stored, err := newStoredAuthorization(verified, ledgerCanonical, trust)
	if err != nil {
		return nil, true, coded(codeStateRejected, err)
	}
	return stored, true, nil
}

func replayEntry(ledger ledgerView, idempotencyKey string) (ledgerEntryView, bool, error) {
	var match ledgerEntryView
	found := false
	for _, entry := range ledger.Entries {
		var request requestView
		if err := decodeJSONView(entry.Request, &request, "ledger request"); err != nil {
			return match, false, err
		}
		if request.IdempotencyKey != idempotencyKey {
			continue
		}
		if found {
			return match, false, fmt.Errorf("ledger repeats an idempotency key")
		}
		match, found = entry, true
	}
	return match, found, nil
}
