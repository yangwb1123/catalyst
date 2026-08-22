//go:build unix && !aix && !solaris

package authenticatedadrlifecycleauthority

import (
	"bytes"
	"testing"
)

func TestLockedAuthorityBytesMustEqualPreflight(t *testing.T) {
	fixture := newAuthorityFixture(t)
	authorization := fixture.approvalStored(t)
	deps := productionDependencies
	var observed *observedStateSession
	deps.openState = func(config Config) (stateSession, error) {
		base, err := openProtectedState(config)
		if err != nil {
			return nil, err
		}
		observed = &observedStateSession{stateSession: base,
			seedPath: config.StateSignerSeedPath, mutatePath: config.LifecycleTrustRootPath}
		return observed, nil
	}
	stored, err := transitionAndStoreWith(fixture.lifecycleConfig,
		fixture.lifecycleInput(t, authorization), authorization, fixture.lifecycleTrust(), deps)
	if stored != nil || observed == nil || observed.seedReads != 0 || observed.commits != 0 {
		t.Fatalf("root drift leaked output/seed/commit: %v %+v", stored, observed)
	}
	assertLifecycleCode(t, err, codeTrustRootRejected)
}

func TestProspectiveCASFailurePrecedesSeedAndCommit(t *testing.T) {
	fixture, authorization, input := positionedFreshTransition(t)
	request := loadRawObject(t, input.RequestJSON)
	request["expected_current_head_set_sha256"] = string(bytes.Repeat([]byte{'0'}, 64))
	resealLifecycleRequest(t, request, fixture.lifecycleRequestPrivate)
	input.RequestJSON = canonicalForTest(t, request)
	deps := productionDependencies
	var observed *observedStateSession
	deps.openState = func(config Config) (stateSession, error) {
		base, err := openProtectedState(config)
		if err != nil {
			return nil, err
		}
		observed = &observedStateSession{stateSession: base, seedPath: config.StateSignerSeedPath}
		return observed, nil
	}
	stored, err := transitionAndStoreWith(fixture.lifecycleConfig, input,
		authorization, fixture.lifecycleTrust(), deps)
	if stored != nil || observed == nil || observed.seedReads != 0 || observed.commits != 0 {
		t.Fatalf("CAS preflight leaked output/seed/commit: %v %+v", stored, observed)
	}
	assertLifecycleCode(t, err, codeCASConflict)
}
