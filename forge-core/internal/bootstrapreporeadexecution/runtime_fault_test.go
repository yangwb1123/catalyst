//go:build linux && (amd64 || arm64)

package bootstrapreporeadexecution

import (
	"context"
	"fmt"
	"os"
	"testing"
	"time"

	"forgeos/forge-core/internal/grantstate"
	"forgeos/forge-core/internal/pinnedreporead"
)

type commitFaultController struct {
	commitAt int
	commits  int
	publish  bool
	reads    int
}

type commitFaultSession struct {
	stateSession
	control *commitFaultController
}

func (session *commitFaultSession) Commit(expected grantstate.Snapshot, next []byte) error {
	session.control.commits++
	if session.control.commits != session.control.commitAt {
		return session.stateSession.Commit(expected, next)
	}
	if session.control.publish {
		if err := session.stateSession.Commit(expected, next); err != nil {
			return err
		}
		return &grantstate.Error{Code: grantstate.CodePersistenceUncertain,
			Op: "test commit", Err: fmt.Errorf("published then uncertain")}
	}
	return &grantstate.Error{Code: grantstate.CodePersistence,
		Op: "test commit", Err: fmt.Errorf("failed before publish")}
}

func TestRuntimeCommitFaultsNeverCauseASecondRepositoryRead(t *testing.T) {
	cases := []struct {
		name, terminal string
		commitAt       int
		publish        bool
		firstReads     int
		finalReads     int
	}{
		{"reserved failed before publish", "completed", 1, false, 0, 1},
		{"reserved published uncertain", "quarantined", 1, true, 0, 0},
		{"intent failed before publish", "quarantined", 2, false, 0, 0},
		{"intent published uncertain", "quarantined", 2, true, 0, 0},
		{"completed published uncertain", "completed", 3, true, 1, 1},
		{"completed failed before publish", "quarantined", 3, false, 1, 1},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			assertCommitFaultConverges(t, test.commitAt, test.publish,
				test.firstReads, test.finalReads, test.terminal)
		})
	}
}

func assertCommitFaultConverges(t *testing.T, commitAt int, publish bool,
	firstReads, finalReads int, terminal string) {
	t.Helper()
	bundle := newRuntimeBundle(t)
	layout := installRuntimeBundle(t, bundle)
	control := &commitFaultController{commitAt: commitAt, publish: publish}
	deps := faultDependencies(control)
	_, firstErr := executeWith(layout.config, deps)
	wantCode := CodePersistenceFailed
	if publish {
		wantCode = CodePersistenceUncertain
	}
	if ErrorCode(firstErr) != wantCode || control.reads != firstReads {
		t.Fatalf("first error/reads = %q/%d, want %q/%d", ErrorCode(firstErr),
			control.reads, wantCode, firstReads)
	}
	output, err := convergeFaultedExecution(layout.config, deps)
	if err != nil {
		t.Fatal(err)
	}
	delivery := decodeTestJSON(t, output)
	receipt := delivery["receipt"].(map[string]any)
	if delivery["delivery_disposition"] != "exact_replay" ||
		delivery["execution_result"] != nil || receipt["state"] != terminal ||
		control.reads != finalReads {
		t.Fatalf("terminal delivery/read count drifted: %s / %d", output, control.reads)
	}
}

func convergeFaultedExecution(config Config, deps dependencies) ([]byte, error) {
	output, err := executeWith(config, deps)
	if err == nil {
		clear(output)
		return executeWith(config, deps)
	}
	if ErrorCode(err) != CodeGrantConsumed {
		return nil, err
	}
	return executeWith(config, deps)
}

func faultDependencies(control *commitFaultController) dependencies {
	clock := &increasingClock{value: 1_700_000_005_000}
	monotonic := time.Unix(0, 0)
	return dependencies{checkPlatform: pinnedreporead.CheckPlatform,
		preflightRepository: pinnedreporead.PreflightRepository, clock: clock,
		monotonicNow: func() time.Time {
			monotonic = monotonic.Add(time.Millisecond)
			return monotonic
		}, openState: func(config grantstate.Config) (stateSession, error) {
			opened, err := openUsageState(config)
			if err != nil {
				return nil, err
			}
			return &commitFaultSession{stateSession: opened, control: control}, nil
		}, readFiles: func(ctx context.Context, repository *os.File,
			entries []pinnedreporead.ExpectedEntry,
			limits pinnedreporead.Limits) ([]pinnedreporead.File, error) {
			control.reads++
			return pinnedreporead.Read(ctx, repository, entries, limits)
		}}
}

type increasingClock struct{ value int64 }

func (clock *increasingClock) nowUnixMilli() (int64, error) {
	value := clock.value
	clock.value++
	return value, nil
}
