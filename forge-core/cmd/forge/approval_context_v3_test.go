package main

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/approvalcontext"
	"forgeos/forge-core/internal/approvalcontextstore"
	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/declaredartifact"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/outputbindingstore"
	"forgeos/forge-core/internal/productsource"
)

type approvalV3Fixture struct {
	root    string
	wf      asset.Workflow
	receipt outputbinding.AgentOutputReceipt
	context approvalcontext.Context
}

func TestBoundApproveInstallsExactV3AndIgnoresFlag(t *testing.T) {
	fixture := newApprovalV3Fixture(t, "design", nil,
		map[string]string{"docs/design/proposal.md": "proposal"})
	if humanApproved(fixture.root, "design", true) {
		t.Fatal("--approved bypassed a bound stage before v3 marker installation")
	}
	if code := writeApproval(fixture.root, "design", true); code != 0 {
		t.Fatalf("writeApproval code = %d", code)
	}
	if !humanApproved(fixture.root, "design", false) {
		t.Fatal("fresh bound positive marker was rejected")
	}
	path := approvalPath(fixture.root, "design")
	data, err := os.ReadFile(path)
	if err != nil || data[len(data)-1] == '\n' {
		t.Fatalf("marker wire = %q, %v", data, err)
	}
	marker, err := approvalcontext.DecodeCanonicalMarker(data)
	if err != nil || marker.AgentOutputReceiptSHA256 != fixture.receipt.ReceiptSHA256 {
		t.Fatalf("marker = %#v, %v", marker, err)
	}
	if code := writeApproval(fixture.root, "design", false); code != 0 {
		t.Fatalf("reject code = %d", code)
	}
	rejected, err := os.ReadFile(rejectionPath(fixture.root, "design"))
	if err != nil || !strings.Contains(string(rejected), decisionMarkerFormat) ||
		humanApproved(fixture.root, "design", false) {
		t.Fatalf("negative v2 marker did not fail closed: %q, %v", rejected, err)
	}
}

func TestBoundApprovalRejectsMissingOldTamperedAndConflictingMarkers(t *testing.T) {
	for _, test := range []struct {
		name  string
		write func(*testing.T, approvalV3Fixture)
	}{
		{"missing", func(t *testing.T, fixture approvalV3Fixture) {}},
		{"empty", func(t *testing.T, fixture approvalV3Fixture) {
			writeApprovalTestFile(t, approvalPath(fixture.root, "design"), nil)
		}},
		{"old-v2", func(t *testing.T, fixture approvalV3Fixture) {
			data, _ := json.Marshal(decisionMarker{Format: decisionMarkerFormat,
				Stage: "design", Decision: "approved", ActorHint: "operator", CreatedAt: time.Now().Format(time.RFC3339)})
			writeApprovalTestFile(t, approvalPath(fixture.root, "design"), append(data, '\n'))
		}},
		{"tampered-v3", func(t *testing.T, fixture approvalV3Fixture) {
			installApprovalV3ForTest(t, fixture)
			path := approvalPath(fixture.root, "design")
			data, _ := os.ReadFile(path)
			marker, _ := approvalcontext.DecodeCanonicalMarker(data)
			marker.SourceAfterSHA256 = outputbinding.SHA256([]byte("tampered"))
			data, _ = approvalcontext.CanonicalMarkerJSON(marker)
			writeApprovalTestFile(t, path, data)
		}},
		{"opposite-conflict", func(t *testing.T, fixture approvalV3Fixture) {
			installApprovalV3ForTest(t, fixture)
			writeApprovalTestFile(t, rejectionPath(fixture.root, "design"), []byte("rejected"))
		}},
	} {
		t.Run(test.name, func(t *testing.T) {
			fixture := newApprovalV3Fixture(t, "design", nil,
				map[string]string{"docs/design/proposal.md": "proposal"})
			test.write(t, fixture)
			if humanApproved(fixture.root, "design", true) {
				t.Fatal("invalid positive state satisfied bound human gate")
			}
		})
	}
}

