package authenticatedadrlifecyclecontract

import (
	"bytes"
	"encoding/hex"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"forgeos/forge-core/internal/adrv2"
)

func TestGoldenPinsAndCanonicalProductionDecode(t *testing.T) {
	raw := goldenPhysical(t)
	if sha256Bytes(raw) != goldenPhysicalSHA256Pin {
		t.Fatalf("golden pin drifted: %s", sha256Bytes(raw))
	}
	if !bytes.HasSuffix(raw, []byte{'\n'}) || bytes.HasSuffix(raw, []byte("\n\n")) {
		t.Fatal("golden must carry exactly one physical LF")
	}
	if _, err := DecodeCanonicalBundle(raw); err == nil {
		t.Fatal("production decoder accepted the golden-only LF")
	}
	bundle, err := DecodeCanonicalBundle(raw[:len(raw)-1])
	if err != nil {
		t.Fatal(err)
	}
	canonical, err := CanonicalBundleJSON(bundle)
	if err != nil || !bytes.Equal(canonical, raw[:len(raw)-1]) {
		t.Fatalf("canonical bundle mismatch: %v", err)
	}
}

func TestSchemaAndProposalPins(t *testing.T) {
	root := repositoryRoot(t)
	assertFilePin(t, filepath.Join(root, "docs", "contracts",
		"authenticated-architecture-decision-lifecycle-v1.schema.json"), schemaPhysicalSHA256Pin)
	assertFilePin(t, filepath.Join(root, "docs", "contracts",
		"authenticated-architecture-decision-approval-v1.schema.json"), approvalSchemaSHA256Pin)
	assertFilePin(t, filepath.Join(root, "docs", "contracts",
		"architecture-decision-record-v2.schema.json"), adrV2SchemaSHA256Pin)
	proposalNames := []string{"ADR-9003-lifecycle-head-a.md", "ADR-9004-lifecycle-head-b.md",
		"ADR-9005-lifecycle-join.md"}
	for index, name := range proposalNames {
		assertProposalPin(t, filepath.Join(root, "docs", "contracts", "fixtures", name),
			name, index)
	}
}

func assertFilePin(t *testing.T, path, expected string) {
	t.Helper()
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if actual := sha256Bytes(raw); actual != expected {
		t.Fatalf("%s pin drifted: %s", path, actual)
	}
}

func assertProposalPin(t *testing.T, path, name string, index int) {
	t.Helper()
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if actual := sha256Bytes(raw); actual != proposalPhysicalSHA256Pins[index] {
		t.Fatalf("%s physical pin drifted: %s", name, actual)
	}
	document, err := adrv2.ValidateDocument(name, raw)
	if err != nil {
		t.Fatal(err)
	}
	frontmatter := document.Frontmatter
	if frontmatter.Status != "proposed" || frontmatter.AcceptanceID != nil ||
		frontmatter.AcceptedAtUnixMS != nil || len(frontmatter.SupersededBy) != 0 {
		t.Fatalf("%s is not strict immutable Proposed v2", name)
	}
	if frontmatter.BodySHA256 != proposalBodySHA256Pins[index] ||
		frontmatter.SelfSHA256 != proposalSelfSHA256Pins[index] {
		t.Fatalf("%s body/self pin drifted", name)
	}
}

func TestGoldenLedgerJoinAndIndependentSequences(t *testing.T) {
	bundle, err := DecodeCanonicalBundle(goldenInstance(t))
	if err != nil {
		t.Fatal(err)
	}
	facts, err := StructuralFacts(bundle)
	if err != nil {
		t.Fatal(err)
	}
	sequences, approvalSequences := make([]int64, len(facts.Entries)), make([]int64, len(facts.Entries))
	for index, entry := range facts.Entries {
		sequences[index], approvalSequences[index] = entry.Sequence, entry.ApprovalReceiptSequence
	}
	if !reflect.DeepEqual(sequences, []int64{1, 2, 3}) ||
		!reflect.DeepEqual(approvalSequences, []int64{2, 1, 7}) {
		t.Fatalf("ledger sequences coupled or drifted: %v / %v", sequences, approvalSequences)
	}
	if !reflect.DeepEqual(facts.Entries[2].TargetADRs, []string{"ADR-9003", "ADR-9004"}) {
		t.Fatalf("atomic join targets drifted: %v", facts.Entries[2].TargetADRs)
	}
	if !reflect.DeepEqual(facts.HeadADRIDs, []string{"ADR-9005"}) {
		t.Fatalf("head set drifted: %v", facts.HeadADRIDs)
	}
	statuses := []string{facts.Decisions[0].Status, facts.Decisions[1].Status,
		facts.Decisions[2].Status}
	if !reflect.DeepEqual(statuses, []string{"superseded", "superseded", "accepted"}) {
		t.Fatalf("view statuses drifted: %v", statuses)
	}
}

func TestRebuildResultAndSignatureHelpers(t *testing.T) {
	bundle, err := DecodeCanonicalBundle(goldenInstance(t))
	if err != nil {
		t.Fatal(err)
	}
	rebuilt, err := RebuildMaterializedView(bundle)
	if err != nil {
		t.Fatal(err)
	}
	view, err := CanonicalMaterializedViewJSON(bundle)
	if err != nil || !bytes.Equal(rebuilt, view) {
		t.Fatalf("view rebuild mismatch: %v", err)
	}
	stored, err := BuildTransitionResult(bundle, 3, "stored")
	if err != nil {
		t.Fatal(err)
	}
	result, err := CanonicalResultJSON(bundle)
	if err != nil || !bytes.Equal(stored, result) {
		t.Fatalf("stored result rebuild mismatch: %v", err)
	}
	if _, err = BuildTransitionResult(bundle, 1, "exact_replay"); err != nil {
		t.Fatalf("historical exact replay failed: %v", err)
	}
	if _, err = BuildTransitionResult(bundle, 1, "stored"); err == nil {
		t.Fatal("historical stored result was accepted")
	}
	checks, err := SignatureChecks(bundle)
	if err != nil {
		t.Fatal(err)
	}
	assertSignatureChecks(t, checks)
}

func assertSignatureChecks(t *testing.T, checks []SignatureCheck) {
	t.Helper()
	if len(checks) != 15 {
		t.Fatalf("signature check count = %d, want 15", len(checks))
	}
	domainCounts := map[string]int{}
	for _, check := range checks {
		if len(check.Message) == 0 || len(check.Signature) != 64 || check.Key.KeyID == "" {
			t.Fatalf("malformed detached signature check: %+v", check)
		}
		digest, decodeErr := hex.DecodeString(check.ArtifactSHA256)
		if decodeErr != nil || !bytes.Equal(check.Message,
			append(append([]byte(nil), []byte(check.Domain)...), digest...)) {
			t.Fatalf("signature message/domain drifted: %+v", check)
		}
		domainCounts[check.Domain]++
	}
	expectedCounts := map[string]int{
		"forgeos.authenticated-architecture-decision-approval.authorization-receipt.signature.v1\x00": 3,
		approvalLedgerSignatureDomain: 3, requestSignatureDomain: 3,
		acceptanceSignatureDomain: 3, supersessionSignatureDomain: 2,
		stateSignatureDomain: 1,
	}
	if !reflect.DeepEqual(domainCounts, expectedCounts) {
		t.Fatalf("signature domain inventory drifted: %#v", domainCounts)
	}
}
