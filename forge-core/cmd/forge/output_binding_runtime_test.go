package main

import (
	"encoding/json"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/approvalcontextstore"
	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/outputbindingstore"
	"forgeos/forge-core/internal/statefs"
)

func TestOutputBindingRuntimeSealsExactReviewerReceipt(t *testing.T) {
	runtime, phase := outputBindingFixture(t)
	if err := runtime.prepare(phase, "engineering"); err != nil {
		t.Fatal(err)
	}
	runtime.recordBuild(phase, "opus", "base prompt", "", nil)
	argv, err := runtime.finalize(phase, "engineering", []string{"agent", "-p", "base prompt"})
	if err != nil {
		t.Fatal(err)
	}
	pending := runtime.pending[phase.Name]
	if pending.promptContext != outputbinding.SHA256([]byte("base prompt")) ||
		pending.finalPrompt != outputbinding.SHA256([]byte(argv[len(argv)-1])) {
		t.Fatalf("prompt digests do not bind exact executor bytes: %#v", pending)
	}
	if !strings.Contains(argv[len(argv)-1], pending.challenge) ||
		!strings.HasSuffix(argv[len(argv)-1], pending.preflight.BindingSHA256+"\n") {
		t.Fatalf("binding trailer missing: %q", argv[len(argv)-1])
	}
	semantic := "review complete\n" + reviewerBindingPrefix + pending.preflight.BindingSHA256 + "\nVERDICT: APPROVE\n"
	if err := runtime.commit(phase.Name, semantic, semantic, 0); err != nil {
		t.Fatal(err)
	}
	receipts, err := outputbindingstore.New(runtime.root).Load()
	if err != nil || len(receipts) != 1 {
		t.Fatalf("receipts = %#v, %v", receipts, err)
	}
	receipt := receipts[0]
	if receipt.Verdict == nil || *receipt.Verdict != VerdictApprove ||
		receipt.RawOutputSHA256 != receipt.SemanticOutputSHA256 || receipt.Attempt != 1 {
		t.Fatalf("receipt = %#v", receipt)
	}
	if err := runtime.validateVerdict(phase, VerdictApprove); err != nil {
		t.Fatalf("current bound verdict rejected: %v", err)
	}
}

func TestOutputBindingObserverIsNilSafeForLegacyWorkflow(t *testing.T) {
	observer := combineBuildObservers(outputBindingBuildObserver(nil))
	observer(asset.Phase{Name: "legacy"}, "model", "prompt", "source", nil)
}

func TestOutputBindingRuntimeRejectsReplayAndReadonlyDrift(t *testing.T) {
	runtime, phase := outputBindingFixture(t)
	if err := runtime.prepare(phase, "engineering"); err != nil {
		t.Fatal(err)
	}
	runtime.recordBuild(phase, "opus", "prompt", "", nil)
	if _, err := runtime.finalize(phase, "engineering", []string{"agent", "-p", "prompt"}); err != nil {
		t.Fatal(err)
	}
	pending := runtime.pending[phase.Name]
	wrong := reviewerBindingPrefix + strings.Repeat("0", 64) + "\nVERDICT: APPROVE\n"
	if err := runtime.commit(phase.Name, wrong, wrong, 0); err == nil {
		t.Fatal("wrong binding echo accepted")
	}
	if _, present, _ := readBindingLedger(runtime); present {
		t.Fatal("rejected attempt wrote a receipt")
	}
	if err := runtime.prepare(phase, "engineering"); err != nil {
		t.Fatal(err)
	}
	runtime.recordBuild(phase, "opus", "prompt", "", nil)
	if _, err := runtime.finalize(phase, "engineering", []string{"agent", "-p", "prompt"}); err != nil {
		t.Fatal(err)
	}
	pending = runtime.pending[phase.Name]
	if pending.attempt != 2 {
		t.Fatalf("attempt = %d, want 2", pending.attempt)
	}
	if err := os.WriteFile(filepath.Join(runtime.root, "product.txt"), []byte("drift"), 0o644); err != nil {
		t.Fatal(err)
	}
	valid := reviewerBindingPrefix + pending.preflight.BindingSHA256 + "\nVERDICT: APPROVE\n"
	if err := runtime.commit(phase.Name, valid, valid, 0); err == nil || !strings.Contains(err.Error(), "readonly") {
		t.Fatalf("readonly drift error = %v", err)
	}
}

