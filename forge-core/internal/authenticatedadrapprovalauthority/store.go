package authenticatedadrapprovalauthority

import (
	"bytes"
	"encoding/base64"
	"errors"
	"fmt"

	contract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

func authorizeNew(session stateSession, config Config,
	encoded contract.EncodedAuthorizationInput, input *contract.AuthorizationInput,
	view inputView, snapshot stateSnapshot, prior *contract.Ledger,
	priorView ledgerView, root *contract.TrustRoot,
	trust ExternalTrust, deps dependencies) (*StoredAuthorization, error) {
	receiptDraft, receiptMessage, err := preflightNew(encoded, input, view, prior,
		priorView, trust)
	if err != nil {
		return nil, err
	}
	if err = deps.preflightOutput(input, receiptDraft, prior,
		trust.ObservedAtUnixMS); err != nil {
		return nil, err
	}
	signer, err := loadStateSigner(session, config, root)
	if err != nil {
		return nil, err
	}
	defer signer.close()
	receiptSignature, err := signer.sign(receiptMessage)
	if err != nil {
		return nil, coded(codeSigningRejected, err)
	}
	receipt, err := contract.SealReceipt(receiptDraft, receiptSignature)
	if err != nil {
		return nil, coded(codeSigningRejected, err)
	}
	ledger, canonical, err := buildSignedLedger(input, receipt, prior,
		trust.ObservedAtUnixMS, signer)
	if err != nil {
		return nil, err
	}
	prospective, err := deps.validateOutput(input, receipt, ledger, trust)
	if err != nil {
		return nil, err
	}
	if err = session.commit(snapshot, canonical); err != nil {
		return nil, commitError(err)
	}
	return reopenAndVerifyStored(session, input, receipt, ledger, canonical,
		prospective, root, trust)
}

func preflightStoredOutputShape(input *contract.AuthorizationInput,
	receiptDraft *contract.ReceiptDraft, prior *contract.Ledger, clock int64) error {
	placeholder := base64.RawURLEncoding.EncodeToString(make([]byte, 64))
	receipt, err := contract.SealReceipt(receiptDraft, placeholder)
	if err != nil {
		return coded(codeCapacityExhausted, err)
	}
	ledgerDraft, _, err := contract.NewLedgerDraft(input, receipt, prior, clock)
	if err != nil {
		return coded(codeCapacityExhausted, err)
	}
	ledger, err := contract.SealLedger(ledgerDraft, placeholder)
	if err != nil {
		return coded(codeCapacityExhausted, err)
	}
	if _, err = contract.CanonicalLedgerJSON(ledger); err != nil {
		return coded(codeCapacityExhausted, err)
	}
	bundle, err := contract.StoredBundle(input, receipt, ledger)
	if err == nil {
		_, err = contract.CanonicalBundleJSON(bundle)
	}
	if err != nil {
		return coded(codeCapacityExhausted, err)
	}
	return nil
}

func validateProspectiveOutput(input *contract.AuthorizationInput,
	receipt *contract.Receipt, ledger *contract.Ledger,
	trust ExternalTrust) ([]byte, error) {
	bundle, err := contract.StoredBundle(input, receipt, ledger)
	if err != nil {
		return nil, coded(codeLedgerRejected, err)
	}
	canonical, err := contract.CanonicalBundleJSON(bundle)
	if err != nil {
		return nil, coded(codeCapacityExhausted, err)
	}
	if _, err = VerifyBundle(canonical, trust); err != nil {
		return nil, err
	}
	return canonical, nil
}

func commitError(err error) error {
	if errors.Is(err, errStateConflict) {
		return coded(codeCASConflict, err)
	}
	if errors.Is(err, errStateUncertain) {
		return coded(codePersistenceUncertain, err)
	}
	return coded(codePersistenceFailed, err)
}

func loadStateSigner(session stateSession, config Config,
	root *contract.TrustRoot) (*stateSigner, error) {
	key, err := stateSigningKey(root)
	if err != nil {
		return nil, coded(codeSignerKeyRejected, err)
	}
	seed, err := session.readLeaf(config.StateSignerSeedPath, stateSeedBytes, privateMode)
	if err != nil {
		return nil, coded(codeSignerKeyRejected, err)
	}
	defer clearBytes(seed)
	signer, err := newStateSigner(seed, key)
	if err != nil {
		return nil, coded(codeSignerKeyRejected, err)
	}
	return signer, nil
}

func stateSigningKey(root *contract.TrustRoot) (contract.RootKey, error) {
	facts, err := contract.Facts(root)
	if err != nil {
		return contract.RootKey{}, err
	}
	var result contract.RootKey
	count := 0
	for _, key := range facts.RootKeys {
		if key.Usage == "approval_authorization_state_sign" {
			result, count = key, count+1
		}
	}
	if count != 1 {
		return contract.RootKey{}, fmt.Errorf("root has no unique state signing key")
	}
	return result, nil
}

func buildSignedLedger(input *contract.AuthorizationInput, receipt *contract.Receipt,
	prior *contract.Ledger, clock int64, signer *stateSigner) (*contract.Ledger, []byte, error) {
	draft, message, err := contract.NewLedgerDraft(input, receipt, prior, clock)
	if err != nil {
		return nil, nil, coded(codeLedgerRejected, err)
	}
	signature, err := signer.sign(message)
	if err != nil {
		return nil, nil, coded(codeSigningRejected, err)
	}
	ledger, err := contract.SealLedger(draft, signature)
	if err != nil {
		return nil, nil, coded(codeSigningRejected, err)
	}
	canonical, err := contract.CanonicalLedgerJSON(ledger)
	if err != nil {
		return nil, nil, coded(codeLedgerRejected, err)
	}
	return ledger, canonical, nil
}

func reopenAndVerifyStored(session stateSession, input *contract.AuthorizationInput,
	receipt *contract.Receipt, built *contract.Ledger, canonical []byte,
	prospective []byte, root *contract.TrustRoot,
	trust ExternalTrust) (*StoredAuthorization, error) {
	reopened, err := session.current()
	if err != nil || !reopened.Present || !bytes.Equal(reopened.Data, canonical) {
		return nil, coded(codePersistenceUncertain,
			fmt.Errorf("published ledger did not strictly reopen"))
	}
	ledger, _, err := authenticateStored(reopened, root, trust)
	if err != nil {
		return nil, coded(codePersistenceUncertain, err)
	}
	builtBytes, err := contract.CanonicalLedgerJSON(built)
	if err != nil || !bytes.Equal(builtBytes, reopened.Data) {
		return nil, coded(codePersistenceUncertain, fmt.Errorf("reopened ledger differs"))
	}
	bundle, err := contract.StoredBundle(input, receipt, ledger)
	if err != nil {
		return nil, coded(codePersistenceUncertain, err)
	}
	bundleBytes, err := contract.CanonicalBundleJSON(bundle)
	if err != nil {
		return nil, coded(codePersistenceUncertain, err)
	}
	if !bytes.Equal(bundleBytes, prospective) {
		return nil, coded(codePersistenceUncertain,
			fmt.Errorf("reopened delivery bundle differs from preflight"))
	}
	verified, err := VerifyBundle(bundleBytes, trust)
	if err != nil {
		return nil, coded(codePersistenceUncertain, err)
	}
	stored, err := newStoredAuthorization(verified, reopened.Data, trust)
	if err != nil {
		return nil, coded(codePersistenceUncertain, err)
	}
	return stored, nil
}