func TestBoundApprovalRejectsSourceWorkflowJournalAndContextDrift(t *testing.T) {
	for _, test := range []struct {
		name  string
		drift func(*testing.T, approvalV3Fixture)
	}{
		{"source", func(t *testing.T, fixture approvalV3Fixture) {
			writeApprovalTestFile(t, filepath.Join(fixture.root, "product.txt"), []byte("changed"))
		}},
		{"workflow", func(t *testing.T, fixture approvalV3Fixture) {
			fixture.wf.Phases[0].Description = "changed"
			writeApprovalWorkflow(t, fixture.root, fixture.wf)
		}},
		{"journal-head", func(t *testing.T, fixture approvalV3Fixture) {
			appendApprovalReceipt(t, fixture, "later-run")
		}},
		{"journal-rollback", func(t *testing.T, fixture approvalV3Fixture) {
			writeApprovalTestFile(t, outputbindingstore.Path(fixture.root), nil)
		}},
		{"claim-rollback", func(t *testing.T, fixture approvalV3Fixture) {
			writeApprovalTestFile(t, outputbindingstore.ClaimPath(fixture.root), nil)
		}},
		{"context-wire", func(t *testing.T, fixture approvalV3Fixture) {
			path, _ := approvalcontextstore.Path(fixture.root, "design")
			data, _ := os.ReadFile(path)
			writeApprovalTestFile(t, path, append(data, '\n'))
		}},
	} {
		t.Run(test.name, func(t *testing.T) {
			fixture := newApprovalV3Fixture(t, "design", nil,
				map[string]string{"docs/design/proposal.md": "proposal"})
			installApprovalV3ForTest(t, fixture)
			test.drift(t, fixture)
			if validBoundApproval(fixture.root, "design") {
				t.Fatal("stale bound approval remained valid")
			}
		})
	}
}

func TestBoundApprovalRecapturesReleaseInputsAndOutputs(t *testing.T) {
	tests := []struct {
		name, path string
	}{
		{"input", "docs/release/fixed-input.md"},
		{"output", "docs/release/deployment-validation.md"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			fixture := newApprovalV3Fixture(t, "deploy",
				map[string]string{"docs/release/fixed-input.md": "fixed"},
				map[string]string{"docs/release/deployment-validation.md": "approved\nVERDICT: APPROVE\n"})
			installApprovalV3ForTest(t, fixture)
			writeApprovalTestFile(t, filepath.Join(fixture.root, filepath.FromSlash(test.path)), []byte("drift"))
			if validBoundApproval(fixture.root, "deploy") {
				t.Fatal("release artifact drift retained approval")
			}
		})
	}
}

func TestBoundApprovalAllowsHistoricalOverlappingInput(t *testing.T) {
	path := "docs/release/deployment-validation.md"
	fixture := newApprovalV3Fixture(t, "deploy", nil,
		map[string]string{path: "current-output\nVERDICT: APPROVE\n"})
	oldInputs, err := outputbinding.SealManifest([]outputbinding.ManifestItem{{
		Bytes: int64(len("old-input")), Path: path, SHA256: outputbinding.SHA256([]byte("old-input")),
	}})
	if err != nil {
		t.Fatal(err)
	}
	replaceApprovalReceipt(t, &fixture, oldInputs, "overlap")
	installApprovalV3ForTest(t, fixture)
	if !validBoundApproval(fixture.root, "deploy") {
		t.Fatal("historical overlapping input was incorrectly recaptured as current bytes")
	}
}

func TestBoundReleaseApprovalRequiresExactV2Receipt(t *testing.T) {
	for _, test := range []struct {
		name   string
		mutate func(*testing.T, approvalV3Fixture)
	}{
		{"v1-diagnostic-only", writeV1ReleaseReceiptForTest},
		{"reference-tamper", func(t *testing.T, fixture approvalV3Fixture) {
			path, _ := releaseValidationReceiptPath(fixture.root, "deploy")
			data, _ := os.ReadFile(path)
			var receipt releaseValidationReceipt
			if err := json.Unmarshal(data, &receipt); err != nil {
				t.Fatal(err)
			}
			receipt.ApprovalContextSHA256 = outputbinding.SHA256([]byte("wrong-context"))
			data, _ = json.Marshal(receipt)
			writeApprovalTestFile(t, path, data)
		}},
		{"noncanonical-wire", func(t *testing.T, fixture approvalV3Fixture) {
			path, _ := releaseValidationReceiptPath(fixture.root, "deploy")
			data, _ := os.ReadFile(path)
			writeApprovalTestFile(t, path, append(data, '\n'))
		}},
		{"permission-drift", func(t *testing.T, fixture approvalV3Fixture) {
			path, _ := releaseValidationReceiptPath(fixture.root, "deploy")
			if err := os.Chmod(path, 0o400); err != nil {
				t.Fatal(err)
			}
		}},
	} {
		t.Run(test.name, func(t *testing.T) {
			fixture := newApprovalV3Fixture(t, "deploy", nil,
				map[string]string{"docs/release/deployment-validation.md": "VERDICT: APPROVE\n"})
			installApprovalV3ForTest(t, fixture)
			test.mutate(t, fixture)
			if validBoundApproval(fixture.root, "deploy") {
				t.Fatal("invalid release validation receipt retained positive approval")
			}
		})
	}
}