func TestOutputBindingEntropyFailureStopsBeforeFinalization(t *testing.T) {
	runtime, phase := outputBindingFixture(t)
	runtime.randBytes = func([]byte) error { return errors.New("entropy unavailable") }
	if err := runtime.prepare(phase, "engineering"); err == nil || !strings.Contains(err.Error(), "entropy unavailable") {
		t.Fatalf("entropy error = %v", err)
	}
	if len(runtime.pending) != 0 {
		t.Fatalf("entropy failure retained pending state: %#v", runtime.pending)
	}
}

func TestPreflightClaimAdvancesInterruptedAttemptAfterRestart(t *testing.T) {
	runtime, phase := outputBindingFixture(t)
	runtime.randBytes = fillBindingTestByte(0x11)
	if err := runtime.prepare(phase, "engineering"); err != nil {
		t.Fatal(err)
	}
	runtime.recordBuild(phase, "opus", "prompt", "", nil)
	if _, err := runtime.finalize(phase, "engineering", []string{"agent", "-p", "prompt"}); err != nil {
		t.Fatal(err)
	}
	restarted := restartOutputBindingRuntime(runtime)
	restarted.randBytes = fillBindingTestByte(0x22)
	if err := restarted.prepare(phase, "engineering"); err != nil {
		t.Fatal(err)
	}
	if got := restarted.pending[phase.Name].attempt; got != 2 {
		t.Fatalf("attempt after interrupted preflight = %d, want 2", got)
	}
}

func TestReceiptJournalIsNotImplicitlyPublishedAfterRestart(t *testing.T) {
	runtime, phase := outputBindingFixture(t)
	if err := runtime.prepare(phase, "engineering"); err != nil {
		t.Fatal(err)
	}
	runtime.recordBuild(phase, "opus", "prompt", "", nil)
	if _, err := runtime.finalize(phase, "engineering", []string{"agent", "-p", "prompt"}); err != nil {
		t.Fatal(err)
	}
	binding := runtime.pending[phase.Name].preflight.BindingSHA256
	payload := reviewerBindingPrefix + binding + "\nVERDICT: APPROVE\n"
	if err := runtime.commit(phase.Name, payload, payload, 0); err != nil {
		t.Fatal(err)
	}
	restarted := restartOutputBindingRuntime(runtime)
	if err := restarted.seedAttemptFromJournal(phase.Name); err != nil {
		t.Fatal(err)
	}
	if len(restarted.accepted) != 0 || restarted.attempts[phase.Name] != 1 {
		t.Fatalf("unreferenced journal receipt restored as accepted: %#v", restarted.accepted)
	}
}

func restartOutputBindingRuntime(runtime *outputBindingRuntime) *outputBindingRuntime {
	return newOutputBindingRuntime(
		outputBindingWorkflowInfo{root: runtime.root, runID: runtime.runID, wf: runtime.wf},
		outputBindingExecutionInfo{opts: runtime.opts, policy: runtime.policy, isClaude: runtime.isClaude},
		runtime.priorEmits, runtime.outputPaths,
	)
}

func fillBindingTestByte(value byte) func([]byte) error {
	return func(target []byte) error {
		for index := range target {
			target[index] = value
		}
		return nil
	}
}

