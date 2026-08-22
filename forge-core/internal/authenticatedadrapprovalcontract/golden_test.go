package authenticatedadrapprovalcontract

import (
	"bytes"
	"testing"

	"forgeos/forge-core/internal/adrv2"
)

func TestSuccessMarkerRemainsExplicitlyStructural(t *testing.T) {
	const expected = "STRUCTURALLY_VALID_AUTHENTICATED_ADR_APPROVAL_V1_CANDIDATE " +
		"(declared structure/digests/relations only; no authentication, authorization, " +
		"acceptance, persistence, effect, root-pin, time-currentness, " +
		"revocation-currentness, CAS, or durability attestation)"
	if SuccessMarker != expected {
		t.Fatalf("success marker drifted: %q", SuccessMarker)
	}
}

func TestGoldenPinsCanonicalAndOpaqueDecoders(t *testing.T) {
	bundle, instance := loadGoldenBundle(t)
	physical := loadGoldenRaw(t)
	if got := physicalSHA256(physical); got != goldenPhysicalSHA256 {
		t.Fatalf("golden physical SHA-256 = %s", got)
	}
	if got := physicalSHA256(loadSchema(t)); got != schemaPhysicalSHA256 {
		t.Fatalf("schema physical SHA-256 = %s", got)
	}
	proposal := loadProposal(t)
	if got := physicalSHA256(proposal); got != proposalPhysicalSHA256Pin {
		t.Fatalf("proposal physical SHA-256 = %s", got)
	}
	document, err := adrv2.ValidateDocument(
		"ADR-9002-authenticated-approval-target.md", proposal)
	if err != nil {
		t.Fatal(err)
	}
	if document.Frontmatter.BodySHA256 != proposalBodySHA256Pin ||
		document.Frontmatter.SelfSHA256 != proposalSelfSHA256Pin {
		t.Fatalf("proposal body/self pins = %s / %s",
			document.Frontmatter.BodySHA256, document.Frontmatter.SelfSHA256)
	}
	encoded, err := CanonicalBundleJSON(bundle)
	if err != nil || !bytes.Equal(encoded, instance) {
		t.Fatalf("canonical bundle mismatch: %v", err)
	}
	if _, err = DecodeCanonicalBundle(physical); err == nil {
		t.Fatal("instance decoder accepted physical golden LF")
	}
	root := goldenRoot(t, bundle)
	rootSHA, epoch := root.Identity()
	if rootSHA != bundle.document["trust_root"].(map[string]any)["root_sha256"] || epoch != 1 {
		t.Fatal("root identity projection drifted")
	}
	if _, err = root.ResolveKey("fixture-policy-key-1"); err != nil {
		t.Fatal(err)
	}
	facts, err := Facts(root)
	if err != nil || facts.TrustDomain != "forgeos.fixture.authenticated-adr-approval" || len(facts.RootKeys) != 6 {
		t.Fatalf("root facts = %+v, %v", facts, err)
	}
}

func TestGoldenInputLedgerAndSignatureChecks(t *testing.T) {
	bundle, _ := loadGoldenBundle(t)
	root := goldenRoot(t, bundle)
	input := goldenInput(t, bundle, root)
	ledger := goldenLedger(t, bundle, root)
	inputFacts, err := Facts(input)
	if err != nil || inputFacts.RecordKeySHA256 == "" || inputFacts.ExpectedNextSequence != 1 ||
		inputFacts.ExpectedLedgerSHA256 != nil {
		t.Fatalf("input facts = %+v, %v", inputFacts, err)
	}
	ledgerFacts, err := Facts(ledger)
	if err != nil || len(ledgerFacts.ReplayRecords) != 1 ||
		ledgerFacts.ReplayRecords[0].RecordKeySHA256 != inputFacts.RecordKeySHA256 {
		t.Fatalf("ledger facts = %+v, %v", ledgerFacts, err)
	}
	encodedLedger, err := CanonicalLedgerJSON(ledger)
	if err != nil {
		t.Fatal(err)
	}
	wantLedger, _ := boundedCanonicalJSON(bundle.document["authorization_ledger"], maxLedgerBytes, "ledger")
	if !bytes.Equal(encodedLedger, wantLedger) {
		t.Fatal("ledger canonical bytes drifted")
	}
	for name, value := range map[string]struct {
		value any
		want  int
	}{"input": {input, 7}, "ledger": {ledger, 9}, "bundle": {bundle, 9}} {
		checks, checkErr := SignatureChecks(value.value)
		if checkErr != nil || len(checks) != value.want {
			t.Fatalf("%s checks = %d, %v; want %d", name, len(checks), checkErr, value.want)
		}
		for _, check := range checks {
			if len(check.Message) != len(check.Domain)+32 || len(check.Signature) != 64 {
				t.Fatalf("%s check shape drifted", name)
			}
			if !bytes.Equal(check.Message[:len(check.Domain)], []byte(check.Domain)) {
				t.Fatalf("%s signature message domain drifted", name)
			}
		}
	}
}

