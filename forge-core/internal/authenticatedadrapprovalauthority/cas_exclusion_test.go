//go:build unix

package authenticatedadrapprovalauthority

import (
	"crypto/ed25519"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"os"
	"path/filepath"
	"strings"
	"testing"

	contract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

func TestBootstrapBindingExclusionsPinExactProposedDocuments(t *testing.T) {
	names := []string{
		"ADR-0079-authenticated-architecture-decision-approval-v1-prerequisite.md",
		"ADR-0080-authenticated-architecture-decision-approval-v1-proposed-candidate-governance-and-source-distribution.md",
		"ADR-0081-authenticated-architecture-decision-approval-authorization-service-v1.md",
	}
	for _, name := range names {
		t.Run(name[:8], func(t *testing.T) {
			raw, err := os.ReadFile(filepath.Join("..", "..", "..", "docs", "adr", name))
			if err != nil {
				t.Fatal(err)
			}
			frontmatter, _ := splitTestADR(t, raw)
			physical := sha256.Sum256(raw)
			binding := map[string]any{
				"adr_id":           frontmatter["adr_id"],
				"api_version":      "forgeos.architecture-decision-proposal-binding/v1",
				"body_sha256":      frontmatter["body_sha256"],
				"canonicalization": "forgeos.canonical-json/v1",
				"document_name":    name, "kind": "ArchitectureDecisionProposalBinding",
				"physical_sha256":         hex.EncodeToString(physical[:]),
				"profile_id":              "authenticated_architecture_decision_approval_v1",
				"proposal_binding_sha256": "", "self_sha256": frontmatter["self_sha256"],
				"status": "proposed",
			}
			digest := testSelfDigest(t, testProposalBindingDomain, binding,
				[]string{"proposal_binding_sha256"}, false)
			if !excludedBootstrapProposalBindings[digest] {
				t.Fatalf("exact bootstrap proposal binding %s is not excluded", digest)
			}
		})
	}
}

func TestCASPrefixCapacityAndProposalUniqueness(t *testing.T) {
	t.Run("genesis CAS", func(t *testing.T) {
		fixture := newServiceFixture(t)
		digest := strings.Repeat("a", 64)
		fixture.advanceRequest(t, 2, &digest, "adr0081-genesis-cas-conflict")
		_, err := AuthorizeAndStore(fixture.config(t), fixture.encoded(t), fixture.trust())
		assertCode(t, err, codeCASConflict)
	})
	t.Run("revocation prefix", func(t *testing.T) {
		fixture := newServiceFixture(t)
		config := fixture.config(t)
		if _, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust()); err != nil {
			t.Fatal(err)
		}
		ledgerSHA := readLedgerSHA(t, config)
		fixture.revocation["expires_at_unix_ms"] =
			fixture.revocation["expires_at_unix_ms"].(int64) - 1
		sealFixtureRevocation(t, fixture, fixture.revocation)
		fixture.request["revocation_sha256"] = fixture.revocation["revocation_sha256"]
		fixture.advanceRequest(t, 2, &ledgerSHA, "adr0081-revocation-prefix-conflict")
		_, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust())
		assertCode(t, err, codeRevocationRejected)
		assertLedgerEntryCount(t, config, 1)
	})
	t.Run("proposal uniqueness", func(t *testing.T) {
		fixture := newServiceFixture(t)
		config := fixture.config(t)
		if _, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust()); err != nil {
			t.Fatal(err)
		}
		ledgerSHA := readLedgerSHA(t, config)
		fixture.advanceRequest(t, 2, &ledgerSHA, "adr0081-second-authorized-request")
		_, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust())
		assertCode(t, err, codeProposalAlreadyAllowed)
		assertLedgerEntryCount(t, config, 1)
	})
	t.Run("capacity preflight", func(t *testing.T) {
		fixture := newServiceFixture(t)
		prior := ledgerView{Entries: make([]ledgerEntryView, 64)}
		err := requireCapacityAndPrefix(fixture.encoded(t), prior, true)
		assertCode(t, err, codeCapacityExhausted)
		encoded := contract.EncodedAuthorizationInput{
			RevocationSnapshots: make([][]byte, 257)}
		err = requireCapacityAndPrefix(encoded, ledgerView{}, false)
		assertCode(t, err, codeCapacityExhausted)
	})
}

