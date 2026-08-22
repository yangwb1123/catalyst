package bootstrapreporeadexecution

import (
	"fmt"

	"forgeos/forge-core/internal/bootstraprepoexecutionauthority"
	"forgeos/forge-core/internal/grantstate"
)

const maxUsageTransitions = int64(256)

func executeLocked(config Config, deps dependencies, session stateSession) ([]byte, error) {
	authorities, err := loadAuthorities(config, session)
	if err != nil {
		return nil, err
	}
	snapshot, ledger, err := loadUsageLedger(session, authorities.execution, authorities.ledger)
	if err != nil {
		return nil, err
	}
	if _, _, _, active := ledger.Position(); active != "" {
		return nil, recoverOrphan(config, deps.clock, session, snapshot, ledger, authorities)
	}
	identity, err := readIdentityBytes(config, session)
	if err != nil {
		return nil, err
	}
	defer identity.clear()
	status, err := tryReplay(ledger, identity)
	if err != nil {
		return nil, err
	}
	if status.conflict {
		return nil, coded(CodeIdempotencyConflict, fmt.Errorf("Grant or record key is already bound"))
	}
	if len(status.output) != 0 {
		return status.output, nil
	}
	if status.digests && !status.found {
		return nil, coded(CodeInvocationRejected,
			fmt.Errorf("digest-only replay identity does not name a terminal usage record"))
	}
	if status.found {
		return nil, coded(CodeGrantConsumed, fmt.Errorf("Grant already has a usage record"))
	}
	inputs, err := decodeExecutionInputs(config, session, authorities, identity)
	if err != nil {
		return nil, err
	}
	if !inputs.policy.AllowsExecution() {
		return nil, coded(CodePolicyDenied, fmt.Errorf("execution Policy is deny/do_not_activate"))
	}
	return startExecution(config, deps, session, snapshot, ledger, authorities, inputs)
}

func recoverOrphan(config Config, runtimeClock clock, session stateSession,
	snapshot grantstate.Snapshot, ledger *bootstraprepoexecutionauthority.Ledger,
	authorities authorityInputs) error {
	now, err := currentTime(runtimeClock, ledger)
	if err != nil {
		return err
	}
	signer, err := loadSigner(config, session, authorities.execution)
	if err != nil {
		return err
	}
	defer signer.Close()
	next, _, found, err := bootstraprepoexecutionauthority.QuarantineOrphan(
		ledger, authorities.ledger, now, signer)
	if err != nil || !found {
		return coded(CodeLedgerRejected, fmt.Errorf("active usage tail cannot be quarantined"))
	}
	if _, _, err = persistUsageLedger(session, snapshot, next,
		authorities.execution, authorities.ledger); err != nil {
		return err
	}
	return coded(CodeGrantConsumed, fmt.Errorf("active usage tail was quarantined"))
}

func startExecution(config Config, deps dependencies, session stateSession,
	snapshot grantstate.Snapshot, ledger *bootstraprepoexecutionauthority.Ledger,
	authorities authorityInputs, inputs executionInputs) ([]byte, error) {
	next, _, _, _ := ledger.Position()
	if next > maxUsageTransitions-2 {
		return nil, coded(CodeLedgerRejected, fmt.Errorf("usage ledger lacks three transition slots"))
	}
	now, err := currentTime(deps.clock, ledger)
	if err != nil {
		return nil, err
	}
	if err = bootstraprepoexecutionauthority.ValidateExecutionTime(
		inputs.policy, inputs.invocation, now); err != nil {
		return nil, coded(CodeClockRejected, err)
	}
	signer, err := loadSigner(config, session, authorities.execution)
	if err != nil {
		return nil, err
	}
	defer signer.Close()
	return executeTransitions(deps, config.RepositoryRoot, session, snapshot, ledger,
		authorities, inputs, signer, now)
}

func currentTime(runtimeClock clock,
	ledger *bootstraprepoexecutionauthority.Ledger) (int64, error) {
	now, err := runtimeClock.nowUnixMilli()
	_, _, highWater, _ := ledger.Position()
	if err != nil || now < 0 {
		return 0, coded(CodeClockRejected, fmt.Errorf("wall clock is unavailable"))
	}
	if now < highWater {
		return 0, coded(CodeClockRejected, fmt.Errorf("wall clock regressed below usage high-water"))
	}
	return now, nil
}
