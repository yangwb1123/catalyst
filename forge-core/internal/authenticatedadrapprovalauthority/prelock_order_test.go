//go:build unix

package authenticatedadrapprovalauthority

import (
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestInvalidPrelockInputsCreateNoLockOrLedger(t *testing.T) {
	cases := []struct {
		name   string
		mutate func(*testing.T, *serviceFixture, *ExternalTrust)
		code   Code
	}{
		{"signature", func(*testing.T, *serviceFixture, *ExternalTrust) {},
			codeSignatureRejected},
		{"root pin", func(_ *testing.T, _ *serviceFixture, trust *ExternalTrust) {
			trust.PinnedTrustRootSHA256 = "0000000000000000000000000000000000000000000000000000000000000000"
		}, codeTrustRootRejected},
		{"expired", func(_ *testing.T, fixture *serviceFixture, trust *ExternalTrust) {
			trust.ObservedAtUnixMS = fixture.request["expires_at_unix_ms"].(int64)
		}, codeTimeRejected},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			fixture := newServiceFixture(t)
			config := fixture.config(t)
			trust := fixture.trust()
			encoded := fixture.encoded(t)
			if test.name == "signature" {
				encoded.Request = mutateSignatureByte(t, encoded.Request)
			} else {
				test.mutate(t, fixture, &trust)
			}
			result, err := AuthorizeAndStore(config, encoded, trust)
			if result != nil {
				t.Fatal("invalid prelock input returned output")
			}
			assertCode(t, err, test.code)
			assertStateLeavesAbsent(t, config)
		})
	}
}

func TestLockedRootMustEqualPreflightBytes(t *testing.T) {
	fixture := newServiceFixture(t)
	state := fixtureMemoryState(t, fixture)
	deps := memoryDependencies(state)
	preflightRoot := cloneBytes(state.root)
	deps.readTrustRoot = func(Config) ([]byte, error) {
		return cloneBytes(preflightRoot), nil
	}
	state.root = append([]byte(nil), state.root...)
	state.root[len(state.root)-1] ^= 1
	result, err := authorizeAndStoreWith(memoryConfig(), fixture.encoded(t),
		fixture.trust(), deps)
	if result != nil || state.seedReads != 0 || state.commits != 0 {
		t.Fatalf("root rebind failure leaked output/seed/commit: %v %d %d",
			result, state.seedReads, state.commits)
	}
	assertCode(t, err, codeTrustRootRejected)
}

func TestLatestRevocationsRejectAuthorizedInputBeforeStateOpen(t *testing.T) {
	for _, target := range []string{"policy", "request", "approver", "approval", "state"} {
		t.Run(target, func(t *testing.T) {
			fixture := newServiceFixture(t)
			original := fixture.encoded(t)
			var approvals, keys []string
			switch target {
			case "policy":
				keys = []string{fixture.byUsage["approval_policy_sign"]}
			case "request":
				keys = []string{fixture.byUsage["approval_request_auth"]}
			case "approver":
				record := fixture.request["approval_records"].([]any)[0].(map[string]any)
				keys = []string{record["authority_proof"].(map[string]any)["key_id"].(string)}
			case "approval":
				record := fixture.request["approval_records"].([]any)[0].(map[string]any)
				approvals = []string{record["approval_id"].(string)}
			case "state":
				keys = []string{fixture.byUsage["approval_authorization_state_sign"]}
			}
			fixture.appendRevocation(t, approvals, keys)
			input := withCurrentSnapshots(t, original, fixture)
			state := fixtureMemoryState(t, fixture)
			deps := memoryDependencies(state)
			opens := 0
			deps.openState = func(Config) (stateSession, error) {
				opens++
				return state, nil
			}
			result, err := authorizeAndStoreWith(memoryConfig(), input,
				fixture.trust(), deps)
			if result != nil || opens != 0 || state.seedReads != 0 || state.commits != 0 {
				t.Fatalf("revoked input reached state: result=%v opens=%d seed=%d commits=%d",
					result, opens, state.seedReads, state.commits)
			}
			assertCode(t, err, codeAuthorizationNotCurrent)
		})
	}
}

func TestLatestRevocationRejectsDeniedInputBeforeStateOpen(t *testing.T) {
	fixture := newServiceFixture(t)
	denyFixturePolicy(t, fixture)
	original := fixture.encoded(t)
	record := fixture.request["approval_records"].([]any)[0].(map[string]any)
	keyID := record["authority_proof"].(map[string]any)["key_id"].(string)
	fixture.appendRevocation(t, nil, []string{keyID})
	input := withCurrentSnapshots(t, original, fixture)
	state := fixtureMemoryState(t, fixture)
	deps := memoryDependencies(state)
	opens := 0
	deps.openState = func(Config) (stateSession, error) {
		opens++
		return state, nil
	}
	result, err := authorizeAndStoreWith(memoryConfig(), input, fixture.trust(), deps)
	if result != nil || opens != 0 || state.seedReads != 0 || state.commits != 0 {
		t.Fatalf("revoked denied input reached state: result=%v opens=%d seed=%d commits=%d",
			result, opens, state.seedReads, state.commits)
	}
	assertCode(t, err, codeAuthorizationNotCurrent)
}

func assertStateLeavesAbsent(t *testing.T, config Config) {
	t.Helper()
	for _, name := range []string{stateLockFile, stateLedgerFile} {
		_, err := os.Lstat(filepath.Join(config.AuthorityRoot, config.StateDir, name))
		if !errors.Is(err, os.ErrNotExist) {
			t.Fatalf("invalid prelock input created %s: %v", name, err)
		}
	}
}
