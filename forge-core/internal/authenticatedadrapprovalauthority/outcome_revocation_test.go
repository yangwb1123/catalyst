//go:build unix

package authenticatedadrapprovalauthority

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"sort"
	"testing"
)

const (
	testLedgerDomain     = "forgeos.authenticated-architecture-decision-approval.authorization-ledger.v1\x00"
	testLedgerSignDomain = "forgeos.authenticated-architecture-decision-approval.authorization-ledger.signature.v1\x00"
)

func TestDeniedOutcomesAreAuthenticatedStoredWithExactPrecedence(t *testing.T) {
	cases := []struct {
		name   string
		mutate func(*testing.T, *serviceFixture)
		reason string
	}{
		{"policy denied", denyFixturePolicy, "policy_denied"},
		{"policy denied over reject", denyAndRejectFixture, "policy_denied"},
		{"authenticated reject", rejectFixtureApproval, "authenticated_reject"},
		{"insufficient approvals", removeFixtureApproval,
			"insufficient_authenticated_approvals"},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			fixture := newServiceFixture(t)
			test.mutate(t, fixture)
			result, err := AuthorizeAndStore(fixture.config(t), fixture.encoded(t),
				fixture.trust())
			if err != nil {
				t.Fatal(err)
			}
			verified := result.Verified()
			if verified == nil ||
				verified.AuthorizationDecision() != "acceptance_transition_not_authorized" {
				t.Fatal("denied outcome became authorized")
			}
			if reason := bundleReason(t, verified.CanonicalJSON()); reason != test.reason {
				t.Fatalf("reason = %q, want %q", reason, test.reason)
			}
		})
	}
}

func TestVerifyBundleRejectsLaterApprovalAndKeyRevocation(t *testing.T) {
	for _, revokeKey := range []bool{false, true} {
		name := "approval"
		if revokeKey {
			name = "key"
		}
		t.Run(name, func(t *testing.T) {
			fixture := newServiceFixture(t)
			result, err := AuthorizeAndStore(fixture.config(t), fixture.encoded(t),
				fixture.trust())
			if err != nil {
				t.Fatal(err)
			}
			canonical, trust := extendBundleRevocation(t, fixture,
				result.Verified().CanonicalJSON(), revokeKey)
			verified, err := VerifyBundle(canonical, trust)
			if verified != nil {
				t.Fatal("revoked authorization returned a verified view")
			}
			assertCode(t, err, codeAuthorizationNotCurrent)
		})
	}
}

func TestVerifyBundleRejectsGoldenFixtureAuthority(t *testing.T) {
	raw, err := os.ReadFile(filepath.Join("..", "..", "..", "docs", "contracts",
		"fixtures", "authenticated-architecture-decision-approval-v1.json"))
	if err != nil {
		t.Fatal(err)
	}
	raw = bytes.TrimSuffix(raw, []byte("\n"))
	var bundle bundleView
	if err = json.Unmarshal(raw, &bundle); err != nil {
		t.Fatal(err)
	}
	latest, err := parseLatestRevocation(bundle.Ledger)
	if err != nil {
		t.Fatal(err)
	}
	var root struct {
		RootSHA256 string `json:"root_sha256"`
		TrustEpoch int64  `json:"trust_epoch"`
	}
	if err = json.Unmarshal(bundle.Root, &root); err != nil {
		t.Fatal(err)
	}
	trust := ExternalTrust{PinnedTrustRootSHA256: root.RootSHA256,
		PinnedTrustEpoch: root.TrustEpoch, ObservedAtUnixMS: testObservedAt,
		RevocationHighWaterSequence: latest.Sequence,
		RevocationHighWaterSHA256:   latest.SHA256}
	_, err = VerifyBundle(raw, trust)
	assertCode(t, err, codeFixtureAuthority)
}

func denyFixturePolicy(t *testing.T, fixture *serviceFixture) {
	t.Helper()
	fixture.policy["disposition"] = "deny"
	fixture.sealPolicy(t)
	fixture.sealApprovals(t)
	fixture.sealRequest(t)
}

