package authenticatedadrlifecycleauthority

import (
	"errors"
	"fmt"

	approvalauthority "forgeos/forge-core/internal/authenticatedadrapprovalauthority"
)

type dependencies struct {
	checkPlatform   func() error
	readMaterials   func(Config) ([][]byte, error)
	openState       func(Config) (stateSession, error)
	openReplayState func(Config) (stateSession, error)
}

var productionDependencies = dependencies{checkPlatform: checkStatePlatform,
	readMaterials: readProtectedMaterials, openState: openProtectedState,
	openReplayState: openProtectedReplayState}

// TransitionAndStore consumes an opaque StoredAuthorization for a fresh
// transition. An exact historical match bypasses current capability and returns
// exact_replay; fresh output follows durable publication and authenticated reopen.
func TransitionAndStore(config Config, encoded EncodedTransitionInput,
	stored *approvalauthority.StoredAuthorization,
	trust ExternalTrust) (*StoredTransition, error) {
	return transitionAndStoreWith(config, encoded, stored, trust, productionDependencies)
}

// ReplayStored returns one exact historical transition without signing,
// publishing, or authority application writes. It does not require a current
// approval capability; ordinary filesystem reads may retain host atime semantics.
func ReplayStored(config Config, encoded EncodedTransitionInput,
	trust ExternalTrust) (*StoredTransition, error) {
	return replayStoredWith(config, encoded, trust, productionDependencies)
}

func transitionAndStoreWith(config Config, encoded EncodedTransitionInput,
	stored *approvalauthority.StoredAuthorization, trust ExternalTrust,
	deps dependencies) (*StoredTransition, error) {
	config = cloneConfig(config)
	encoded.RequestJSON = cloneBytes(encoded.RequestJSON)
	material, session, snapshot, prior, replayInput, err := beginOperation(config, encoded,
		trust, deps, deps.openState)
	if err != nil {
		return nil, err
	}
	defer clearMaterial(&material)
	var result *StoredTransition
	if replay, matched, replayErr := replayExact(prior, replayInput, material); matched {
		result, err = replay, replayErr
	} else {
		result, err = transitionFresh(session, config, encoded, stored, trust,
			material, snapshot, prior)
	}
	closeErr := session.close()
	if err != nil {
		return nil, err
	}
	if closeErr != nil {
		return nil, coded(codePersistenceUncertain, closeErr)
	}
	return result, nil
}

func transitionFresh(session stateSession, config Config, encoded EncodedTransitionInput,
	stored *approvalauthority.StoredAuthorization, trust ExternalTrust,
	material authorityMaterial, snapshot stateSnapshot,
	prior *authenticatedState) (*StoredTransition, error) {
	input, err := prepareFreshInput(encoded, stored, material, trust, config)
	if err != nil {
		return nil, err
	}
	placeholder, err := buildProspective(input, prior, material, placeholderWireSigner{}, false)
	if err != nil {
		return nil, classifyBuildError(err)
	}
	if len(placeholder.stateJSON) == 0 {
		return nil, coded(codeInputRejected, fmt.Errorf("preflight state is absent"))
	}
	signer, err := loadStateSigner(session, config, material)
	if err != nil {
		return nil, err
	}
	defer signer.close()
	prospective, err := buildProspective(input, prior, material, realWireSigner{signer: signer}, true)
	if err != nil {
		return nil, classifyBuildError(err)
	}
	if err = session.commit(snapshot, prospective.stateJSON); err != nil {
		return nil, commitError(err)
	}
	return reopenStored(session, prospective, material, trust)
}

func replayStoredWith(config Config, encoded EncodedTransitionInput,
	trust ExternalTrust, deps dependencies) (*StoredTransition, error) {
	config = cloneConfig(config)
	encoded.RequestJSON = cloneBytes(encoded.RequestJSON)
	material, session, _, prior, input, err := beginOperation(config, encoded,
		trust, deps, deps.openReplayState)
	if err != nil {
		return nil, err
	}
	defer clearMaterial(&material)
	replay, matched, err := replayExact(prior, input, material)
	if !matched && err == nil {
		err = coded(codeInputRejected, fmt.Errorf("exact historical request is absent"))
	}
	closeErr := session.close()
	if err != nil {
		return nil, err
	}
	if closeErr != nil {
		return nil, coded(codeStateRejected, closeErr)
	}
	return replay, nil
}

