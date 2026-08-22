package bootstraprepoexecutionauthority

import (
	"bytes"
	"crypto/sha256"
	"strings"
	"testing"
)

func TestFreshAuthoritiesFullStateMachineAndReplay(t *testing.T) {
	context := newRuntimeContext(t)
	defer context.signer.Close()
	if !context.policy.AllowsExecution() {
		t.Fatal("fresh authenticated Policy did not activate execution")
	}
	ledger, err := DecodeLedger(nil, context.trust, context.issuanceLedger)
	if err != nil || ledger != nil {
		t.Fatalf("absent UsageLedger did not decode as nil: %v", err)
	}
	ledger = appendTransition(t, context, ledger, "reserved_no_repo_io", 1_700_000_005_000, "", nil)
	lateIntent := int64(1_700_000_304_000)
	ledger = appendTransition(t, context, ledger, "effect_intent", lateIntent, "", nil)
	contents := fixtureContents(t, loadFixtureDocumentOnly(t)["execution_result"])
	result, err := BuildResult(context.policy, context.invocation, context.manifest,
		contents, lateIntent+1, 17)
	if err != nil {
		t.Fatal(err)
	}
	metadata, err := BuildMetadata(result)
	if err != nil {
		t.Fatal(err)
	}
	ledger = appendTransition(t, context, ledger, "completed", lateIntent+1, "", metadata)
	delivery, state, found, conflict, err := ledger.Replay(context.policyBytes, context.invocationBytes)
	if err != nil || !found || conflict || state != "completed" || delivery == nil {
		t.Fatalf("fresh exact replay failed: state=%s found=%v conflict=%v err=%v", state, found, conflict, err)
	}
	encoded, err := CanonicalJSON(delivery)
	if err != nil || bytes.Contains(encoded, []byte("content_base64url")) ||
		!bytes.Contains(encoded, []byte(`"execution_result":null`)) {
		t.Fatalf("fresh replay leaked raw content: %s err=%v", encoded, err)
	}
	_, _, _, active := ledger.Position()
	if active != "" {
		t.Fatalf("terminal Ledger retained active state %q", active)
	}
}

func TestOrphanQuarantineUsesEmbeddedAuthenticatedInputs(t *testing.T) {
	for _, withIntent := range []bool{false, true} {
		context := newRuntimeContext(t)
		ledger := appendTransition(t, context, nil, "reserved_no_repo_io", 1_700_000_005_000, "", nil)
		if withIntent {
			ledger = appendTransition(t, context, ledger, "effect_intent", 1_700_000_006_000, "", nil)
		}
		next, receipt, found, err := QuarantineOrphan(ledger, context.issuanceLedger,
			1_700_000_400_000, context.signer)
		if err != nil || !found || receipt == nil {
			t.Fatalf("orphan quarantine failed: intent=%v found=%v err=%v", withIntent, found, err)
		}
		delivery, state, replayed, conflict, err := next.Replay(context.policyBytes, context.invocationBytes)
		if err != nil || !replayed || conflict || state != "quarantined" || delivery == nil {
			t.Fatalf("quarantine replay failed: state=%s found=%v conflict=%v err=%v",
				state, replayed, conflict, err)
		}
		encoded, _ := CanonicalJSON(delivery)
		if !bytes.Contains(encoded, []byte(`"result_metadata":null`)) {
			t.Fatalf("quarantine replay metadata is not null: %s", encoded)
		}
		if _, _, found, err = QuarantineOrphan(next, context.issuanceLedger,
			1_700_000_400_001, context.signer); err != nil || found {
			t.Fatalf("terminal Ledger was quarantined twice: found=%v err=%v", found, err)
		}
		context.signer.Close()
	}
}

func TestTransitionReasonsAndIdentityReuseFailClosed(t *testing.T) {
	context := newRuntimeContext(t)
	defer context.signer.Close()
	ledger := appendTransition(t, context, nil, "reserved_no_repo_io", 1_700_000_005_000, "", nil)
	if _, err := IssueReceipt(ledger, "failed_consumed", context.policy, context.invocation,
		context.manifest, nil, 1_700_000_006_000, "repository_read_failed", context.signer); err == nil {
		t.Fatal("failed_consumed skipped effect_intent")
	}
	ledger = appendTransition(t, context, ledger, "effect_intent", 1_700_000_006_000, "", nil)
	if _, err := IssueReceipt(ledger, "failed_consumed", context.policy, context.invocation,
		context.manifest, nil, 1_700_000_007_000, "invented_reason", context.signer); err == nil {
		t.Fatal("unsupported failure reason was accepted")
	}
	ledger = appendTransition(t, context, ledger, "failed_consumed", 1_700_000_007_000,
		"repository_read_failed", nil)
	if _, err := IssueReceipt(ledger, "reserved_no_repo_io", context.policy, context.invocation,
		context.manifest, nil, 1_700_000_008_000, "", context.signer); err == nil {
		t.Fatal("consumed Grant was reserved again")
	}
	delivery, state, found, conflict, err := ledger.Replay(context.policyBytes, context.invocationBytes)
	if err != nil || !found || conflict || state != "failed_consumed" || delivery == nil {
		t.Fatalf("failed-consumed replay failed: %s %v %v %v", state, found, conflict, err)
	}
}