func denyAndRejectFixture(t *testing.T, fixture *serviceFixture) {
	t.Helper()
	rejectFixtureApproval(t, fixture)
	denyFixturePolicy(t, fixture)
}

func rejectFixtureApproval(t *testing.T, fixture *serviceFixture) {
	t.Helper()
	record := fixture.request["approval_records"].([]any)[0].(map[string]any)
	record["decision"] = "reject"
	record["decision_basis"].(map[string]any)["reason_codes"] =
		[]any{"architecture_decision_rejected"}
	fixture.sealApprovals(t)
	fixture.sealRequest(t)
}

func removeFixtureApproval(t *testing.T, fixture *serviceFixture) {
	t.Helper()
	fixture.request["approval_records"] = fixture.request["approval_records"].([]any)[:1]
	fixture.sealApprovals(t)
	fixture.sealRequest(t)
}

func bundleReason(t *testing.T, raw []byte) string {
	t.Helper()
	var bundle struct {
		Receipt struct {
			ReasonCodes []string `json:"reason_codes"`
		} `json:"authorization_receipt"`
	}
	if err := json.Unmarshal(raw, &bundle); err != nil || len(bundle.Receipt.ReasonCodes) != 1 {
		t.Fatalf("receipt reason shape drifted: %v", err)
	}
	return bundle.Receipt.ReasonCodes[0]
}

func extendBundleRevocation(t *testing.T, fixture *serviceFixture, raw []byte,
	revokeKey bool) ([]byte, ExternalTrust) {
	t.Helper()
	bundle, err := decodeTestObject(raw)
	if err != nil {
		t.Fatal(err)
	}
	ledger := bundle["authorization_ledger"].(map[string]any)
	snapshots := ledger["revocation_snapshots"].([]any)
	latest := cloneTestObject(snapshots[len(snapshots)-1])
	latest["prior_revocation_sha256"] = latest["revocation_sha256"]
	latest["revocation_sequence"] = int64(2)
	latest["effective_at_unix_ms"] = testObservedAt + 1
	request := bundle["authorization_request"].(map[string]any)
	record := request["approval_records"].([]any)[0].(map[string]any)
	if revokeKey {
		latest["revoked_key_ids"] = []any{
			record["authority_proof"].(map[string]any)["key_id"].(string)}
	} else {
		latest["revoked_approval_ids"] = []any{record["approval_id"].(string)}
	}
	sealTestRevocation(t, fixture, latest)
	ledger["revocation_snapshots"] = append(snapshots, latest)
	ledger["revocation_high_water_sequence"] = int64(2)
	ledger["revocation_high_water_sha256"] = latest["revocation_sha256"]
	ledger["clock_high_water_unix_ms"] = testObservedAt + 1
	sealTestLedger(t, fixture, ledger)
	bundle["authorization_result"].(map[string]any)["delivery_disposition"] = "exact_replay"
	trust := fixture.trust()
	trust.ObservedAtUnixMS = testObservedAt + 1
	trust.RevocationHighWaterSequence = 2
	trust.RevocationHighWaterSHA256 = latest["revocation_sha256"].(string)
	return testCanonical(t, bundle), trust
}

func sealTestRevocation(t *testing.T, fixture *serviceFixture, node map[string]any) {
	t.Helper()
	digest := testSelfDigest(t, testRevocationDomain, node,
		[]string{"revocation_sha256"}, true)
	node["revocation_sha256"] = digest
	testSignNode(t, node, fixture.private[fixture.byUsage["approval_revocation_sign"]],
		testRevokeSignDomain, digest)
	for _, field := range []string{"revoked_approval_ids", "revoked_key_ids"} {
		sort.Slice(node[field].([]any), func(left, right int) bool {
			return node[field].([]any)[left].(string) < node[field].([]any)[right].(string)
		})
	}
}

func sealTestLedger(t *testing.T, fixture *serviceFixture, node map[string]any) {
	t.Helper()
	digest := testSelfDigest(t, testLedgerDomain, node, []string{"ledger_sha256"}, true)
	node["ledger_sha256"] = digest
	testSignNode(t, node, fixture.private[fixture.byUsage["approval_authorization_state_sign"]],
		testLedgerSignDomain, digest)
}