func beginOperation(config Config, encoded EncodedTransitionInput, trust ExternalTrust,
	deps dependencies, openState func(Config) (stateSession, error)) (authorityMaterial, stateSession, stateSnapshot,
	*authenticatedState, preparedInput, error) {
	material, replayInput, err := preflightOperation(config, encoded, trust, deps)
	if err != nil {
		return authorityMaterial{}, nil, stateSnapshot{}, nil, preparedInput{}, err
	}
	if openState == nil {
		clearMaterial(&material)
		return authorityMaterial{}, nil, stateSnapshot{}, nil, preparedInput{},
			coded(codeInvalidConfig, fmt.Errorf("state opener is absent"))
	}
	session, err := openState(config)
	if err != nil {
		clearMaterial(&material)
		return authorityMaterial{}, nil, stateSnapshot{}, nil, preparedInput{}, stateError(err)
	}
	if err = bindLockedMaterials(session, config, material); err != nil {
		_ = session.close()
		clearMaterial(&material)
		return authorityMaterial{}, nil, stateSnapshot{}, nil, preparedInput{}, err
	}
	snapshot, err := session.current()
	if err != nil {
		_ = session.close()
		clearMaterial(&material)
		return authorityMaterial{}, nil, stateSnapshot{}, nil, preparedInput{}, stateError(err)
	}
	prior, err := authenticateState(snapshot, material, trust)
	if err != nil {
		_ = session.close()
		clearMaterial(&material)
		return authorityMaterial{}, nil, stateSnapshot{}, nil, preparedInput{}, err
	}
	return material, session, snapshot, prior, replayInput, nil
}

func preflightOperation(config Config, encoded EncodedTransitionInput,
	trust ExternalTrust, deps dependencies) (authorityMaterial, preparedInput, error) {
	var material authorityMaterial
	if err := validateConfig(config); err != nil {
		return material, preparedInput{}, coded(codeInvalidConfig, err)
	}
	if err := validateExternalTrust(trust); err != nil {
		return material, preparedInput{}, coded(codeTrustRootRejected, err)
	}
	if deps.checkPlatform == nil || deps.readMaterials == nil {
		return material, preparedInput{}, coded(codeInvalidConfig, fmt.Errorf("dependencies incomplete"))
	}
	if err := deps.checkPlatform(); err != nil {
		return material, preparedInput{}, coded(codeUnsupported, err)
	}
	raw, err := deps.readMaterials(config)
	if err != nil {
		return material, preparedInput{}, coded(codeTrustRootRejected, err)
	}
	defer clearMatrix(raw)
	if len(raw) != 3 {
		return material, preparedInput{}, coded(codeTrustRootRejected, fmt.Errorf("authority material incomplete"))
	}
	material, err = decodeAuthorityMaterial(raw[0], raw[1], raw[2], trust)
	if err != nil {
		return authorityMaterial{}, preparedInput{}, err
	}
	replayInput, err := prepareReplayInput(encoded, material, trust)
	if err != nil {
		clearMaterial(&material)
		return authorityMaterial{}, preparedInput{}, err
	}
	return material, replayInput, nil
}

func bindLockedMaterials(session stateSession, config Config,
	material authorityMaterial) error {
	paths := []string{config.SignatureProfilePath, config.ApprovalTrustRootPath,
		config.LifecycleTrustRootPath}
	maxima := []int64{maxProfile, maxRoot, maxRoot}
	expected := [][]byte{material.profileRaw, material.approvalRaw, material.lifecycleRaw}
	for index, path := range paths {
		raw, err := session.readLeaf(path, maxima[index], privateMode)
		if err != nil || !exactBytes(raw, expected[index]) {
			clearBytes(raw)
			return coded(codeTrustRootRejected, fmt.Errorf("authority material changed across lock"))
		}
		clearBytes(raw)
	}
	return nil
}

func loadStateSigner(session stateSession, config Config,
	material authorityMaterial) (*stateSigner, error) {
	seed, err := session.readLeaf(config.StateSignerSeedPath, seedBytes, privateMode)
	if err != nil {
		return nil, coded(codeSignerKeyRejected, err)
	}
	defer clearBytes(seed)
	signer, err := newStateSigner(seed, material.stateKey)
	if err != nil {
		return nil, coded(codeSignerKeyRejected, err)
	}
	return signer, nil
}

