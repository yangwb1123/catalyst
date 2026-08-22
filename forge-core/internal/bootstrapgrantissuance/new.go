package bootstrapgrantissuance

import (
	"fmt"

	"forgeos/forge-core/internal/bootstrapgrantauthority"
	"forgeos/forge-core/internal/grantstate"
)

func issueNew(config Config, clock clock, session stateSession, snapshot grantstate.Snapshot,
	ledger *bootstrapgrantauthority.Ledger, inputs authenticatedInputs) ([]byte, error) {
	storedAt, err := clock.nowUnixMilli()
	if err != nil || storedAt < 0 {
		return nil, coded(CodeClockRejected, fmt.Errorf("wall clock unavailable"))
	}
	if ledger != nil && storedAt < ledger.ClockHighWater() {
		return nil, coded(CodeClockRejected, fmt.Errorf("wall clock rolled back below ledger high-water"))
	}
	if err := bootstrapgrantauthority.ValidateIssuanceTime(
		inputs.policy, inputs.request, storedAt,
	); err != nil {
		return nil, coded(CodeClockRejected, err)
	}
	issuer, err := loadIssuer(config, session, inputs.trust)
	if err != nil {
		return nil, err
	}
	defer issuer.Close()
	grant, receipt, nextLedger, err := buildDecision(inputs, ledger, storedAt, issuer)
	if err != nil {
		return nil, coded(CodeSigningRejected, err)
	}
	nextBytes, err := bootstrapgrantauthority.CanonicalLedgerJSON(nextLedger)
	if err != nil {
		return nil, coded(CodeSigningRejected, err)
	}
	if err := session.Commit(snapshot, nextBytes); err != nil {
		return nil, persistenceError(err)
	}
	if err := verifyPublished(nextBytes, inputs, session); err != nil {
		return nil, err
	}
	result, err := bootstrapgrantauthority.StoredResult(grant, receipt)
	if err != nil {
		return nil, coded(CodePersistenceUncertain, err)
	}
	output, err := bootstrapgrantauthority.CanonicalResultJSON(result)
	if err != nil {
		return nil, coded(CodePersistenceUncertain, err)
	}
	return output, nil
}

func loadIssuer(config Config, session stateSession,
	trust *bootstrapgrantauthority.Trust) (*bootstrapgrantauthority.Issuer, error) {
	seed, err := session.ReadLeaf(config.IssuerSeedPath, issuerSeedBytes, privateMode)
	if err != nil {
		return nil, coded(CodeIssuerKeyRejected, err)
	}
	defer clear(seed)
	issuer, err := bootstrapgrantauthority.NewIssuer(seed, trust)
	if err != nil {
		return nil, coded(CodeIssuerKeyRejected, err)
	}
	return issuer, nil
}

func buildDecision(inputs authenticatedInputs, ledger *bootstrapgrantauthority.Ledger,
	storedAt int64, issuer *bootstrapgrantauthority.Issuer) (*bootstrapgrantauthority.Grant,
	*bootstrapgrantauthority.Receipt, *bootstrapgrantauthority.Ledger, error) {
	var grant *bootstrapgrantauthority.Grant
	var err error
	if inputs.policy.PolicyDisposition() == "allow" {
		grant, err = bootstrapgrantauthority.IssueGrant(inputs.policy, inputs.request, storedAt, issuer)
		if err != nil {
			return nil, nil, nil, err
		}
	}
	receipt, err := bootstrapgrantauthority.IssueReceipt(inputs.policy, inputs.request, grant,
		ledger.NextSequence(), ledger.PriorReceiptSHA256(), storedAt, issuer)
	if err != nil {
		return nil, nil, nil, err
	}
	next, err := bootstrapgrantauthority.AppendLedger(ledger, inputs.policy, inputs.request,
		grant, receipt, issuer)
	return grant, receipt, next, err
}

func persistenceError(err error) error {
	if grantstate.ErrorCode(err) == grantstate.CodePersistenceUncertain {
		return coded(CodePersistenceUncertain, err)
	}
	return coded(CodePersistenceFailed, err)
}
