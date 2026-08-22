//go:build unix

package authenticatedadrapprovalauthority

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"sync"
	"testing"

	contract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

func TestAuthorizeStoreVerifyAndExactReplay(t *testing.T) {
	fixture := newServiceFixture(t)
	config := fixture.config(t)
	first, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust())
	if err != nil {
		t.Fatal(err)
	}
	firstVerified := first.Verified()
	if firstVerified == nil ||
		firstVerified.AuthorizationDecision() != "acceptance_transition_authorized" ||
		firstVerified.ReceiptSHA256() == "" || firstVerified.ProposalBindingSHA256() == "" {
		t.Fatalf("unexpected verified result: %+v", first)
	}
	verified, err := VerifyBundle(firstVerified.CanonicalJSON(), fixture.trust())
	if err != nil || verified.ReceiptSHA256() != firstVerified.ReceiptSHA256() {
		t.Fatalf("stored bundle did not verify: %v", err)
	}
	ledgerPath := filepath.Join(config.AuthorityRoot, config.StateDir, stateLedgerFile)
	before, err := os.ReadFile(ledgerPath)
	if err != nil {
		t.Fatal(err)
	}
	replay, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust())
	if err != nil {
		t.Fatal(err)
	}
	replayVerified := replay.Verified()
	after, _ := os.ReadFile(ledgerPath)
	if replayVerified == nil || !bytes.Equal(before, after) ||
		replayVerified.ReceiptSHA256() != firstVerified.ReceiptSHA256() {
		t.Fatal("exact replay changed ledger or receipt")
	}
	assertDeliveryEnvelopeUnsigned(t, replayVerified.CanonicalJSON())
}

func TestAuthorizeRejectsSignatureDomainPinAndTimeMutations(t *testing.T) {
	t.Run("signature byte", func(t *testing.T) {
		fixture := newServiceFixture(t)
		encoded := fixture.encoded(t)
		encoded.Request = mutateSignatureByte(t, encoded.Request)
		_, err := AuthorizeAndStore(fixture.config(t), encoded, fixture.trust())
		assertCode(t, err, codeSignatureRejected)
	})
	t.Run("wrong domain", func(t *testing.T) {
		fixture := newServiceFixture(t)
		digest := fixture.request["request_sha256"].(string)
		key := fixture.private[fixture.byUsage["approval_request_auth"]]
		fixture.request["signature"].(map[string]any)["signature_base64url"] =
			testSignDigest(t, key, testPolicySignDomain, digest)
		_, err := AuthorizeAndStore(fixture.config(t), fixture.encoded(t), fixture.trust())
		assertCode(t, err, codeSignatureRejected)
	})
	t.Run("root pin", func(t *testing.T) {
		fixture := newServiceFixture(t)
		trust := fixture.trust()
		trust.PinnedTrustRootSHA256 = string(bytes.Repeat([]byte{'0'}, 64))
		_, err := AuthorizeAndStore(fixture.config(t), fixture.encoded(t), trust)
		assertCode(t, err, codeTrustRootRejected)
	})
	t.Run("expiry boundary", func(t *testing.T) {
		fixture := newServiceFixture(t)
		trust := fixture.trust()
		trust.ObservedAtUnixMS = fixture.request["expires_at_unix_ms"].(int64)
		_, err := AuthorizeAndStore(fixture.config(t), fixture.encoded(t), trust)
		assertCode(t, err, codeTimeRejected)
	})
}

func TestAuthorizeClockRegressionAndIdempotencyConflict(t *testing.T) {
	fixture := newServiceFixture(t)
	config := fixture.config(t)
	if _, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust()); err != nil {
		t.Fatal(err)
	}
	regressed := fixture.trust()
	regressed.ObservedAtUnixMS--
	_, err := AuthorizeAndStore(config, fixture.encoded(t), regressed)
	assertCode(t, err, codeTimeRejected)

	fixture.request["expires_at_unix_ms"] = fixture.request["expires_at_unix_ms"].(int64) - 1
	fixture.sealRequest(t)
	_, err = AuthorizeAndStore(config, fixture.encoded(t), fixture.trust())
	assertCode(t, err, codeIdempotencyConflict)
}

