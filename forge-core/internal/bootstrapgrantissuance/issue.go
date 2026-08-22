package bootstrapgrantissuance

import (
	"bytes"
	"fmt"
	"time"

	"forgeos/forge-core/internal/bootstrapgrantauthority"
	"forgeos/forge-core/internal/grantstate"
)

type wallClock struct{}

func (wallClock) nowUnixMilli() (int64, error) { return time.Now().UnixMilli(), nil }

// IssueBootstrap authenticates, signs, durably stores, and strictly reopens
// one bootstrap decision. Returned bytes never attest that an effect ran.
func IssueBootstrap(config Config) ([]byte, error) {
	deps := dependencies{clock: wallClock{}, openState: openGrantState}
	return issueWith(config, deps)
}

func issueWith(config Config, deps dependencies) ([]byte, error) {
	if err := validateConfig(config); err != nil {
		return nil, coded(CodeInvalidConfig, err)
	}
	if deps.clock == nil || deps.openState == nil {
		return nil, coded(CodeInvalidConfig, fmt.Errorf("runtime dependencies are unavailable"))
	}
	session, err := deps.openState(stateConfig(config))
	if err != nil {
		if session != nil {
			_ = session.Close()
		}
		return nil, stateError(err)
	}
	if session == nil {
		return nil, coded(CodeStateRejected, fmt.Errorf("state session is unavailable"))
	}
	output, runErr := runLocked(config, deps.clock, session)
	closeErr := session.Close()
	if runErr != nil {
		return nil, runErr
	}
	if closeErr != nil {
		return nil, coded(CodePersistenceUncertain, closeErr)
	}
	return output, nil
}

func openGrantState(config grantstate.Config) (stateSession, error) {
	return grantstate.Open(config)
}

func stateConfig(config Config) grantstate.Config {
	return grantstate.Config{
		AuthorityRoot: config.AuthorityRoot, RepositoryRoot: config.RepositoryRoot,
		StateDir: config.StateDir, MaxBytes: maxLedgerBytes,
	}
}

func runLocked(config Config, clock clock, session stateSession) ([]byte, error) {
	inputs, err := loadAuthenticatedInputs(config, session)
	if err != nil {
		return nil, err
	}
	snapshot, ledger, err := loadLedger(inputs.trust, session)
	if err != nil {
		return nil, err
	}
	replay, found, err := findReplay(ledger, inputs)
	if err != nil || found {
		return replay, err
	}
	return issueNew(config, clock, session, snapshot, ledger, inputs)
}

func findReplay(ledger *bootstrapgrantauthority.Ledger,
	inputs authenticatedInputs) ([]byte, bool, error) {
	if ledger == nil {
		return nil, false, nil
	}
	record, found, err := ledger.FindRecord(inputs.policy, inputs.request)
	if err != nil {
		return nil, false, coded(CodeIdempotencyConflict, err)
	}
	if !found {
		return nil, false, nil
	}
	result, err := record.Result()
	if err != nil {
		return nil, false, coded(CodeLedgerRejected, err)
	}
	encoded, err := bootstrapgrantauthority.CanonicalResultJSON(result)
	if err != nil {
		return nil, false, coded(CodeLedgerRejected, err)
	}
	return encoded, true, nil
}

func verifyPublished(expected []byte, inputs authenticatedInputs,
	session stateSession) error {
	current, err := session.Current()
	if err != nil {
		return coded(CodePersistenceUncertain, err)
	}
	if !current.Present || !bytes.Equal(current.Data, expected) {
		return coded(CodePersistenceUncertain, fmt.Errorf("published ledger bytes differ"))
	}
	ledger, err := bootstrapgrantauthority.DecodeLedger(current.Data, inputs.trust)
	if err != nil {
		return coded(CodePersistenceUncertain, err)
	}
	_, found, err := ledger.FindRecord(inputs.policy, inputs.request)
	if err != nil || !found {
		return coded(CodePersistenceUncertain, fmt.Errorf("published record is absent or differs"))
	}
	return nil
}
