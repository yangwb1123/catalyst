package bootstrapgrantissuance

import (
	"errors"
	"testing"

	"forgeos/forge-core/internal/grantstate"
)

func TestCommitFaultsReturnNoOutput(t *testing.T) {
	for _, test := range []struct {
		name string
		err  error
		code Code
	}{
		{"before-rename", errors.New("injected commit failure"), CodePersistenceFailed},
		{"after-rename", &grantstate.Error{Code: grantstate.CodePersistenceUncertain,
			Op: "commit", Err: errors.New("injected uncertain failure")}, CodePersistenceUncertain},
	} {
		t.Run(test.name, func(t *testing.T) {
			layout := newIssuanceLayout(t)
			state := fixtureMemoryState(layout)
			state.commitErr = test.err
			clock := &fixedClock{value: fixtureStoredAt}
			output, err := issueWith(layout.config, memoryDependencies(clock, state))
			if output != nil || ErrorCode(err) != test.code || state.commits != 1 || state.closes != 1 {
				t.Fatalf("fault = %q, %v, commits=%d closes=%d",
					output, err, state.commits, state.closes)
			}
		})
	}
}

func TestCloseFailureWithholdsOutput(t *testing.T) {
	layout := newIssuanceLayout(t)
	state := fixtureMemoryState(layout)
	state.closeErr = errors.New("injected close failure")
	clock := &fixedClock{value: fixtureStoredAt}
	output, err := issueWith(layout.config, memoryDependencies(clock, state))
	if output != nil || ErrorCode(err) != CodePersistenceUncertain || state.commits != 1 || state.closes != 1 {
		t.Fatalf("close fault = %q, %v, commits=%d closes=%d",
			output, err, state.commits, state.closes)
	}
}

func TestStrictPostCommitReadbackFailureIsPersistenceUncertain(t *testing.T) {
	layout := newIssuanceLayout(t)
	state := fixtureMemoryState(layout)
	state.corruptRead = true
	clock := &fixedClock{value: fixtureStoredAt}
	output, err := issueWith(layout.config, memoryDependencies(clock, state))
	if output != nil || ErrorCode(err) != CodePersistenceUncertain || state.commits != 1 {
		t.Fatalf("readback fault = %q, %v, commits=%d", output, err, state.commits)
	}
}

func TestStateBusyIsStableAndDoesNotReadClock(t *testing.T) {
	layout := newIssuanceLayout(t)
	clock := &fixedClock{err: errors.New("must not run")}
	deps := dependencies{clock: clock, openState: func(grantstate.Config) (stateSession, error) {
		return nil, &grantstate.Error{Code: grantstate.CodeBusy, Op: "lock", Err: grantstate.ErrBusy}
	}}
	output, err := issueWith(layout.config, deps)
	if output != nil || ErrorCode(err) != CodeStateBusy || clock.calls != 0 {
		t.Fatalf("busy = %q, %v, clock calls=%d", output, err, clock.calls)
	}
}

func TestClockFailureReadsNoIssuerKeyOrStateWrite(t *testing.T) {
	layout := newIssuanceLayout(t)
	state := fixtureMemoryState(layout)
	delete(state.leaves, layout.config.IssuerSeedPath)
	clock := &fixedClock{err: errors.New("clock unavailable")}
	output, err := issueWith(layout.config, memoryDependencies(clock, state))
	if output != nil || ErrorCode(err) != CodeClockRejected || clock.calls != 1 || state.commits != 0 {
		t.Fatalf("clock failure = %q, %v calls=%d commits=%d",
			output, err, clock.calls, state.commits)
	}
}

func fixtureMemoryState(layout testLayout) *memoryState {
	return &memoryState{leaves: map[string][]byte{
		layout.config.TrustRootPath:  clone(layout.docs.root),
		layout.config.PolicyPath:     clone(layout.docs.policy),
		layout.config.RequestPath:    clone(layout.docs.request),
		layout.config.IssuerSeedPath: clone(layout.docs.issuerSeed),
	}}
}

func memoryDependencies(clock clock, state *memoryState) dependencies {
	return dependencies{clock: clock, openState: func(grantstate.Config) (stateSession, error) {
		return state, nil
	}}
}