func TestCanonicalReceiptDecoderProjectsDetachedFactsAndSignature(t *testing.T) {
	bundle, _ := loadGoldenBundle(t)
	root := goldenRoot(t, bundle)
	raw, err := boundedCanonicalJSON(bundle.document["authorization_receipt"],
		maxReceiptBytes, "receipt")
	if err != nil {
		t.Fatal(err)
	}
	receipt, err := DecodeCanonicalReceipt(raw, root)
	if err != nil {
		t.Fatal(err)
	}
	facts, err := Facts(receipt)
	if err != nil || facts.Kind != "receipt" ||
		facts.AuthorizationDecision != "acceptance_transition_authorized" ||
		facts.ReceiptLedgerSequence != 1 || facts.ReceiptEvaluatedAtUnixMS == 0 ||
		facts.ReceiptID == "" || facts.ReceiptSHA256 == "" ||
		len(facts.QualifyingApprovalIDs) == 0 || len(facts.AuthorizationReasonCodes) != 0 {
		t.Fatalf("receipt facts = %+v, %v", facts, err)
	}
	checks, err := SignatureChecks(receipt)
	if err != nil || len(checks) != 1 || checks[0].Artifact != "receipt" ||
		checks[0].Domain != receiptSignatureDomain || len(checks[0].Signature) != 64 {
		t.Fatalf("receipt signature checks = %+v, %v", checks, err)
	}
	facts.QualifyingApprovalIDs[0] = "caller-mutated"
	again, err := Facts(receipt)
	if err != nil || again.QualifyingApprovalIDs[0] == "caller-mutated" {
		t.Fatal("receipt facts shared caller-mutable storage")
	}
	if _, err = DecodeCanonicalReceipt(append(raw, '\n'), root); err == nil {
		t.Fatal("receipt decoder accepted a physical LF")
	}
	if _, err = DecodeCanonicalReceipt(raw, nil); err == nil {
		t.Fatal("receipt decoder accepted a nil trust root")
	}
}

func TestGoldenDraftsStoredAndReplayReconstructExactly(t *testing.T) {
	bundle, instance := loadGoldenBundle(t)
	root := goldenRoot(t, bundle)
	input := goldenInput(t, bundle, root)
	wantReceipt := bundle.document["authorization_receipt"].(map[string]any)
	evaluated := wantReceipt["evaluated_at_unix_ms"].(int64)
	receiptDraft, message, err := NewReceiptDraft(input, evaluated, nil)
	if err != nil || len(message) != len(receiptSignatureDomain)+32 {
		t.Fatalf("receipt draft: %v", err)
	}
	receiptSignature := wantReceipt["signature"].(map[string]any)["signature_base64url"].(string)
	receipt, err := SealReceipt(receiptDraft, receiptSignature)
	if err != nil || !canonicalEqual(receipt.document, wantReceipt) {
		t.Fatalf("sealed receipt mismatch: %v", err)
	}
	wantLedger := bundle.document["authorization_ledger"].(map[string]any)
	clock := wantLedger["clock_high_water_unix_ms"].(int64)
	ledgerDraft, message, err := NewLedgerDraft(input, receipt, nil, clock)
	if err != nil || len(message) != len(ledgerSignatureDomain)+32 {
		t.Fatalf("ledger draft: %v", err)
	}
	ledgerSignature := wantLedger["signature"].(map[string]any)["signature_base64url"].(string)
	ledger, err := SealLedger(ledgerDraft, ledgerSignature)
	if err != nil || !canonicalEqual(ledger.document, wantLedger) {
		t.Fatalf("sealed ledger mismatch: %v", err)
	}
	stored, err := StoredBundle(input, receipt, ledger)
	if err != nil {
		t.Fatal(err)
	}
	storedBytes, err := CanonicalBundleJSON(stored)
	if err != nil || !bytes.Equal(storedBytes, instance) {
		t.Fatalf("stored bundle mismatch: %v", err)
	}
	replay, err := ExactReplayBundle(ledger, input.request["idempotency_key"].(string))
	if err != nil {
		t.Fatal(err)
	}
	if replay.document["authorization_result"].(map[string]any)["delivery_disposition"] != "exact_replay" {
		t.Fatal("replay disposition drifted")
	}
	if _, err = CanonicalResultJSON(replay); err != nil {
		t.Fatal(err)
	}
	position, err := ledger.Position()
	if err != nil || position.NextSequence != 2 || position.LedgerSHA256 != wantLedger["ledger_sha256"] {
		t.Fatalf("ledger position = %+v, %v", position, err)
	}
}
