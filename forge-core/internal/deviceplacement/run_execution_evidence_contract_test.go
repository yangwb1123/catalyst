package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/auditprojection"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"forgeos/forge-core/internal/runtimebridge/runmodel"
)

func TestRunExecutionEvidenceContractFixture(t *testing.T) {
	expected := readRunExecutionEvidenceFixture(t)
	run, err := auditprojection.ProjectRunObserved(
		model.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"},
		"conversation-001",
		runExecutionEvidenceSummary(),
	)
	if err != nil {
		t.Fatal(err)
	}
	receipt, err := ObserveSessionRunnerReceipt(sessionRunnerReceiptRequest())
	if err != nil {
		t.Fatal(err)
	}
	actual, err := ObserveRunExecutionEvidence(RunExecutionEvidenceInput{Run: run, Receipt: receipt})
	if err != nil {
		t.Fatalf("observe Run execution evidence: %v", err)
	}
	if actual.Authority != (RunExecutionEvidenceAuthority{}) || !actual.MetadataObserved || actual.ContentIncluded {
		t.Fatalf("execution evidence gained authority or content: %#v", actual)
	}
	if actual.Uncertain || actual.ReconciliationRequired {
		t.Fatalf("completed receipt was marked uncertain: %#v", actual)
	}
	if !jsonEqual(expected, actual) {
		t.Fatalf("execution evidence differs from fixture:\nwant %#v\ngot  %#v", expected, actual)
	}
	assertRunExecutionEvidenceWireIsClosedAndContentFree(t, actual)
}

func TestRunExecutionEvidenceRejectsConfusedOrAuthoritativeInputs(t *testing.T) {
	run := runExecutionEvidenceRun(t)
	receipt, err := ObserveSessionRunnerReceipt(sessionRunnerReceiptRequest())
	if err != nil {
		t.Fatal(err)
	}
	base := RunExecutionEvidenceInput{Run: run, Receipt: receipt}

	foreignOwner := base
	foreignOwner.Run.OwnerRef = strings.Repeat("a", 64)
	if _, err := ObserveRunExecutionEvidence(foreignOwner); err != ErrRunExecutionOwnerMismatch {
		t.Fatalf("foreign owner error = %v, want %v", err, ErrRunExecutionOwnerMismatch)
	}
	foreignRun := base
	foreignRun.Run.RunID = "run-foreign"
	if _, err := ObserveRunExecutionEvidence(foreignRun); err != ErrRunExecutionBindingMismatch {
		t.Fatalf("foreign Run error = %v, want %v", err, ErrRunExecutionBindingMismatch)
	}
	authoritative := base
	authoritative.Run.Authority.ExecutionAuthorized = true
	if _, err := ObserveRunExecutionEvidence(authoritative); err != ErrInvalidRunExecutionEvidence {
		t.Fatalf("authoritative Run error = %v, want %v", err, ErrInvalidRunExecutionEvidence)
	}
	authoritativeReceipt := base
	authoritativeReceipt.Receipt.Authority.ReceiptPersisted = true
	if _, err := ObserveRunExecutionEvidence(authoritativeReceipt); err != ErrInvalidRunExecutionEvidence {
		t.Fatalf("authoritative receipt error = %v, want %v", err, ErrInvalidRunExecutionEvidence)
	}

	encoded, err := json.Marshal(readRunExecutionEvidenceFixture(t))
	if err != nil {
		t.Fatal(err)
	}
	var shape map[string]json.RawMessage
	if err := json.Unmarshal(encoded, &shape); err != nil {
		t.Fatal(err)
	}
	shape["prompt"] = json.RawMessage(`"raw prompt"`)
	mutated, err := json.Marshal(shape)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := decodeRunExecutionEvidenceFixture(mutated); err == nil {
		t.Fatal("unknown content field was accepted")
	}
}

func TestRunExecutionEvidenceRetainsUncertainManualReconciliation(t *testing.T) {
	run := runExecutionEvidenceRun(t)
	request := sessionRunnerReceiptRequest()
	request.Receipt.DispositionKind = "uncertain"
	request.Receipt.Uncertain = true
	request.Receipt.ReconciliationRequired = true
	request.Receipt.ManualReviewRequired = true
	request.Receipt.FollowUp = "reconciliation_manual"
	receipt, err := ObserveSessionRunnerReceipt(request)
	if err != nil {
		t.Fatal(err)
	}
	evidence, err := ObserveRunExecutionEvidence(RunExecutionEvidenceInput{Run: run, Receipt: receipt})
	if err != nil {
		t.Fatal(err)
	}
	if !evidence.Uncertain || !evidence.ReconciliationRequired {
		t.Fatalf("uncertain evidence lost manual reconciliation: %#v", evidence)
	}
}

func assertRunExecutionEvidenceWireIsClosedAndContentFree(t *testing.T, value RunExecutionEvidence) {
	t.Helper()
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	var shape map[string]json.RawMessage
	if _, err := decodeRunExecutionEvidenceFixture(encoded); err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(encoded, &shape); err != nil {
		t.Fatal(err)
	}
	for _, forbidden := range []string{"prompt_content", "message", "output", "tool", "provider", "token", "credential", "artifact", "issuer", "subject", "tenant_id"} {
		if bytes.Contains(bytes.ToLower(encoded), []byte(`"`+forbidden+`"`)) {
			t.Fatalf("execution evidence wire contains forbidden field %q", forbidden)
		}
	}
	if len(shape) != 18 || len(shape["authority"]) == 0 {
		t.Fatalf("unexpected execution evidence shape: %v", shape)
	}
}

func runExecutionEvidenceRun(t *testing.T) auditprojection.RunObserved {
	t.Helper()
	run, err := auditprojection.ProjectRunObserved(
		model.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"},
		"conversation-001",
		runExecutionEvidenceSummary(),
	)
	if err != nil {
		t.Fatal(err)
	}
	return run
}

func runExecutionEvidenceSummary() runmodel.OwnedRunSummary {
	return runmodel.OwnedRunSummary{RunID: "run-001", PromptID: "prompt-001", CreatedAtMS: 200, LatestSequence: 5, Status: "nonterminal"}
}

func readRunExecutionEvidenceFixture(t *testing.T) RunExecutionEvidence {
	t.Helper()
	path := os.Getenv("FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE")
	if path == "" {
		path = filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "forge-run-execution-evidence-v1.json")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	fixture, err := decodeRunExecutionEvidenceFixture(encoded)
	if err != nil {
		t.Fatalf("decode Run execution evidence fixture: %v", err)
	}
	return fixture
}

func decodeRunExecutionEvidenceFixture(encoded []byte) (RunExecutionEvidence, error) {
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	var fixture RunExecutionEvidence
	if err := decoder.Decode(&fixture); err != nil {
		return RunExecutionEvidence{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return RunExecutionEvidence{}, io.ErrUnexpectedEOF
		}
		return RunExecutionEvidence{}, err
	}
	return fixture, nil
}