func TestOutputBindingFinalizeRejectsPersistentPreparationDrift(t *testing.T) {
	tests := []struct {
		name  string
		drift func(*testing.T, *outputBindingRuntime)
	}{
		{name: "product source", drift: func(t *testing.T, runtime *outputBindingRuntime) {
			if err := os.WriteFile(filepath.Join(runtime.root, "product.txt"), []byte("changed"), 0o644); err != nil {
				t.Fatal(err)
			}
		}},
		{name: "native workflow", drift: func(t *testing.T, runtime *outputBindingRuntime) {
			runtime.wf.Phases[2].Readonly = false
			writeBindingWorkflow(t, runtime)
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			runtime, phase := outputBindingFixture(t)
			if err := runtime.prepare(phase, "engineering"); err != nil {
				t.Fatal(err)
			}
			runtime.recordBuild(phase, "opus", "prompt", "", nil)
			test.drift(t, runtime)
			if _, err := runtime.finalize(phase, "engineering", []string{"agent", "-p", "prompt"}); err == nil {
				t.Fatal("persistent preparation drift was accepted")
			}
			if _, present, _ := readBindingLedger(runtime); present {
				t.Fatal("preflight drift wrote a receipt")
			}
		})
	}
}

func TestOutputBindingCommitRejectsLiveWorkflowDrift(t *testing.T) {
	runtime, phase := outputBindingFixture(t)
	if err := runtime.prepare(phase, "engineering"); err != nil {
		t.Fatal(err)
	}
	runtime.recordBuild(phase, "opus", "prompt", "", nil)
	if _, err := runtime.finalize(phase, "engineering", []string{"agent", "-p", "prompt"}); err != nil {
		t.Fatal(err)
	}
	binding := runtime.pending[phase.Name].preflight.BindingSHA256
	runtime.wf.Phases[2].Readonly = false
	writeBindingWorkflow(t, runtime)
	payload := reviewerBindingPrefix + binding + "\nVERDICT: APPROVE\n"
	if err := runtime.commit(phase.Name, payload, payload, 0); err == nil ||
		!strings.Contains(err.Error(), "live native workflow") {
		t.Fatalf("workflow commit drift error = %v", err)
	}
	if _, present, _ := readBindingLedger(runtime); present {
		t.Fatal("workflow commit drift wrote a receipt")
	}
}

func TestApprovalContextCommitFailureCannotRestoreTerminalReceipt(t *testing.T) {
	runtime, phase := approvalContextFailureRuntime(t)
	if err := runtime.prepare(phase, "engineering"); err != nil {
		t.Fatal(err)
	}
	runtime.recordBuild(phase, "opus", "prompt", "", nil)
	if _, err := runtime.finalize(phase, "engineering", []string{"agent", "-p", "prompt"}); err != nil {
		t.Fatal(err)
	}
	contextPath, _ := approvalcontextstore.Path(runtime.root, "design")
	if err := statefs.EnsurePrivateDir(forgeDir(runtime.root)); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(filepath.Join(runtime.root, "outside"), contextPath); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	if err := runtime.commit(phase.Name, "done", "done", 0); err == nil {
		t.Fatal("approval context publication failure accepted terminal output")
	}
	receipts, err := outputbindingstore.New(runtime.root).Load()
	if err != nil || len(receipts) != 1 {
		t.Fatalf("context failure did not leave the intended orphan receipt: %#v, %v", receipts, err)
	}
	if err := os.Remove(contextPath); err != nil {
		t.Fatal(err)
	}
	if _, err := verifyBoundApprovalContext(runtime.root, "design"); err == nil {
		t.Fatal("orphan terminal receipt verified without its approval context")
	}
	restarted, _ := approvalContextFailureRuntimeFrom(t, runtime.root)
	if err := restarted.seedAttemptFromJournal(phase.Name); err != nil {
		t.Fatal(err)
	}
	if _, accepted := restarted.accepted[phase.Name]; accepted || restarted.attempts[phase.Name] != 1 {
		t.Fatalf("orphan restore accepted=%v attempts=%d", accepted, restarted.attempts[phase.Name])
	}
}

func approvalContextFailureRuntime(t *testing.T) (*outputBindingRuntime, asset.Phase) {
	t.Helper()
	base, _ := outputBindingFixture(t)
	return approvalContextFailureRuntimeFrom(t, base.root)
}

