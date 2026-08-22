package bootstrapreporeadexecution

import (
	"forgeos/forge-core/internal/bootstrapgrantauthority"
	"forgeos/forge-core/internal/bootstraprepoexecutionauthority"
)

type authorityInputs struct {
	execution *bootstraprepoexecutionauthority.Trust
	issuance  *bootstrapgrantauthority.Trust
	ledger    *bootstrapgrantauthority.Ledger
}

type executionInputs struct {
	manifest   *bootstraprepoexecutionauthority.Manifest
	policy     *bootstraprepoexecutionauthority.Policy
	invocation *bootstraprepoexecutionauthority.Invocation
}

type identityBytes struct {
	policy        []byte
	invocation    []byte
	invocationErr error
}

func loadAuthorities(config Config, session stateSession) (authorityInputs, error) {
	issuanceBytes, err := session.ReadLeaf(config.IssuanceTrustRootPath,
		maxIssuanceRootBytes, privateMode)
	if err != nil {
		return authorityInputs{}, coded(CodeIssuanceRootRejected, err)
	}
	issuance, err := bootstrapgrantauthority.DecodePinnedTrustRoot(
		issuanceBytes, config.PinnedIssuanceRootSHA256)
	clear(issuanceBytes)
	if err != nil {
		return authorityInputs{}, coded(CodeIssuanceRootRejected, err)
	}
	ledger, err := loadIssuanceLedger(config, session, issuance)
	if err != nil {
		return authorityInputs{}, err
	}
	executionBytes, err := session.ReadLeaf(config.ExecutionTrustRootPath,
		maxExecutionRootBytes, privateMode)
	if err != nil {
		return authorityInputs{}, coded(CodeExecutionRootRejected, err)
	}
	execution, err := bootstraprepoexecutionauthority.DecodePinnedTrustRoot(
		executionBytes, config.PinnedExecutionRootSHA256, issuance)
	clear(executionBytes)
	if err != nil {
		return authorityInputs{}, coded(CodeExecutionRootRejected, err)
	}
	return authorityInputs{execution: execution, issuance: issuance, ledger: ledger}, nil
}

func loadIssuanceLedger(config Config, session stateSession,
	trust *bootstrapgrantauthority.Trust) (*bootstrapgrantauthority.Ledger, error) {
	ledgerBytes, err := session.ReadLeaf(config.IssuanceLedgerPath,
		maxIssuanceLedger, privateMode)
	if err != nil {
		return nil, coded(CodeIssuanceLedgerRejected, err)
	}
	ledger, err := bootstrapgrantauthority.DecodeLedger(ledgerBytes, trust)
	clear(ledgerBytes)
	if err != nil || ledger == nil {
		return nil, coded(CodeIssuanceLedgerRejected, err)
	}
	return ledger, nil
}

func loadExecutionInputs(config Config, session stateSession,
	authorities authorityInputs) (executionInputs, error) {
	identity, err := readIdentityBytes(config, session)
	if err != nil {
		return executionInputs{}, err
	}
	defer identity.clear()
	return decodeExecutionInputs(config, session, authorities, identity)
}

func decodeExecutionInputs(config Config, session stateSession,
	authorities authorityInputs, identity identityBytes) (executionInputs, error) {
	manifestBytes, err := session.ReadLeaf(config.ManifestPath, maxManifestBytes, privateMode)
	if err != nil {
		return executionInputs{}, coded(CodeManifestRejected, err)
	}
	manifest, err := bootstraprepoexecutionauthority.DecodeManifest(manifestBytes)
	clear(manifestBytes)
	if err != nil {
		return executionInputs{}, coded(CodeManifestRejected, err)
	}
	policy, err := bootstraprepoexecutionauthority.DecodePolicy(
		identity.policy, authorities.execution, authorities.ledger, manifest)
	if err != nil {
		return executionInputs{}, coded(CodePolicyRejected, err)
	}
	inputs := executionInputs{manifest: manifest, policy: policy}
	if !policy.AllowsExecution() {
		return inputs, nil
	}
	if identity.invocationErr != nil {
		return executionInputs{}, coded(CodeInvocationRejected, identity.invocationErr)
	}
	invocation, err := bootstraprepoexecutionauthority.DecodeInvocation(
		identity.invocation, authorities.execution, manifest, policy)
	if err != nil {
		return executionInputs{}, coded(CodeInvocationRejected, err)
	}
	inputs.invocation = invocation
	return inputs, nil
}

func readIdentityBytes(config Config, session stateSession) (identityBytes, error) {
	policy, err := session.ReadLeaf(config.ExecutionPolicyPath, maxPolicyBytes, privateMode)
	if err != nil {
		return identityBytes{}, coded(CodePolicyRejected, err)
	}
	invocation, invocationErr := session.ReadLeaf(config.InvocationPath,
		maxInvocationBytes, privateMode)
	return identityBytes{policy: policy, invocation: invocation,
		invocationErr: invocationErr}, nil
}

func (value *identityBytes) clear() {
	clear(value.policy)
	clear(value.invocation)
	value.policy, value.invocation = nil, nil
}

func loadSigner(config Config, session stateSession,
	trust *bootstraprepoexecutionauthority.Trust,
) (*bootstraprepoexecutionauthority.Signer, error) {
	seed, err := session.ReadLeaf(config.ReceiptSeedPath, receiptSeedBytes, privateMode)
	if err != nil {
		return nil, coded(CodeSignerKeyRejected, err)
	}
	defer clear(seed)
	signer, err := bootstraprepoexecutionauthority.NewSigner(seed, trust)
	if err != nil {
		return nil, coded(CodeSignerKeyRejected, err)
	}
	return signer, nil
}

func clear(value []byte) {
	for index := range value {
		value[index] = 0
	}
}
