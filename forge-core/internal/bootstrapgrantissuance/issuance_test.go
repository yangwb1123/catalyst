package bootstrapgrantissuance

import (
	"bytes"
	"encoding/json"
	"os"
	"testing"
)

func TestStoredIssuanceMatchesGoldenAndStrictlyPersists(t *testing.T) {
	layout := newIssuanceLayout(t)
	clock := &fixedClock{value: fixtureStoredAt}
	output, err := issueWith(layout.config, realDependencies(clock))
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(output, layout.docs.result) {
		t.Fatalf("stored output differs from golden\ngot:  %s\nwant: %s", output, layout.docs.result)
	}
	ledger, err := os.ReadFile(ledgerPath(layout))
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(ledger, layout.docs.ledger) {
		t.Fatal("durable ledger differs from cross-language golden")
	}
	if clock.calls != 1 {
		t.Fatalf("wall clock calls = %d, want 1", clock.calls)
	}
}

func TestExactReplayReadsNoClockOrPrivateKeyAndWritesNothing(t *testing.T) {
	layout := newIssuanceLayout(t)
	firstClock := &fixedClock{value: fixtureStoredAt}
	if _, err := issueWith(layout.config, realDependencies(firstClock)); err != nil {
		t.Fatal(err)
	}
	before, err := os.Stat(ledgerPath(layout))
	if err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(layout.key); err != nil {
		t.Fatal(err)
	}
	replayClock := &fixedClock{err: os.ErrInvalid}
	output, err := issueWith(layout.config, realDependencies(replayClock))
	if err != nil {
		t.Fatal(err)
	}
	assertDisposition(t, output, "exact_replay")
	after, err := os.Stat(ledgerPath(layout))
	if err != nil {
		t.Fatal(err)
	}
	if replayClock.calls != 0 || !os.SameFile(before, after) ||
		!before.ModTime().Equal(after.ModTime()) || before.Size() != after.Size() {
		t.Fatalf("replay touched clock or ledger: calls=%d before=%v after=%v",
			replayClock.calls, before.ModTime(), after.ModTime())
	}
	assertSameSignedDecision(t, layout.docs.result, output)
}

func TestAuthenticatedDenyStoresReceiptWithoutGrant(t *testing.T) {
	layout := newIssuanceLayout(t)
	policy, request := denyDocuments(t, layout.docs)
	writeLeaf(t, layout.config.AuthorityRoot, layout.config.PolicyPath, policy)
	writeLeaf(t, layout.config.AuthorityRoot, layout.config.RequestPath, request)
	clock := &fixedClock{value: fixtureStoredAt}
	output, err := issueWith(layout.config, realDependencies(clock))
	if err != nil {
		t.Fatal(err)
	}
	document := decodeDocument(t, output)
	if document["grant"] != nil || document["delivery_disposition"] != "stored" {
		t.Fatalf("deny result Grant/disposition = %#v / %#v",
			document["grant"], document["delivery_disposition"])
	}
	receipt := document["receipt"].(map[string]any)
	if receipt["decision"] != "denied" || receipt["denial_reason"] != "policy_denied" {
		t.Fatalf("deny receipt = %#v", receipt)
	}
}

func denyDocuments(t *testing.T, docs fixtureDocuments) ([]byte, []byte) {
	policy := rewriteSigned(t, docs.policy, "policy_sha256",
		[]byte("forgeos.bootstrap-grant-policy.v1\x00"),
		[]byte("forgeos.bootstrap-grant-policy.signature.v1\x00"), docs.policySeed,
		func(document map[string]any) { document["disposition"] = "deny" })
	policyNode := decodeDocument(t, policy)
	policyHash := policyNode["policy_sha256"]
	request := rewriteSigned(t, docs.request, "request_sha256",
		[]byte("forgeos.bootstrap-grant-request.v1\x00"),
		[]byte("forgeos.bootstrap-grant-request.signature.v1\x00"), docs.requestSeed,
		func(document map[string]any) { document["policy_sha256"] = policyHash })
	return policy, request
}

func assertDisposition(t *testing.T, data []byte, expected string) {
	t.Helper()
	document := decodeDocument(t, data)
	if document["delivery_disposition"] != expected {
		t.Fatalf("delivery disposition = %v, want %s", document["delivery_disposition"], expected)
	}
}

func assertSameSignedDecision(t *testing.T, stored, replay []byte) {
	t.Helper()
	first, second := decodeDocument(t, stored), decodeDocument(t, replay)
	delete(first, "delivery_disposition")
	delete(second, "delivery_disposition")
	firstBytes, _ := json.Marshal(first)
	secondBytes, _ := json.Marshal(second)
	if !bytes.Equal(firstBytes, secondBytes) {
		t.Fatal("exact replay changed the signed Grant or Receipt")
	}
}