func TestConcurrentIdenticalRequestsAppendAtMostOnce(t *testing.T) {
	fixture := newServiceFixture(t)
	config := fixture.config(t)
	start := make(chan struct{})
	errorsFound := make([]error, 2)
	var wait sync.WaitGroup
	for index := range errorsFound {
		wait.Add(1)
		go func(slot int) {
			defer wait.Done()
			<-start
			_, errorsFound[slot] = AuthorizeAndStore(config, fixture.encoded(t), fixture.trust())
		}(index)
	}
	close(start)
	wait.Wait()
	successes := 0
	for _, err := range errorsFound {
		if err == nil {
			successes++
		} else if ErrorCode(err) != codeStateBusy {
			t.Fatalf("unexpected concurrent outcome: %v", ErrorCode(err))
		}
	}
	if successes < 1 {
		t.Fatal("no concurrent request completed")
	}
	assertLedgerEntryCount(t, config, 1)
}

func TestConcurrentConflictingRequestsAppendAtMostOnce(t *testing.T) {
	fixture := newServiceFixture(t)
	config := fixture.config(t)
	inputs := []contract.EncodedAuthorizationInput{
		fixture.encoded(t), contract.EncodedAuthorizationInput{}}
	fixture.advanceRequest(t, 1, nil, "adr0081-conflicting-genesis-request")
	inputs[1] = fixture.encoded(t)
	trust := fixture.trust()
	start := make(chan struct{})
	errorsFound := make([]error, len(inputs))
	var wait sync.WaitGroup
	for index := range inputs {
		wait.Add(1)
		go func(slot int) {
			defer wait.Done()
			<-start
			_, errorsFound[slot] = AuthorizeAndStore(config, inputs[slot], trust)
		}(index)
	}
	close(start)
	wait.Wait()
	successes := 0
	for _, err := range errorsFound {
		if err == nil {
			successes++
			continue
		}
		code := ErrorCode(err)
		if code != codeStateBusy && code != codeCASConflict {
			t.Fatalf("unexpected conflicting outcome: %v", code)
		}
	}
	if successes != 1 {
		t.Fatalf("conflicting successes = %d, want 1", successes)
	}
	assertLedgerEntryCount(t, config, 1)
}

func assertDeliveryEnvelopeUnsigned(t *testing.T, bundle []byte) {
	t.Helper()
	var node map[string]json.RawMessage
	if err := json.Unmarshal(bundle, &node); err != nil {
		t.Fatal(err)
	}
	var result map[string]json.RawMessage
	if err := json.Unmarshal(node["authorization_result"], &result); err != nil {
		t.Fatal(err)
	}
	if len(result) != 5 || result["signature"] != nil || result["result_sha256"] != nil {
		t.Fatalf("delivery result signed or shape drifted: %v", result)
	}
}

func mutateSignatureByte(t *testing.T, raw []byte) []byte {
	t.Helper()
	var node map[string]any
	if err := json.Unmarshal(raw, &node); err != nil {
		t.Fatal(err)
	}
	signature := node["signature"].(map[string]any)["signature_base64url"].(string)
	replacement := byte('A')
	if signature[0] == replacement {
		replacement = 'B'
	}
	node["signature"].(map[string]any)["signature_base64url"] =
		string(replacement) + signature[1:]
	return testCanonical(t, node)
}

func assertCode(t *testing.T, err error, want Code) {
	t.Helper()
	if got := ErrorCode(err); got != want {
		t.Fatalf("error code = %q, want %q (error %v)", got, want, err)
	}
}

func assertLedgerEntryCount(t *testing.T, config Config, want int) {
	t.Helper()
	raw, err := os.ReadFile(filepath.Join(config.AuthorityRoot, config.StateDir, stateLedgerFile))
	if err != nil {
		t.Fatal(err)
	}
	var ledger ledgerView
	if err = json.Unmarshal(raw, &ledger); err != nil || len(ledger.Entries) != want {
		t.Fatalf("ledger entries = %d, want %d: %v", len(ledger.Entries), want, err)
	}
}
