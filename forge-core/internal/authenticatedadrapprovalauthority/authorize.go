package authenticatedadrapprovalauthority

import (
	"bytes"
	"errors"
	"fmt"

	contract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

var productionDependencies = dependencies{
	checkPlatform:   checkStatePlatform,
	readTrustRoot:   readProtectedTrustRoot,
	openState:       openProtectedState,
	preflightOutput: preflightStoredOutputShape,
	validateOutput:  validateProspectiveOutput,
}

type preparedAuthorization struct {
	encoded contract.EncodedAuthorizationInput
	rootRaw []byte
}

// AuthorizeAndStore authenticates one ADR-0079 input and returns an opaque
// stored authorization only after an exact signed complete ledger has been
// durably published and reopened, or after locked exact replay against current
// state. It performs no ADR lifecycle transition or repository write.
func AuthorizeAndStore(config Config, encoded contract.EncodedAuthorizationInput,
	trust ExternalTrust) (*StoredAuthorization, error) {
	return authorizeAndStoreWith(config, encoded, trust, productionDependencies)
}

func authorizeAndStoreWith(config Config, encoded contract.EncodedAuthorizationInput,
	trust ExternalTrust, deps dependencies) (*StoredAuthorization, error) {
	config = cloneConfig(config)
	encoded = cloneEncodedInput(encoded)
	if err := validateConfig(config); err != nil {
		return nil, coded(codeInvalidConfig, err)
	}
	if err := validateExternalTrust(trust); err != nil {
		return nil, coded(codeTrustRootRejected, err)
	}
	if deps.checkPlatform == nil || deps.readTrustRoot == nil || deps.openState == nil ||
		deps.preflightOutput == nil || deps.validateOutput == nil {
		return nil, coded(codeInvalidConfig, fmt.Errorf("authority dependencies are incomplete"))
	}
	if err := deps.checkPlatform(); err != nil {
		return nil, coded(codeUnsupported, err)
	}
	prepared, err := prepareAuthorization(config, encoded, trust, deps)
	if err != nil {
		return nil, err
	}
	session, err := deps.openState(config)
	if err != nil {
		return nil, stateError(err)
	}
	result, operationErr := authorizeLocked(session, config, prepared, trust, deps)
	closeErr := session.close()
	if operationErr != nil {
		return nil, operationErr
	}
	if closeErr != nil {
		return nil, coded(codePersistenceUncertain, closeErr)
	}
	return result, nil
}

func prepareAuthorization(config Config, encoded contract.EncodedAuthorizationInput,
	trust ExternalTrust, deps dependencies) (preparedAuthorization, error) {
	var prepared preparedAuthorization
	raw, err := deps.readTrustRoot(config)
	if err != nil {
		return prepared, coded(codeTrustRootRejected, err)
	}
	root, err := decodeAndAuthenticateRoot(raw, trust)
	if err != nil {
		clearBytes(raw)
		return prepared, err
	}
	input, _, err := authenticateInput(encoded, root, trust)
	if err != nil {
		clearBytes(raw)
		return prepared, err
	}
	if err = rejectExcludedProposal(input, config); err != nil {
		clearBytes(raw)
		return prepared, err
	}
	view, viewErr := parseInputView(encoded.Policy, encoded.Request,
		encoded.RevocationSnapshots)
	if viewErr != nil {
		clearBytes(raw)
		return prepared, coded(codeInputRejected, viewErr)
	}
	if err = preflightInputCurrent(input, view, root, trust); err != nil {
		clearBytes(raw)
		return prepared, err
	}
	prepared = preparedAuthorization{encoded: encoded, rootRaw: cloneBytes(raw)}
	clearBytes(raw)
	return prepared, nil
}

func authorizeLocked(session stateSession, config Config,
	prepared preparedAuthorization, trust ExternalTrust,
	deps dependencies) (*StoredAuthorization, error) {
	defer clearBytes(prepared.rootRaw)
	root, err := loadTrustedRoot(session, config, prepared.rootRaw, trust)
	if err != nil {
		return nil, err
	}
	input, view, err := authenticateInput(prepared.encoded, root, trust)
	if err != nil {
		return nil, err
	}
	snapshot, err := session.current()
	if err != nil {
		return nil, stateError(err)
	}
	prior, priorView, err := authenticateStored(snapshot, root, trust)
	if err != nil {
		return nil, err
	}
	if replay, matched, replayErr := replayIfExact(prior, priorView,
		prepared.encoded, view, snapshot.Data, trust); matched {
		return replay, replayErr
	}
	return authorizeNew(session, config, prepared.encoded, input, view, snapshot, prior,
		priorView, root, trust, deps)
}

func loadTrustedRoot(session stateSession, config Config, expected []byte,
	trust ExternalTrust) (*contract.TrustRoot, error) {
	raw, err := session.readLeaf(config.TrustRootPath, maxTrustRootBytes, privateMode)
	if err != nil {
		return nil, coded(codeTrustRootRejected, err)
	}
	if !exactBytes(raw, expected) {
		clearBytes(raw)
		return nil, coded(codeTrustRootRejected,
			fmt.Errorf("trust root changed between preflight and locked binding"))
	}
	root, err := contract.DecodeCanonicalTrustRoot(raw)
	clearBytes(raw)
	if err != nil {
		return nil, coded(codeTrustRootRejected, err)
	}
	if err = authenticateRoot(root, trust); err != nil {
		return nil, err
	}
	return root, nil
}

func decodeAndAuthenticateRoot(raw []byte,
	trust ExternalTrust) (*contract.TrustRoot, error) {
	root, err := contract.DecodeCanonicalTrustRoot(raw)
	if err != nil {
		return nil, coded(codeTrustRootRejected, err)
	}
	if err = authenticateRoot(root, trust); err != nil {
		return nil, err
	}
	return root, nil
}

func authenticateInput(encoded contract.EncodedAuthorizationInput, root *contract.TrustRoot,
	trust ExternalTrust) (*contract.AuthorizationInput, inputView, error) {
	var view inputView
	input, err := contract.DecodeAuthorizationInput(encoded, root)
	if err != nil {
		return nil, view, coded(codeInputRejected, err)
	}
	checks, err := contract.SignatureChecks(input)
	if err != nil {
		return nil, view, coded(codeInputRejected, err)
	}
	if err = verifySignatureChecks(checks); err != nil {
		return nil, view, coded(codeSignatureRejected, err)
	}
	view, err = parseInputView(encoded.Policy, encoded.Request, encoded.RevocationSnapshots)
	if err != nil {
		return nil, view, coded(codeInputRejected, err)
	}
	if err = requireExternalHighWater(view.Latest, trust); err != nil {
		return nil, view, err
	}
	if err = requireCurrentWindow(view.Latest.EffectiveAtUnixMS,
		view.Latest.ExpiresAtUnixMS, trust.ObservedAtUnixMS); err != nil {
		return nil, view, coded(codeRevocationRejected, err)
	}
	return input, view, nil
}

func authenticateStored(snapshot stateSnapshot, root *contract.TrustRoot,
	trust ExternalTrust) (*contract.Ledger, ledgerView, error) {
	var view ledgerView
	if !snapshot.Present {
		return nil, view, nil
	}
	ledger, err := contract.DecodeCanonicalLedger(snapshot.Data, root)
	if err != nil {
		return nil, view, coded(codeLedgerRejected, err)
	}
	checks, err := contract.SignatureChecks(ledger)
	if err != nil || verifySignatureChecks(checks) != nil {
		return nil, view, coded(codeSignatureRejected, fmt.Errorf("stored ledger signature rejected"))
	}
	if err = decodeJSONView(snapshot.Data, &view, "stored ledger"); err != nil {
		return nil, view, coded(codeLedgerRejected, err)
	}
	if view.ClockHighWaterUnixMS > trust.ObservedAtUnixMS {
		return nil, view, coded(codeTimeRejected, fmt.Errorf("trusted time regresses below stored clock"))
	}
	return ledger, view, nil
}

func stateError(err error) error {
	switch {
	case errors.Is(err, errStateBusy):
		return coded(codeStateBusy, err)
	case errors.Is(err, errStateConflict):
		return coded(codeCASConflict, err)
	case errors.Is(err, errStateUncertain):
		return coded(codePersistenceUncertain, err)
	case errors.Is(err, errUnsupported):
		return coded(codeUnsupported, err)
	default:
		return coded(codeStateRejected, err)
	}
}

func exactBytes(left, right []byte) bool {
	return len(left) == len(right) && bytes.Equal(left, right)
}

func cloneEncodedInput(value contract.EncodedAuthorizationInput) contract.EncodedAuthorizationInput {
	result := contract.EncodedAuthorizationInput{
		ProposalDocument: cloneBytes(value.ProposalDocument), Policy: cloneBytes(value.Policy),
		Request: cloneBytes(value.Request), RevocationSnapshots: make([][]byte,
			len(value.RevocationSnapshots))}
	for index, snapshot := range value.RevocationSnapshots {
		result.RevocationSnapshots[index] = cloneBytes(snapshot)
	}
	return result
}

func cloneConfig(value Config) Config {
	result := value
	result.ExtraExcludedProposalBindingSHA256s = append([]string(nil),
		value.ExtraExcludedProposalBindingSHA256s...)
	return result
}