func TestSignedLedgerSupportsConsecutiveDistinctGrantGroups(t *testing.T) {
	first, second := newRuntimePair(t)
	defer first.signer.Close()
	if _, err := IssueReceipt(nil, "reserved_no_repo_io", first.policy,
		second.invocation, first.manifest, nil, 1_700_000_005_000, "", first.signer); err == nil {
		t.Fatal("receipt signer accepted authenticated inputs from different Grant groups")
	}
	ledger := appendTransition(t, first, nil, "reserved_no_repo_io", 1_700_000_005_000, "", nil)
	ledger = appendTransition(t, first, ledger, "effect_intent", 1_700_000_006_000, "", nil)
	ledger = appendTransition(t, first, ledger, "failed_consumed", 1_700_000_007_000,
		"repository_read_failed", nil)
	ledger = appendTransition(t, second, ledger, "reserved_no_repo_io", 1_700_000_008_000, "", nil)
	ledger = appendTransition(t, second, ledger, "effect_intent", 1_700_000_009_000, "", nil)
	ledger = appendTransition(t, second, ledger, "quarantined", 1_700_000_010_000,
		"effect_outcome_uncertain", nil)
	encoded, err := CanonicalJSON(ledger)
	if err != nil {
		t.Fatal(err)
	}
	ledger, err = DecodeLedger(encoded, first.trust, first.issuanceLedger)
	if err != nil {
		t.Fatal(err)
	}
	assertTerminalReplay(t, ledger, first, "failed_consumed")
	assertTerminalReplay(t, ledger, second, "quarantined")
	next, _, _, active := ledger.Position()
	if next != 7 || active != "" {
		t.Fatalf("multi-Grant Ledger position differs: next=%d active=%q", next, active)
	}
}

func assertTerminalReplay(t *testing.T, ledger *Ledger, context *runtimeContext,
	wantState string) {
	t.Helper()
	delivery, state, found, conflict, err := ledger.Replay(context.policyBytes,
		context.invocationBytes)
	if err != nil || !found || conflict || delivery == nil || state != wantState {
		t.Fatalf("terminal replay differs: state=%q found=%v conflict=%v err=%v",
			state, found, conflict, err)
	}
}

func TestHigherClockWatermarkAndCapacityBounds(t *testing.T) {
	fixture := loadExecutionFixture(t)
	seed := sha256.Sum256([]byte("forgeos-adr0058-fixture-execution-receipt-sign-seed-v1"))
	ledger := cloneNode(fixture.document["usage_ledger"]).(map[string]any)
	ledger["clock_high_water_unix_ms"] = int64(1_700_000_999_999)
	reselfDigest(t, ledger, ledgerDomain, "ledger_sha256", maxLedgerBytes, true,
		"", seed[:], ledgerSignatureDomain)
	if _, err := validateLedgerDocument(ledger, fixture.trust); err != nil {
		t.Fatalf("conservative higher clock watermark was rejected: %v", err)
	}
	context := newRuntimeContext(t)
	defer context.signer.Close()
	fullByCount := &Ledger{entries: make([]*usageEntry, maxLedgerItems-2),
		document: map[string]any{}, byGrant: map[string]*usageGroup{},
		byRecord: map[string]*usageGroup{}, trust: context.trust}
	if _, err := IssueReceipt(fullByCount, "reserved_no_repo_io", context.policy,
		context.invocation, context.manifest, nil, 1_700_000_005_000, "", context.signer); err == nil {
		t.Fatal("reservation without two remaining sequence slots was accepted")
	}
	fullByBytes := &Ledger{document: oversizedLedgerNode(), byGrant: map[string]*usageGroup{},
		byRecord: map[string]*usageGroup{}, trust: context.trust}
	if _, err := IssueReceipt(fullByBytes, "reserved_no_repo_io", context.policy,
		context.invocation, context.manifest, nil, 1_700_000_005_000, "", context.signer); err == nil {
		t.Fatal("reservation without worst-case byte capacity was accepted")
	}
	if _, err := DecodeLedger(bytes.Repeat([]byte{'x'}, maxLedgerBytes+1), context.trust,
		context.issuanceLedger); err == nil {
		t.Fatal("oversized UsageLedger bytes were accepted")
	}
	documents := [][]byte{{1}, {2}, {3}}
	reserve := 3*maxReceiptBytes + maxMetadataBytes + reservationOverheadBytes + 3
	if err := validateReservationPrefixCapacity(maxLedgerBytes-reserve, documents...); err != nil {
		t.Fatalf("exact reservation capacity boundary was rejected: %v", err)
	}
	if err := validateReservationPrefixCapacity(maxLedgerBytes-reserve+1, documents...); err == nil {
		t.Fatal("historical reservation prefix beyond byte capacity was accepted")
	}
}

func appendTransition(t *testing.T, context *runtimeContext, current *Ledger,
	state string, at int64, reason string, metadata *Metadata) *Ledger {
	t.Helper()
	receipt, err := IssueReceipt(current, state, context.policy, context.invocation,
		context.manifest, metadata, at, reason, context.signer)
	if err != nil {
		t.Fatal(err)
	}
	ledger, err := AppendLedger(current, context.issuanceLedger, context.policy,
		context.invocation, context.manifest, receipt, metadata, context.signer)
	if err != nil {
		t.Fatal(err)
	}
	return ledger
}

func oversizedLedgerNode() map[string]any {
	value := strings.Repeat("x", maxStringBytes)
	array := make([]any, 240)
	for index := range array {
		array[index] = value
	}
	return map[string]any{"a": cloneNode(array), "b": cloneNode(array),
		"c": cloneNode(array), "d": cloneNode(array)}
}
