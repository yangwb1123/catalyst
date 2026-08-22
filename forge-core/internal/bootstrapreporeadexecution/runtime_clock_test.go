//go:build linux && (amd64 || arm64)

package bootstrapreporeadexecution

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"testing"
	"time"

	"forgeos/forge-core/internal/grantstate"
	"forgeos/forge-core/internal/pinnedreporead"
)

type repositoryVerifyFailureSession struct {
	stateSession
	verifyCalls int
}

type repositoryOrderControl struct {
	reservedBeforeBind      bool
	reservedBeforePreflight bool
	sawReservation          bool
}

type repositoryOrderSession struct {
	stateSession
	control *repositoryOrderControl
}

type forbiddenVerifySession struct {
	stateSession
	called bool
}

func (session *forbiddenVerifySession) VerifyRepository() error {
	session.called = true
	return fmt.Errorf("repository verification must not run after cancellation")
}

func (session *repositoryOrderSession) Commit(expected grantstate.Snapshot, next []byte) error {
	if err := session.stateSession.Commit(expected, next); err != nil {
		return err
	}
	if bytes.Contains(next, []byte(`"state":"reserved_no_repo_io"`)) {
		session.control.sawReservation = true
	}
	return nil
}

func (session *repositoryOrderSession) BindRepository(path string) (*os.File, error) {
	session.control.reservedBeforeBind = session.control.sawReservation
	return session.stateSession.BindRepository(path)
}

func (session *repositoryVerifyFailureSession) VerifyRepository() error {
	session.verifyCalls++
	if session.verifyCalls == 1 {
		return fmt.Errorf("injected repository binding failure")
	}
	return session.stateSession.VerifyRepository()
}

func TestReservationQuarantineUsesFreshClockAndRecoversClockFailure(t *testing.T) {
	bundle := newRuntimeBundle(t)
	layout := installRuntimeBundle(t, bundle)
	clock := &sequenceClock{values: []int64{1_700_000_005_000}}
	deps := runtimeDependencies(clock, time.Millisecond)
	deps.openState = func(config grantstate.Config) (stateSession, error) {
		opened, err := openUsageState(config)
		if err != nil {
			return nil, err
		}
		return &repositoryVerifyFailureSession{stateSession: opened}, nil
	}
	if _, err := executeWith(layout.config, deps); ErrorCode(err) != CodeClockRejected {
		t.Fatalf("quarantine clock failure = %q, %v", ErrorCode(err), err)
	}
	reserved, err := os.ReadFile(layout.stateLedger)
	if err != nil {
		t.Fatal(err)
	}
	assertUsageTimeline(t, reserved, 1_700_000_005_000)

	recoveryClock := &sequenceClock{values: []int64{1_700_000_006_000}}
	if _, err = executeWith(layout.config,
		runtimeDependencies(recoveryClock, time.Millisecond)); ErrorCode(err) != CodeGrantConsumed {
		t.Fatalf("orphan recovery = %q, %v", ErrorCode(err), err)
	}
	quarantined, err := os.ReadFile(layout.stateLedger)
	if err != nil {
		t.Fatal(err)
	}
	assertUsageTimeline(t, quarantined, 1_700_000_005_000, 1_700_000_006_000)
}

func TestDurableReservationPrecedesRepositoryBinding(t *testing.T) {
	bundle := newRuntimeBundle(t)
	layout := installRuntimeBundle(t, bundle)
	control := &repositoryOrderControl{}
	clock := &sequenceClock{values: []int64{1_700_000_005_000,
		1_700_000_006_000, 1_700_000_007_000}}
	deps := runtimeDependencies(clock, time.Millisecond)
	deps.preflightRepository = func(repository *os.File) error {
		control.reservedBeforePreflight = control.sawReservation
		return pinnedreporead.PreflightRepository(repository)
	}
	deps.openState = func(config grantstate.Config) (stateSession, error) {
		opened, err := openUsageState(config)
		if err != nil {
			return nil, err
		}
		return &repositoryOrderSession{stateSession: opened, control: control}, nil
	}
	if _, err := executeWith(layout.config, deps); err != nil {
		t.Fatal(err)
	}
	if !control.reservedBeforeBind || !control.reservedBeforePreflight {
		t.Fatalf("repository bind/preflight preceded reservation: bind=%v preflight=%v",
			control.reservedBeforeBind, control.reservedBeforePreflight)
	}
}

func TestCanceledDeadlinePrecedesCompositeRepositoryVerification(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	session := &forbiddenVerifySession{}
	if err := verifyRepositoryWithinDeadline(ctx, session); err == nil || session.called {
		t.Fatalf("canceled verification = %v, called=%v", err, session.called)
	}
}

func assertUsageTimeline(t *testing.T, encoded []byte, expected ...int64) {
	t.Helper()
	document := decodeTestJSON(t, encoded)
	entries := document["entries"].([]any)
	if len(entries) != len(expected) {
		t.Fatalf("usage entries = %d, want %d", len(entries), len(expected))
	}
	for index, value := range expected {
		receipt := entries[index].(map[string]any)["receipt"].(map[string]any)
		got, err := receipt["recorded_at_unix_ms"].(json.Number).Int64()
		if err != nil || got != value {
			t.Fatalf("transition %d recorded_at = %d, want %d", index, got, value)
		}
	}
}
