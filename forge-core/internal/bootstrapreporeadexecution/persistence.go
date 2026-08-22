package bootstrapreporeadexecution

import (
	"bytes"
	"fmt"

	"forgeos/forge-core/internal/bootstrapgrantauthority"
	"forgeos/forge-core/internal/bootstraprepoexecutionauthority"
	"forgeos/forge-core/internal/grantstate"
)

func loadUsageLedger(session stateSession, trust *bootstraprepoexecutionauthority.Trust,
	issuance *bootstrapgrantauthority.Ledger,
) (grantstate.Snapshot, *bootstraprepoexecutionauthority.Ledger, error) {
	snapshot, err := session.Current()
	if err != nil {
		return grantstate.Snapshot{}, nil, stateError(err)
	}
	if !snapshot.Present {
		if len(snapshot.Data) != 0 {
			return grantstate.Snapshot{}, nil, coded(CodeLedgerRejected,
				fmt.Errorf("missing usage ledger contains bytes"))
		}
		return snapshot, nil, nil
	}
	ledger, err := bootstraprepoexecutionauthority.DecodeLedger(snapshot.Data, trust, issuance)
	if err != nil || ledger == nil {
		return grantstate.Snapshot{}, nil, coded(CodeLedgerRejected, err)
	}
	return snapshot, ledger, nil
}

func persistUsageLedger(session stateSession, expected grantstate.Snapshot,
	next *bootstraprepoexecutionauthority.Ledger,
	trust *bootstraprepoexecutionauthority.Trust,
	issuance *bootstrapgrantauthority.Ledger,
) (grantstate.Snapshot, *bootstraprepoexecutionauthority.Ledger, error) {
	encoded, err := bootstraprepoexecutionauthority.CanonicalJSON(next)
	if err != nil {
		return grantstate.Snapshot{}, nil, coded(CodePersistenceFailed, err)
	}
	if err = session.Commit(expected, encoded); err != nil {
		return grantstate.Snapshot{}, nil, persistenceError(err)
	}
	current, err := session.Current()
	if err != nil || !current.Present || !bytes.Equal(current.Data, encoded) {
		return grantstate.Snapshot{}, nil, coded(CodePersistenceUncertain,
			fmt.Errorf("published usage ledger bytes differ"))
	}
	reopened, err := bootstraprepoexecutionauthority.DecodeLedger(current.Data, trust, issuance)
	if err != nil || reopened == nil {
		return grantstate.Snapshot{}, nil, coded(CodePersistenceUncertain, err)
	}
	return current, reopened, nil
}

func persistenceError(err error) error {
	if grantstate.ErrorCode(err) == grantstate.CodePersistenceUncertain {
		return coded(CodePersistenceUncertain, err)
	}
	return coded(CodePersistenceFailed, err)
}
