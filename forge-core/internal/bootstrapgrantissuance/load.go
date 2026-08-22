package bootstrapgrantissuance

import (
	"fmt"

	"forgeos/forge-core/internal/bootstrapgrantauthority"
	"forgeos/forge-core/internal/grantstate"
)

func loadAuthenticatedInputs(config Config, session stateSession) (authenticatedInputs, error) {
	rootBytes, err := session.ReadLeaf(config.TrustRootPath, maxTrustRootBytes, privateMode)
	if err != nil {
		return authenticatedInputs{}, coded(CodeTrustRootRejected, err)
	}
	trust, err := bootstrapgrantauthority.DecodePinnedTrustRoot(rootBytes, config.PinnedRootSHA256)
	clear(rootBytes)
	if err != nil {
		return authenticatedInputs{}, coded(CodeTrustRootRejected, err)
	}
	policyBytes, err := session.ReadLeaf(config.PolicyPath, maxPolicyBytes, privateMode)
	if err != nil {
		return authenticatedInputs{}, coded(CodePolicyRejected, err)
	}
	policy, err := bootstrapgrantauthority.DecodePolicy(policyBytes, trust)
	clear(policyBytes)
	if err != nil {
		return authenticatedInputs{}, coded(CodePolicyRejected, err)
	}
	return loadRequest(config, session, trust, policy)
}

func loadRequest(config Config, session stateSession, trust *bootstrapgrantauthority.Trust,
	policy *bootstrapgrantauthority.Policy) (authenticatedInputs, error) {
	requestBytes, err := session.ReadLeaf(config.RequestPath, maxRequestBytes, privateMode)
	if err != nil {
		return authenticatedInputs{}, coded(CodeRequestRejected, err)
	}
	request, err := bootstrapgrantauthority.DecodeRequest(requestBytes, trust, policy)
	clear(requestBytes)
	if err != nil {
		return authenticatedInputs{}, coded(CodeRequestRejected, err)
	}
	return authenticatedInputs{policy: policy, request: request, trust: trust}, nil
}

func loadLedger(trust *bootstrapgrantauthority.Trust,
	session stateSession) (grantstate.Snapshot, *bootstrapgrantauthority.Ledger, error) {
	snapshot, err := session.Current()
	if err != nil {
		return grantstate.Snapshot{}, nil, stateError(err)
	}
	if !snapshot.Present {
		if len(snapshot.Data) != 0 {
			return grantstate.Snapshot{}, nil, coded(CodeLedgerRejected,
				fmt.Errorf("missing ledger snapshot contains bytes"))
		}
		return snapshot, nil, nil
	}
	ledger, err := bootstrapgrantauthority.DecodeLedger(snapshot.Data, trust)
	if err != nil {
		return grantstate.Snapshot{}, nil, coded(CodeLedgerRejected, err)
	}
	return snapshot, ledger, nil
}

func clear(value []byte) {
	for index := range value {
		value[index] = 0
	}
}
