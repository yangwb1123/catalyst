package bootstrapreporeadexecution

import (
	"forgeos/forge-core/internal/bootstrapgrantauthority"
	"forgeos/forge-core/internal/bootstraprepoexecutionauthority"
	"forgeos/forge-core/internal/grantstate"
)

func appendTransition(session stateSession, snapshot grantstate.Snapshot,
	current *bootstraprepoexecutionauthority.Ledger,
	issuance *bootstrapgrantauthority.Ledger,
	inputs executionInputs,
	metadata *bootstraprepoexecutionauthority.Metadata,
	state string,
	recordedAt int64,
	reason string,
	signer *bootstraprepoexecutionauthority.Signer,
	authority *bootstraprepoexecutionauthority.Trust,
) (grantstate.Snapshot, *bootstraprepoexecutionauthority.Ledger,
	*bootstraprepoexecutionauthority.Receipt, error) {
	receipt, err := bootstraprepoexecutionauthority.IssueReceipt(current, state,
		inputs.policy, inputs.invocation, inputs.manifest, metadata, recordedAt, reason, signer)
	if err != nil {
		return grantstate.Snapshot{}, nil, nil, coded(CodeLedgerRejected, err)
	}
	next, err := bootstraprepoexecutionauthority.AppendLedger(current, issuance,
		inputs.policy, inputs.invocation, inputs.manifest, receipt, metadata, signer)
	if err != nil {
		return grantstate.Snapshot{}, nil, nil, coded(CodeLedgerRejected, err)
	}
	published, reopened, err := persistUsageLedger(session, snapshot, next, authority, issuance)
	if err != nil {
		return grantstate.Snapshot{}, nil, nil, err
	}
	return published, reopened, receipt, nil
}