func approvalContextFailureRuntimeFrom(t *testing.T, root string) (*outputBindingRuntime, asset.Phase) {
	t.Helper()
	phase := asset.Phase{Name: "terminal-agent", Agent: "cto", Readonly: true}
	wf := asset.Workflow{Stage: "design", OutputBindingContract: asset.OutputBindingContractLocalDigestV1,
		Phases: []asset.Phase{phase}, Stop: asset.StopCondition{Type: "human_gate", HumanApproval: "required"}}
	opts := runOpts{root: root, mode: "engineering", lifecycle: "mvp",
		materiality: "materiality_not_bound", agentCmd: "agent"}
	runtime := newOutputBindingRuntime(
		outputBindingWorkflowInfo{root: root, runID: "run-1", wf: wf},
		outputBindingExecutionInfo{opts: opts, policy: mode.Effective("engineering", "mvp")}, priorEmitsOf(wf),
	)
	writeBindingWorkflow(t, runtime)
	return runtime, phase
}

func TestOutputBindingReadonlyAllowsOnlyExactDeclaredEmits(t *testing.T) {
	for _, test := range []struct {
		name        string
		mutateOther bool
		wantErr     bool
	}{
		{name: "declared emit", wantErr: false},
		{name: "unrelated mutation", mutateOther: true, wantErr: true},
	} {
		t.Run(test.name, func(t *testing.T) {
			runtime, _ := outputBindingFixture(t)
			phase := asset.Phase{Name: "planner", Agent: "planner", Readonly: true,
				Emits: []string{"planner.md"}}
			runtime.wf.Phases = append([]asset.Phase{phase}, runtime.wf.Phases...)
			runtime.workflowSHA = checkpointWorkflowDigest(runtime.wf)
			runtime.priorEmits = priorEmitsOf(runtime.wf)
			writeBindingWorkflow(t, runtime)
			if err := runtime.prepare(phase, "engineering"); err != nil {
				t.Fatal(err)
			}
			runtime.recordBuild(phase, "opus", "prompt", "", nil)
			if _, err := runtime.finalize(phase, "engineering", []string{"agent", "-p", "prompt"}); err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(filepath.Join(runtime.root, "planner.md"), []byte("plan"), 0o644); err != nil {
				t.Fatal(err)
			}
			if test.mutateOther {
				if err := os.WriteFile(filepath.Join(runtime.root, "product.txt"), []byte("unrelated"), 0o644); err != nil {
					t.Fatal(err)
				}
			}
			err := runtime.commit(phase.Name, "done", "done", 0)
			if test.wantErr && (err == nil || !strings.Contains(err.Error(), "outside declared emits")) {
				t.Fatalf("unrelated mutation error = %v", err)
			}
			if !test.wantErr && err != nil {
				t.Fatalf("declared emit rejected: %v", err)
			}
		})
	}
}