func TestBootstrapAndConfiguredProposalExclusionsArePrelock(t *testing.T) {
	t.Run("built in ADR-0081", func(t *testing.T) {
		fixture := newServiceFixture(t)
		fixture.retargetProposal(t, "ADR-0081", "ADR-0081-self-excluded-test.md")
		fixture.requireValid(t)
		config := fixture.config(t)
		_, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust())
		assertCode(t, err, codeProposalExcluded)
		assertStateLeavesAbsent(t, config)
	})
	t.Run("configured binding", func(t *testing.T) {
		fixture := newServiceFixture(t)
		config := fixture.config(t)
		binding := fixture.policy["proposal_binding"].(map[string]any)
		config.ExtraExcludedProposalBindingSHA256s =
			[]string{binding["proposal_binding_sha256"].(string)}
		_, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust())
		assertCode(t, err, codeProposalExcluded)
		assertStateLeavesAbsent(t, config)
	})
}

func TestVerifyBundleRejectsEveryBootstrapADR(t *testing.T) {
	control := newServiceFixture(t)
	if _, err := VerifyBundle(buildFixtureBundle(t, control), control.trust()); err != nil {
		t.Fatalf("non-bootstrap verifier control failed: %v", err)
	}
	for _, adrID := range []string{"ADR-0079", "ADR-0080", "ADR-0081"} {
		t.Run(adrID, func(t *testing.T) {
			fixture := newServiceFixture(t)
			fixture.retargetProposal(t, adrID, adrID+"-direct-verifier-test.md")
			fixture.requireValid(t)
			result, err := VerifyBundle(buildFixtureBundle(t, fixture), fixture.trust())
			if result != nil {
				t.Fatal("bootstrap proposal returned verified output")
			}
			assertCode(t, err, codeProposalExcluded)
		})
	}
}

func buildFixtureBundle(t *testing.T, fixture *serviceFixture) []byte {
	t.Helper()
	root, err := contract.DecodeCanonicalTrustRoot(testCanonical(t, fixture.root))
	if err != nil {
		t.Fatal(err)
	}
	input, err := contract.DecodeAuthorizationInput(fixture.encoded(t), root)
	if err != nil {
		t.Fatal(err)
	}
	draft, message, err := contract.NewReceiptDraft(input, testObservedAt, nil)
	if err != nil {
		t.Fatal(err)
	}
	key := fixture.private[fixture.byUsage["approval_authorization_state_sign"]]
	receipt, err := contract.SealReceipt(draft,
		base64.RawURLEncoding.EncodeToString(ed25519.Sign(key, message)))
	if err != nil {
		t.Fatal(err)
	}
	ledgerDraft, message, err := contract.NewLedgerDraft(input, receipt, nil, testObservedAt)
	if err != nil {
		t.Fatal(err)
	}
	ledger, err := contract.SealLedger(ledgerDraft,
		base64.RawURLEncoding.EncodeToString(ed25519.Sign(key, message)))
	if err != nil {
		t.Fatal(err)
	}
	bundle, err := contract.StoredBundle(input, receipt, ledger)
	if err != nil {
		t.Fatal(err)
	}
	raw, err := contract.CanonicalBundleJSON(bundle)
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func TestThresholdAndSeparationOfDutyMutationsFailPrelock(t *testing.T) {
	t.Run("threshold", func(t *testing.T) {
		fixture := newServiceFixture(t)
		fixture.policy["threshold"] = int64(3)
		fixture.sealPolicy(t)
		fixture.sealApprovals(t)
		fixture.sealRequest(t)
		config := fixture.config(t)
		_, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust())
		assertCode(t, err, codeInputRejected)
		assertStateLeavesAbsent(t, config)
	})
	t.Run("separation of duty", func(t *testing.T) {
		fixture := newServiceFixture(t)
		record := fixture.request["approval_records"].([]any)[0].(map[string]any)
		record["separation_of_duty"].(map[string]any)["requester"] = map[string]any{
			"authority_domain": "forgeos.test.authenticated-adr-approval",
			"principal_id":     "different-requester", "principal_type": "operator"}
		fixture.sealApprovals(t)
		fixture.sealRequest(t)
		config := fixture.config(t)
		_, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust())
		assertCode(t, err, codeInputRejected)
		assertStateLeavesAbsent(t, config)
	})
}

func readLedgerSHA(t *testing.T, config Config) string {
	t.Helper()
	raw, err := os.ReadFile(filepath.Join(config.AuthorityRoot, config.StateDir, stateLedgerFile))
	if err != nil {
		t.Fatal(err)
	}
	var view ledgerView
	if err = decodeJSONView(raw, &view, "test ledger"); err != nil {
		t.Fatal(err)
	}
	return view.LedgerSHA256
}
