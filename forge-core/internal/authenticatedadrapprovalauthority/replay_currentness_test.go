//go:build unix

package authenticatedadrapprovalauthority

import (
	"testing"

	contract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

func TestExactReplayUsesCurrentStoredRevocationChain(t *testing.T) {
	t.Run("benign chain advance", func(t *testing.T) {
		fixture := newServiceFixture(t)
		config := fixture.config(t)
		original := fixture.encoded(t)
		if _, err := AuthorizeAndStore(config, original, fixture.trust()); err != nil {
			t.Fatal(err)
		}
		appendDeniedSnapshotEntry(t, fixture, config, nil, nil)
		replayInput := withCurrentSnapshots(t, original, fixture)
		replay, err := AuthorizeAndStore(config, replayInput, fixture.trust())
		if err != nil || replay == nil || replay.Verified() == nil ||
			replay.Verified().AuthorizationDecision() != "acceptance_transition_authorized" {
			t.Fatalf("benign advanced replay failed: %v", err)
		}
		assertLedgerEntryCount(t, config, 2)
	})
	t.Run("later approval revocation", func(t *testing.T) {
		fixture := newServiceFixture(t)
		config := fixture.config(t)
		original := fixture.encoded(t)
		if _, err := AuthorizeAndStore(config, original, fixture.trust()); err != nil {
			t.Fatal(err)
		}
		approval := fixture.request["approval_records"].([]any)[0].(map[string]any)
		appendDeniedSnapshotEntry(t, fixture, config,
			[]string{approval["approval_id"].(string)}, nil)
		replayInput := withCurrentSnapshots(t, original, fixture)
		replay, err := AuthorizeAndStore(config, replayInput, fixture.trust())
		if replay != nil {
			t.Fatal("revoked replay returned output")
		}
		assertCode(t, err, codeAuthorizationNotCurrent)
		assertLedgerEntryCount(t, config, 2)
	})
}

func appendDeniedSnapshotEntry(t *testing.T, fixture *serviceFixture, config Config,
	revokedApprovals, revokedKeys []string) {
	t.Helper()
	ledgerSHA := readLedgerSHA(t, config)
	fixture.appendRevocation(t, revokedApprovals, revokedKeys)
	fixture.policy["disposition"] = "deny"
	fixture.sealPolicy(t)
	if len(revokedApprovals) > 0 || len(revokedKeys) > 0 {
		fixture.request["approval_records"] = []any{}
	} else {
		fixture.sealApprovals(t)
	}
	fixture.advanceRequest(t, 2, &ledgerSHA, "adr0081-current-revocation-ledger-entry")
	result, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust())
	if err != nil || result.Verified() == nil ||
		result.Verified().AuthorizationDecision() != "acceptance_transition_not_authorized" {
		t.Fatalf("failed to append current revocation state: %v", err)
	}
}

func withCurrentSnapshots(t *testing.T, original contract.EncodedAuthorizationInput,
	fixture *serviceFixture) contract.EncodedAuthorizationInput {
	t.Helper()
	current := fixture.encoded(t)
	result := cloneEncodedInput(original)
	result.RevocationSnapshots = current.RevocationSnapshots
	return result
}