func TestOutputBindingReadonlyCapturesSourceExcludedReleaseEmit(t *testing.T) {
	runtime, _ := outputBindingFixture(t)
	phase := asset.Phase{Name: "planner", Agent: "planner", Readonly: true,
		Emits: []string{"docs/release/plan.md"}}
	runtime.wf.Phases = append([]asset.Phase{phase}, runtime.wf.Phases...)
	runtime.workflowSHA = checkpointWorkflowDigest(runtime.wf)
	runtime.priorEmits = priorEmitsOf(runtime.wf)
	writeBindingWorkflow(t, runtime)
	if err := runtime.prepare(phase, "engineering"); err != nil {
		t.Fatal(err)
	}
	runtime.recordBuild(phase, "opus", "prompt", "", nil)
	if _, err := runtime.finalize(phase, "engineering", []string{"agent", "-p", "prompt"}); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(runtime.root, "docs", "release"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(runtime.root, "docs", "release", "plan.md"), []byte("plan"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := runtime.commit(phase.Name, "done", "done", 0); err != nil {
		t.Fatalf("release emit rejected: %v", err)
	}
	receipt := runtime.accepted[phase.Name]
	if receipt.SourceBeforeSHA256 != receipt.SourceAfterSHA256 ||
		len(receipt.ArtifactOutputs.Items) != 1 ||
		receipt.ArtifactOutputs.Items[0].Path != "docs/release/plan.md" {
		t.Fatalf("release receipt did not separate source and artifacts: %#v", receipt)
	}
}

func TestOutputBindingMissingAcceptedPriorEmitFailsClosed(t *testing.T) {
	runtime, consumer := outputBindingFixture(t)
	producer := asset.Phase{Name: "planner", Agent: "planner", Emits: []string{"plan.md"}}
	runtime.wf.Phases = append([]asset.Phase{producer}, runtime.wf.Phases...)
	runtime.workflowSHA = checkpointWorkflowDigest(runtime.wf)
	runtime.priorEmits = priorEmitsOf(runtime.wf)
	writeBindingWorkflow(t, runtime)
	if err := runtime.prepare(producer, "engineering"); err != nil {
		t.Fatal(err)
	}
	runtime.recordBuild(producer, "opus", "prompt", "", nil)
	if _, err := runtime.finalize(producer, "engineering", []string{"agent", "-p", "prompt"}); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(runtime.root, "plan.md")
	if err := os.WriteFile(path, []byte("plan"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := runtime.commit(producer.Name, "done", "done", 0); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
	if err := runtime.prepare(consumer, "engineering"); err == nil {
		t.Fatal("downstream attempt omitted a missing current-run accepted artifact")
	}
}

func outputBindingFixture(t *testing.T) (*outputBindingRuntime, asset.Phase) {
	t.Helper()
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "product.txt"), []byte("product"), 0o644); err != nil {
		t.Fatal(err)
	}
	runBindingGit(t, root, "init", "-q")
	runBindingGit(t, root, "add", "product.txt")
	runBindingGit(t, root, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture")
	phase := asset.Phase{Name: "reviewer", Agent: "reviewer", Readonly: true, FreshContext: true,
		RequiredWhen:    "../policies/modes.yml#workflow_depth.reviewer",
		VerdictContract: asset.VerdictContractReviewerV2,
		OnFail:          &asset.OnFail{Action: "loop_back", TargetPhase: "implementer"}}
	wf := asset.Workflow{Stage: "build", OutputBindingContract: asset.OutputBindingContractLocalDigestV1,
		Phases: []asset.Phase{{Name: "implementer", Agent: "implementer"}, phase,
			{Name: "qa", Agent: "qa", Readonly: true, VerdictContract: asset.VerdictContractQAV1,
				RequiredGates: []string{"test"}, OnFail: &asset.OnFail{Action: "loop_back", TargetPhase: "implementer"}}}}
	workflowBytes, err := json.Marshal(wf)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(root, ".agent", "workflows"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, ".agent", "workflows", "build.yml"), workflowBytes, 0o644); err != nil {
		t.Fatal(err)
	}
	opts := runOpts{root: root, mode: "engineering", lifecycle: "mvp", materiality: "L3", agentCmd: "agent"}
	runtime := newOutputBindingRuntime(
		outputBindingWorkflowInfo{root: root, runID: "run-1", wf: wf},
		outputBindingExecutionInfo{opts: opts, policy: mode.Effective("engineering", "mvp")},
		priorEmitsOf(wf),
	)
	return runtime, phase
}

func writeBindingWorkflow(t *testing.T, runtime *outputBindingRuntime) {
	t.Helper()
	data, err := json.Marshal(runtime.wf)
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(runtime.root, ".agent", "workflows", runtime.wf.Stage+".yml")
	if err := os.WriteFile(path, data, 0o644); err != nil {
		t.Fatal(err)
	}
}

func runBindingGit(t *testing.T, root string, args ...string) {
	t.Helper()
	command := exec.Command("git", append([]string{"-C", root}, args...)...)
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("git %v: %v: %s", args, err, output)
	}
}

func readBindingLedger(runtime *outputBindingRuntime) ([]byte, bool, error) {
	path := outputbindingstore.Path(runtime.root)
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		return nil, false, nil
	}
	return data, err == nil, err
}