func writeV1ReleaseReceiptForTest(t *testing.T, fixture approvalV3Fixture) {
	t.Helper()
	current, err := currentReleaseApprovalContext(fixture.root, "deploy")
	if err != nil {
		t.Fatal(err)
	}
	phase := assetPhaseReceipt{Name: fixture.receipt.Phase, RunID: fixture.receipt.RunID,
		Model: fixture.receipt.Model, AgentSHA256: strings.Repeat("a", 64),
		PromptSHA256: fixture.receipt.PromptContextSHA256}
	if err := writeReleaseValidationReceipt(fixture.root, "deploy", phase, current); err != nil {
		t.Fatal(err)
	}
}

func TestApprovalContextPathSwapFailsClosed(t *testing.T) {
	fixture := newApprovalV3Fixture(t, "design", nil,
		map[string]string{"docs/design/proposal.md": "proposal"})
	source, _ := approvalcontextstore.Path(fixture.root, "design")
	target, _ := approvalcontextstore.Path(fixture.root, "deploy")
	data, _ := os.ReadFile(source)
	writeApprovalTestFile(t, target, data)
	if _, err := verifyBoundApprovalContext(fixture.root, "deploy"); err == nil {
		t.Fatal("design context was accepted from deploy context path")
	}
}

func newApprovalV3Fixture(t *testing.T, stage string, inputs, outputs map[string]string) approvalV3Fixture {
	t.Helper()
	root := t.TempDir()
	writeApprovalTestFile(t, filepath.Join(root, "product.txt"), []byte("product"))
	outputPaths := sortedApprovalPaths(outputs)
	wf := approvalFixtureWorkflow(stage, outputPaths)
	files := mergedApprovalFiles(inputs, outputs)
	for _, phase := range wf.Phases {
		for _, path := range phase.Emits {
			if _, exists := files[path]; !exists {
				files[path] = "fixture"
			}
		}
	}
	for path, content := range files {
		writeApprovalTestFile(t, filepath.Join(root, filepath.FromSlash(path)), []byte(content))
	}
	writeApprovalWorkflow(t, root, wf)
	runBindingGit(t, root, "init", "-q")
	runBindingGit(t, root, "add", ".")
	runBindingGit(t, root, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture")
	snapshot, err := productsource.Capture(context.Background(), root, productSourceEnvironment())
	if err != nil {
		t.Fatal(err)
	}
	inputManifest := captureApprovalPaths(t, snapshot, sortedApprovalPaths(inputs))
	outputManifest := captureApprovalPaths(t, snapshot, outputPaths)
	fixture := approvalV3Fixture{root: root, wf: wf}
	fixture.receipt = appendApprovalDraft(t, fixture, inputManifest, outputManifest, snapshot, "initial")
	fixture.context, err = approvalContextFromReceipt(fixture.receipt, fixture.receipt.ObservedAtUnixMS+1)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := approvalcontextstore.Write(root, fixture.context); err != nil {
		t.Fatal(err)
	}
	if releaseApprovalStage(stage) {
		seedBoundReleaseValidationV2(t, fixture)
	}
	return fixture
}

func seedBoundReleaseValidationV2(t *testing.T, fixture approvalV3Fixture) {
	t.Helper()
	current, err := currentReleaseApprovalContext(fixture.root, fixture.wf.Stage)
	if err != nil {
		t.Fatal(err)
	}
	verified, err := verifyBoundApprovalContext(fixture.root, fixture.wf.Stage)
	if err != nil {
		t.Fatal(err)
	}
	phase := assetPhaseReceipt{
		Name: fixture.receipt.Phase, RunID: fixture.receipt.RunID,
		Model: fixture.receipt.Model, AgentSHA256: strings.Repeat("a", 64),
		PromptSHA256: fixture.receipt.PromptContextSHA256,
	}
	if err := writeBoundReleaseValidationReceipt(
		fixture.root, fixture.wf.Stage, phase, current, verified,
	); err != nil {
		t.Fatal(err)
	}
}

func approvalFixtureWorkflow(stage string, outputs []string) asset.Workflow {
	if stage == "deploy" {
		return asset.Workflow{ID: stage, Stage: stage, Readonly: true,
			OutputBindingContract: asset.OutputBindingContractLocalDigestV1,
			Phases: []asset.Phase{
				{Name: "release-planning", Agent: "release-engineer", Readonly: true,
					ModelTier: "sonnet", FeedsForward: true, Emits: append([]string(nil), releaseApprovalFiles[stage][:4]...)},
				{Name: "release-plan-validation", Agent: "release-engineer", Readonly: true,
					ModelTier: "sonnet", Emits: append([]string(nil), releaseApprovalFiles[stage][4:]...),
					OnFail: &asset.OnFail{Action: "loop_back", TargetPhase: "release-planning"}},
			}, Stop: asset.StopCondition{Type: "human_gate", HumanApproval: "required", DurableWait: true,
				Expression: "external_apply_evidence_verified_by_human == true",
				OnRejected: &asset.LoopBack{Action: "loop_back", TargetPhase: "release-planning"},
				OnApproved: asset.OnApproved{NextStage: "evolve"}}}
	}
	phase := asset.Phase{Name: "terminal-agent", Agent: "cto", Readonly: true, Emits: outputs}
	return asset.Workflow{Stage: stage, OutputBindingContract: asset.OutputBindingContractLocalDigestV1,
		Phases: []asset.Phase{phase}, Stop: asset.StopCondition{Type: "human_gate", HumanApproval: "required"}}
}

func appendApprovalDraft(t *testing.T, fixture approvalV3Fixture, inputs, outputs outputbinding.ArtifactManifest,
	snapshot productsource.Snapshot, label string) outputbinding.AgentOutputReceipt {
	t.Helper()
	phase := fixture.wf.Phases[len(fixture.wf.Phases)-1]
	policy := approvalTestPolicy(t, fixture.wf, phase)
	promptDigest := outputbinding.SHA256([]byte("prompt-" + label))
	preflight, err := outputbinding.SealPreflight(outputbinding.PreflightBinding{
		ArtifactInputsSHA256: inputs.ManifestSHA256, Attempt: 1,
		Challenge:                outputbinding.SHA256([]byte("challenge-" + label)),
		LocalRuntimePolicySHA256: policy.BindingSHA256, Phase: phase.Name,
		PromptContextSHA256: promptDigest, RunID: "run-" + label,
		SourceBeforeSHA256: snapshot.SHA256, Workflow: fixture.wf.Stage,
		WorkflowSHA256: policy.WorkflowSHA256,
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := outputbindingstore.New(fixture.root).ClaimPreflight(preflight); err != nil {
		t.Fatal(err)
	}
	draft := approvalTestReceiptDraft(
		fixture.wf.Stage, phase, inputs, outputs, snapshot, policy, preflight, promptDigest, label,
	)
	receipt, err := outputbindingstore.New(fixture.root).Append(draft)
	if err != nil {
		t.Fatal(err)
	}
	return receipt
}

func approvalTestPolicy(t *testing.T, wf asset.Workflow, phase asset.Phase) outputbinding.RuntimePolicyBinding {
	t.Helper()
	effective := mode.Effective("engineering", "mvp")
	modelName := phase.ModelTier
	if modelName == "" {
		modelName = "opus"
	}
	policy, err := outputbinding.SealRuntimePolicy(outputbinding.RuntimePolicyBinding{
		ADR: effective.ADR, Agent: phase.Agent, BuildHalt: effective.BuildHalt,
		DesignDepth: effective.DesignDepth, DiscoverDepth: effective.DiscoverDepth,
		Effect: phase.Effect, EvolveAuthority: effective.EvolveAuthority,
		EvolveDepth: effective.EvolveDepth, Executor: "test-agent", FreshContext: phase.FreshContext,
		Gates: effective.Gates, Lifecycle: "mvp", Materiality: "materiality_not_bound",
		Mode: "engineering", Model: modelName, OutputBindingContract: asset.OutputBindingContractLocalDigestV1,
		Phase: phase.Name, Readonly: phase.Readonly, ReviewDepth: effective.ReviewDepth,
		Reviewer: effective.Reviewer, Stage: wf.Stage,
		VerdictContract: phase.VerdictContract, WorkflowSHA256: checkpointWorkflowDigest(wf),
	})
	if err != nil {
		t.Fatal(err)
	}
	return policy
}

func approvalTestReceiptDraft(stage string, phase asset.Phase, inputs, outputs outputbinding.ArtifactManifest,
	snapshot productsource.Snapshot, policy outputbinding.RuntimePolicyBinding,
	preflight outputbinding.PreflightBinding, promptDigest, label string) outputbinding.AgentOutputReceipt {
	return outputbinding.AgentOutputReceipt{
		Agent: phase.Agent, ArtifactInputs: inputs, ArtifactInputsSHA256: inputs.ManifestSHA256,
		ArtifactOutputs: outputs, ArtifactOutputsSHA256: outputs.ManifestSHA256,
		Attempt: 1, BindingSHA256: preflight.BindingSHA256, Challenge: preflight.Challenge,
		Executor: policy.Executor, FinalPromptSHA256: outputbinding.SHA256([]byte("final-" + label)),
		LocalRuntimePolicySHA256: policy.BindingSHA256, Model: policy.Model,
		ObservedAtUnixMS: 100, Phase: phase.Name, PromptContextSHA256: promptDigest,
		RawOutputBytes: int64(len(label)), RawOutputSHA256: outputbinding.SHA256([]byte(label)),
		RunID: preflight.RunID, RuntimePolicy: policy, SemanticOutputBytes: int64(len(label)),
		SemanticOutputSHA256: outputbinding.SHA256([]byte(label)), SourceAfterSHA256: snapshot.SHA256,
		SourceBeforeSHA256: snapshot.SHA256, SourceRevision: snapshot.Manifest.SourceRevision,
		Workflow: stage,
	}
}

func appendApprovalReceipt(t *testing.T, fixture approvalV3Fixture, label string) {
	t.Helper()
	snapshot, err := productsource.Capture(context.Background(), fixture.root, productSourceEnvironment())
	if err != nil {
		t.Fatal(err)
	}
	appendApprovalDraft(t, fixture, fixture.receipt.ArtifactInputs,
		fixture.receipt.ArtifactOutputs, snapshot, label)
}

func replaceApprovalReceipt(t *testing.T, fixture *approvalV3Fixture,
	inputs outputbinding.ArtifactManifest, label string) {
	t.Helper()
	if err := os.Remove(outputbindingstore.Path(fixture.root)); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(outputbindingstore.ReceiptAnchorPath(fixture.root)); err != nil {
		t.Fatal(err)
	}
	snapshot, err := productsource.Capture(context.Background(), fixture.root, productSourceEnvironment())
	if err != nil {
		t.Fatal(err)
	}
	fixture.receipt = appendApprovalDraft(t, *fixture, inputs, fixture.receipt.ArtifactOutputs, snapshot, label)
	fixture.context, err = approvalContextFromReceipt(fixture.receipt, fixture.receipt.ObservedAtUnixMS+1)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := approvalcontextstore.Write(fixture.root, fixture.context); err != nil {
		t.Fatal(err)
	}
	if releaseApprovalStage(fixture.wf.Stage) {
		seedBoundReleaseValidationV2(t, *fixture)
	}
}

func installApprovalV3ForTest(t *testing.T, fixture approvalV3Fixture) {
	t.Helper()
	if err := installBoundPositiveApproval(fixture.root, fixture.wf.Stage, "operator", time.UnixMilli(200)); err != nil {
		t.Fatal(err)
	}
}

func captureApprovalPaths(t *testing.T, snapshot productsource.Snapshot, paths []string) outputbinding.ArtifactManifest {
	t.Helper()
	manifest, err := declaredartifact.Capture(context.Background(), snapshot, paths)
	if err != nil {
		t.Fatal(err)
	}
	return manifest
}

func writeApprovalWorkflow(t *testing.T, root string, wf asset.Workflow) {
	t.Helper()
	data, err := json.Marshal(wf)
	if err != nil {
		t.Fatal(err)
	}
	writeApprovalTestFile(t, filepath.Join(root, ".agent", "workflows", wf.Stage+".yml"), data)
}

func writeApprovalTestFile(t *testing.T, path string, data []byte) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatal(err)
	}
}

func sortedApprovalPaths(values map[string]string) []string {
	paths := make([]string, 0, len(values))
	for path := range values {
		paths = append(paths, path)
	}
	sort.Strings(paths)
	return paths
}

func mergedApprovalFiles(first, second map[string]string) map[string]string {
	result := map[string]string{}
	for path, content := range first {
		result[path] = content
	}
	for path, content := range second {
		result[path] = content
	}
	return result
}

func TestLegacyApprovalFlagStillWorksWithoutSelector(t *testing.T) {
	root := t.TempDir()
	if !humanApproved(root, "design", true) {
		t.Fatal("legacy project without selector lost --approved behavior")
	}
	if humanApproved(root, "design", false) {
		t.Fatal("legacy project fabricated approval")
	}
}