func replayExact(prior *authenticatedState, input preparedInput,
	material authorityMaterial) (*StoredTransition, bool, error) {
	if prior == nil {
		return nil, false, nil
	}
	entries, err := arrayField(prior.ledger, "entries")
	if err != nil {
		return nil, true, coded(codeStateRejected, err)
	}
	match, err := findReplayEntry(entries, input.idempotency)
	if err != nil {
		return nil, true, coded(codeStateRejected, err)
	}
	if match == nil {
		return nil, false, nil
	}
	requestJSON, err := canonicalJSON(match["request"], maxRequest, "stored request")
	if err != nil || !exactBytes(requestJSON, input.raw) {
		return nil, true, coded(codeIdempotencyConflict, fmt.Errorf("idempotency key reuses different request"))
	}
	sequence, err := intField(match, "sequence")
	if err != nil {
		return nil, true, coded(codeStateRejected, err)
	}
	result, _, err := resultForState(prior.state, sequence, "exact_replay")
	if err != nil {
		return nil, true, coded(codeStateRejected, err)
	}
	node := bundleNode(material, prior.state, result)
	_, stateJSON, resultJSON, err := validateBundleNode(node, material, "exact_replay")
	if err != nil || !exactBytes(stateJSON, prior.canonical) {
		return nil, true, coded(codeStateRejected, fmt.Errorf("exact replay state authentication failed: %w", err))
	}
	stored, err := newStoredTransition(resultJSON, stateJSON, sequence, "exact_replay")
	return stored, true, err
}

func findReplayEntry(entries []any, idempotency string) (map[string]any, error) {
	var match map[string]any
	for _, raw := range entries {
		entry, err := objectValue(raw, "entry")
		if err != nil {
			return nil, err
		}
		request, err := objectField(entry, "request")
		if err != nil {
			return nil, err
		}
		key, err := stringField(request, "idempotency_key")
		if err != nil {
			return nil, err
		}
		if key != idempotency {
			continue
		}
		if match != nil {
			return nil, fmt.Errorf("state repeats idempotency key")
		}
		match = entry
	}
	return match, nil
}

func reopenStored(session stateSession, prospective prospectiveState,
	material authorityMaterial, trust ExternalTrust) (*StoredTransition, error) {
	reopened, err := session.current()
	if err != nil || !reopened.Present || !exactBytes(reopened.Data, prospective.stateJSON) {
		return nil, coded(codePersistenceUncertain, fmt.Errorf("published state did not strictly reopen"))
	}
	authenticated, err := authenticateState(reopened, material, trust)
	if err != nil || !exactBytes(authenticated.canonical, prospective.stateJSON) {
		return nil, coded(codePersistenceUncertain, fmt.Errorf("published state failed authentication: %w", err))
	}
	result, _, err := resultForState(authenticated.state, prospective.sequence, "stored")
	if err != nil {
		return nil, coded(codePersistenceUncertain, err)
	}
	node := bundleNode(material, authenticated.state, result)
	_, stateJSON, resultJSON, err := validateBundleNode(node, material, "stored")
	if err != nil || !exactBytes(stateJSON, prospective.stateJSON) || !exactBytes(resultJSON, prospective.resultJSON) {
		return nil, coded(codePersistenceUncertain, fmt.Errorf("reopened result differs: %w", err))
	}
	stored, err := newStoredTransition(resultJSON, stateJSON, prospective.sequence, "stored")
	if err != nil {
		return nil, coded(codePersistenceUncertain, err)
	}
	return stored, nil
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

func commitError(err error) error {
	if errors.Is(err, errStateConflict) {
		return coded(codeCASConflict, err)
	}
	if errors.Is(err, errStateUncertain) {
		return coded(codePersistenceUncertain, err)
	}
	return coded(codePersistenceFailed, err)
}

func classifyBuildError(err error) error {
	var authority *authorityError
	if errors.As(err, &authority) {
		return err
	}
	return coded(codeInputRejected, err)
}

func clearMaterial(value *authorityMaterial) {
	if value == nil {
		return
	}
	clearBytes(value.profileRaw)
	clearBytes(value.approvalRaw)
	clearBytes(value.lifecycleRaw)
	value.profileRaw, value.approvalRaw, value.lifecycleRaw = nil, nil, nil
}
