package main

import (
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

func TestChainResumesWaitingGateWithoutRerunningCompletedWork(t *testing.T) {
	root := t.TempDir()
	first := humanChainWorkflow("design", "review", "design-agent")
	writeChainAsset(t, root, "design", first)
	writeChainAsset(t, root, "review", humanChainWorkflow("review", "", "review-agent"))
	o := chainOpts(root)
	o.approved, o.maxAgentCalls, o.maxChainStages = true, 5, 5
	o.mode, o.lifecycle = "engineering", "production"
	o.runBudgetUSD = "1.50"

	code, out := captureChainOutput(t, func() int {
		return execEngine(context.Background(), first, o)
	})
	if code != exitChainIncomplete {
		t.Fatalf("first exit=%d output=%s", code, out)
	}
	waiting := mustChainState(t, root)
	if waiting.RunID == "" || waiting.AgentCalls != 2 ||
		waiting.BudgetCapMicros != 1_500_000 || waiting.MaxAgentCalls != 5 {
		t.Fatalf("waiting state lost shared resources: %+v", waiting)
	}
	waiting.SpentUsdMicros = 700_000
	if err := saveChainState(root, waiting); err != nil {
		t.Fatal(err)
	}
	approveChainStage(t, root, "review")

	resumeOpts := chainOpts(root) // default mode/limit and empty lifecycle restore persisted policy
	resumeOpts.lifecycle = ""
	code, out = captureChainOutput(t, func() int {
		return execEngine(context.Background(), first, resumeOpts)
	})
	if code != 0 {
		t.Fatalf("resume exit=%d output=%s", code, out)
	}
	for _, phase := range []string{"design-agent", "review-agent"} {
		if strings.Contains(out, "phase "+phase) {
			t.Errorf("resume reran completed phase %s:\n%s", phase, out)
		}
	}
	done := mustChainState(t, root)
	if done.RunID != waiting.RunID || done.AgentCalls != waiting.AgentCalls ||
		done.BudgetCapMicros != waiting.BudgetCapMicros ||
		done.SpentUsdMicros != waiting.SpentUsdMicros ||
		done.Mode != "engineering" || done.Lifecycle != "production" ||
		done.MaxChainStages != 5 ||
		strings.Join(done.CompletedStages, ",") != "design,review" {
		t.Fatalf("resumed state drifted: before=%+v after=%+v", waiting, done)
	}
}

func TestChainResumeRestoresCTOPolicyBeforeBuild(t *testing.T) {
	root := t.TempDir()
	first := humanChainWorkflow("design", "review")
	writeChainAsset(t, root, "design", first)
	writeChainAsset(t, root, "review", humanChainWorkflow("review", "build"))
	o := chainOpts(root)
	o.approved, o.mode = true, "cto"
	if code, out := captureChainOutput(t, func() int {
		return execEngine(context.Background(), first, o)
	}); code != exitChainIncomplete {
		t.Fatalf("first exit=%d output=%s", code, out)
	}
	captureStdout(t, func() {
		if code := writeApproval(root, "review", true); code != 0 {
			t.Fatalf("approve review = %d", code)
		}
	})
	resume := chainOpts(root) // balanced is the default-as-omitted sentinel
	code, out := captureChainOutput(t, func() int {
		return execEngine(context.Background(), first, resume)
	})
	if code != 0 || !strings.Contains(out, "build=halt") {
		t.Fatalf("resume exit=%d output=%s, want restored CTO halt", code, out)
	}
	state := mustChainState(t, root)
	if state.Status != "halted" || state.CurrentStage != "build" ||
		state.Mode != "cto" || state.Lifecycle != "idea" {
		t.Fatalf("restored CTO state = %+v", state)
	}
}

func TestChainResumeRestoresProductionGatePolicy(t *testing.T) {
	root := t.TempDir()
	first := humanChainWorkflow("design", "review")
	writeChainAsset(t, root, "design", first)
	writeChainAsset(t, root, "review", humanChainWorkflow("review", "build", "review-agent"))
	writeChainAsset(t, root, "build", humanChainWorkflow("build", "", "build-agent"))
	o := chainOpts(root)
	o.approved, o.mode, o.lifecycle = true, "explorer", "production"
	if code, out := captureChainOutput(t, func() int {
		return execEngine(context.Background(), first, o)
	}); code != exitChainIncomplete {
		t.Fatalf("first exit=%d output=%s", code, out)
	}
	for _, stage := range []string{"review", "build"} {
		captureStdout(t, func() {
			if code := writeApproval(root, stage, true); code != 0 {
				t.Fatalf("approve %s = %d", stage, code)
			}
		})
	}
	resume := chainOpts(root)
	resume.lifecycle = ""
	code, out := captureChainOutput(t, func() int {
		return execEngine(context.Background(), first, resume)
	})
	if code != 0 {
		t.Fatalf("resume exit=%d output=%s", code, out)
	}
	for _, want := range []string{
		"stage=build mode=explorer lifecycle=production",
		"gates=[lint build test complexity arch security] reviewer=true",
	} {
		if !strings.Contains(out, want) {
			t.Fatalf("restored production policy missing %q: %s", want, out)
		}
	}
	state := mustChainState(t, root)
	if state.Mode != "explorer" || state.Lifecycle != "production" {
		t.Fatalf("restored production state = %+v", state)
	}
}

func TestChainResumeRejectsPolicyAndStageLimitConflicts(t *testing.T) {
	tests := []struct {
		name string
		edit func(*runOpts)
		want string
	}{
		{"mode downgrade", func(o *runOpts) { o.mode = "explorer" }, "--mode=engineering"},
		{"lifecycle downgrade", func(o *runOpts) { o.lifecycle = "idea" }, "--lifecycle=production"},
		{"stage limit change", func(o *runOpts) { o.maxChainStages = 6 }, "--max-chain-stages=5"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			root := t.TempDir()
			entry := humanChainWorkflow("design", "review")
			current := humanChainWorkflow("review", "build", "review-agent")
			writeChainAsset(t, root, "design", entry)
			writeChainAsset(t, root, "review", current)
			state := chainState{
				RunID: "same-run", Status: "waiting_approval",
				EntryStage: "design", CurrentStage: "review",
				CompletedStages: []string{"design"},
				Mode:            "engineering", Lifecycle: "production",
				MaxChainStages: 5,
			}
			if err := saveChainState(root, state); err != nil {
				t.Fatal(err)
			}
			o := chainOpts(root)
			o.mode, o.lifecycle, o.maxChainStages = "engineering", "production", 5
			tc.edit(&o)
			code, out := captureChainOutput(t, func() int {
				return execEngine(context.Background(), current, o)
			})
			if code != 1 || !strings.Contains(out, tc.want) {
				t.Fatalf("conflict exit=%d output=%s, want %q", code, out, tc.want)
			}
			if strings.Contains(out, "phase review-agent") {
				t.Fatalf("conflicting resume executed weaker-policy work: %s", out)
			}
		})
	}
}

func TestChainResumeRestoresAgentCallCeiling(t *testing.T) {
	root := t.TempDir()
	first := humanChainWorkflow("design", "review", "design-agent")
	writeChainAsset(t, root, "design", first)
	writeChainAsset(t, root, "review", humanChainWorkflow("review", "build", "review-agent"))
	writeChainAsset(t, root, "build", humanChainWorkflow("build", "", "build-agent"))
	approveChainStage(t, root, "build")
	o := chainOpts(root)
	o.approved, o.maxAgentCalls = true, 2
	if code, out := captureChainOutput(t, func() int {
		return execEngine(context.Background(), first, o)
	}); code != exitChainIncomplete {
		t.Fatalf("first exit=%d output=%s", code, out)
	}
	captureStdout(t, func() {
		if code := writeApproval(root, "review", true); code != 0 {
			t.Fatalf("approve review = %d", code)
		}
	})

	code, out := captureChainOutput(t, func() int {
		return execEngine(context.Background(), first, chainOpts(root))
	})
	if code != 1 || !strings.Contains(out, "execution 3 after 2 completed") {
		t.Fatalf("restored cap exit=%d output=%s", code, out)
	}
	state := mustChainState(t, root)
	if state.AgentCalls != 2 {
		t.Fatalf("refused N+1 call changed actual count: %+v", state)
	}
}

func TestChainResumePersistenceFailureKeepsDurableCursor(t *testing.T) {
	root := t.TempDir()
	entry := humanChainWorkflow("design", "review", "design-agent")
	current := humanChainWorkflow("review", "", "review-agent")
	writeChainAsset(t, root, "design", entry)
	writeChainAsset(t, root, "review", current)
	waiting := chainState{
		RunID: "durable-run", Status: "waiting_approval",
		EntryStage: "design", CurrentStage: "review",
		CompletedStages: []string{"design"},
		Mode:            "balanced", Lifecycle: "idea", MaxChainStages: defaultMaxChainStages,
	}
	if err := saveChainState(root, waiting); err != nil {
		t.Fatal(err)
	}
	approveChainStage(t, root, "review")

	attempts, restoreSaver := injectChainPersistenceFailure(t)

	code, out := captureChainOutput(t, func() int {
		return execEngine(context.Background(), entry, chainOpts(root))
	})
	if code != 1 || !strings.Contains(out, "injected chain-state commit failure") {
		t.Fatalf("injected persistence failure exit=%d output=%s", code, out)
	}
	if *attempts != 1 {
		t.Fatalf("runtime save attempts = %d, want one decision-after-check commit", *attempts)
	}
	assertWaitingCursorUnchanged(t, root, waiting)

	restoreSaver()
	code, out = captureChainOutput(t, func() int {
		return execEngine(context.Background(), entry, chainOpts(root))
	})
	if code != 0 {
		t.Fatalf("restart after persistence failure exit=%d output=%s", code, out)
	}
	for _, phase := range []string{"design-agent", "review-agent"} {
		if strings.Contains(out, "phase "+phase) {
			t.Fatalf("restart reran completed/held-gate work %q: %s", phase, out)
		}
	}
	done := mustChainState(t, root)
	if done.RunID != waiting.RunID || done.Status != "completed" ||
		strings.Join(done.CompletedStages, ",") != "design,review" {
		t.Fatalf("recovered cursor = %+v", done)
	}
}

func injectChainPersistenceFailure(t *testing.T) (*int, func()) {
	t.Helper()
	original := persistChainState
	attempts := 0
	persistChainState = func(string, chainState) error {
		attempts++
		return errors.New("injected chain-state commit failure")
	}
	restore := func() { persistChainState = original }
	t.Cleanup(restore)
	return &attempts, restore
}

func assertWaitingCursorUnchanged(t *testing.T, root string, want chainState) {
	t.Helper()
	got := mustChainState(t, root)
	if got.Status != "waiting_approval" || got.CurrentStage != "review" ||
		got.RunID != want.RunID || strings.Join(got.CompletedStages, ",") != "design" {
		t.Fatalf("failed commit destroyed durable cursor: before=%+v after=%+v", want, got)
	}
}

func TestPrepareChainResumeRejectsNonHumanCurrentWorkflow(t *testing.T) {
	root := t.TempDir()
	entry := humanChainWorkflow("build", "evolve")
	writeChainAsset(t, root, "build", entry)
	writeChainAsset(t, root, "evolve", externalChainWorkflow())
	state := resumableTestState("build", "evolve", []string{"build"})
	if err := saveChainState(root, state); err != nil {
		t.Fatal(err)
	}
	if _, _, err := prepareChainResume(entry, chainOpts(root)); err == nil ||
		!strings.Contains(err.Error(), "want human_gate") {
		t.Fatalf("non-human current workflow error = %v", err)
	}
}

func TestPrepareChainResumeRejectsCorruptOrSkippedPath(t *testing.T) {
	tests := []struct {
		name       string
		completed  []string
		reviewNext string
		want       string
	}{
		{
			name: "missing completed prefix", completed: []string{"design"},
			reviewNext: "build", want: "exact path prefix [design review]",
		},
		{
			name: "jumped current into prefix", completed: []string{"design", "build"},
			reviewNext: "build", want: "exact path prefix [design review]",
		},
		{
			name: "unreachable current", completed: []string{"design", "review"},
			reviewNext: "", want: "declares no next_stage",
		},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			root := t.TempDir()
			entry := humanChainWorkflow("design", "review")
			writeChainAsset(t, root, "design", entry)
			writeChainAsset(t, root, "review", humanChainWorkflow("review", tc.reviewNext))
			writeChainAsset(t, root, "build", humanChainWorkflow("build", ""))
			state := resumableTestState("design", "build", tc.completed)
			if err := saveChainState(root, state); err != nil {
				t.Fatal(err)
			}
			if _, _, err := prepareChainResume(entry, chainOpts(root)); err == nil ||
				!strings.Contains(err.Error(), tc.want) {
				t.Fatalf("corrupt path error = %v, want %q", err, tc.want)
			}
		})
	}
}

func TestPrepareChainResumeRejectsDeclaredPathCycle(t *testing.T) {
	root := t.TempDir()
	entry := humanChainWorkflow("design", "review")
	writeChainAsset(t, root, "design", entry)
	writeChainAsset(t, root, "review", humanChainWorkflow("review", "design"))
	state := resumableTestState("design", "build", []string{"design", "review"})
	if err := saveChainState(root, state); err != nil {
		t.Fatal(err)
	}
	if _, _, err := prepareChainResume(entry, chainOpts(root)); err == nil ||
		!strings.Contains(err.Error(), "cycle") {
		t.Fatalf("cyclic path error = %v", err)
	}
}

func TestPrepareChainResumeAcceptsStandaloneRollbackPrefix(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(repoRoot(), ".agent", "workflows", "rollback.yml")
	data, err := os.ReadFile(source)
	if err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(root, ".agent", "workflows", "rollback.yml")
	if err := os.MkdirAll(filepath.Dir(target), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(target, data, 0o600); err != nil {
		t.Fatal(err)
	}
	rollback, err := loadWorkflow(root, "rollback")
	if err != nil {
		t.Fatal(err)
	}
	state := resumableTestState("rollback", "rollback", nil)
	if err := saveChainState(root, state); err != nil {
		t.Fatal(err)
	}
	current, resumed, err := prepareChainResume(rollback, chainOpts(root))
	if err != nil {
		t.Fatalf("standalone rollback rejected: %v", err)
	}
	if resumed == nil || current.Stage != "rollback" || len(resumed.CompletedStages) != 0 {
		t.Fatalf("standalone rollback resume = current:%+v state:%+v", current, resumed)
	}
}

func TestRestrictedChainResumeEntryLoadDoesNotExecuteRepositoryShim(t *testing.T) {
	root := restrictedDeployResumeRepo(t)
	sentinel := installResumeWorkflowShim(t, root, "discover",
		humanChainWorkflow("discover", "design"))
	code, out := captureChainOutput(t, func() int {
		return cmdRun([]string{"discover", "--root", root, "--chain"})
	})
	assertRestrictedResumeShimRejected(t, code, out, sentinel)
}

func TestRestrictedChainResumeRebuildDoesNotExecuteRepositoryShim(t *testing.T) {
	root := restrictedDeployResumeRepo(t)
	sentinel := installResumeWorkflowShim(t, root, "design",
		humanChainWorkflow("design", "review"))
	code, out := captureChainOutput(t, func() int {
		return cmdRun([]string{"deploy", "--root", root, "--chain"})
	})
	assertRestrictedResumeShimRejected(t, code, out, sentinel)
}

func restrictedDeployResumeRepo(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	for _, stage := range []struct{ name, next string }{
		{"discover", "design"}, {"design", "review"}, {"review", "build"},
		{"build", "deploy"},
	} {
		writeChainAsset(t, root, stage.name, humanChainWorkflow(stage.name, stage.next))
	}
	source := filepath.Join(repoRoot(), ".agent", "workflows", "deploy.yml")
	data, err := os.ReadFile(source)
	if err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(root, ".agent", "workflows", "deploy.yml")
	if err := os.WriteFile(target, data, 0o600); err != nil {
		t.Fatal(err)
	}
	state := resumableTestState(
		"discover", "deploy", []string{"discover", "design", "review", "build"},
	)
	if err := saveChainState(root, state); err != nil {
		t.Fatal(err)
	}
	return root
}

func installResumeWorkflowShim(t *testing.T, root, stage string, fallback asset.Workflow) string {
	t.Helper()
	data, err := json.Marshal(fallback)
	if err != nil {
		t.Fatal(err)
	}
	sentinel := filepath.Join(root, stage+"-shim-ran")
	writeFile(t, filepath.Join(root, ".agent", "workflows", stage+".yml"), "stub: true\n")
	if err := os.MkdirAll(filepath.Join(root, "harness"), 0o700); err != nil {
		t.Fatal(err)
	}
	shim := "from pathlib import Path\n" +
		"Path(" + pyQuote(sentinel) + ").write_text('ran')\n" +
		"print(" + pyQuote(string(data)) + ")\n"
	writeFile(t, filepath.Join(root, "harness", "yaml2json.py"), shim)
	return sentinel
}

func assertRestrictedResumeShimRejected(t *testing.T, code int, output, sentinel string) {
	t.Helper()
	if code != 1 || !strings.Contains(output, "fallback is disabled for restricted execution") {
		t.Fatalf("restricted resume exit=%d output=%s", code, output)
	}
	if _, err := os.Stat(sentinel); !os.IsNotExist(err) {
		t.Fatalf("restricted resume executed repository shim %q: %v", sentinel, err)
	}
}

func resumableTestState(entry, current string, completed []string) chainState {
	return chainState{
		RunID: "resume-test", Status: "waiting_approval",
		EntryStage: entry, CurrentStage: current,
		CompletedStages: append([]string(nil), completed...),
		Mode:            "balanced", Lifecycle: "idea", MaxChainStages: defaultMaxChainStages,
	}
}

func TestChainStateV1CannotResume(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(forgeDir(root), 0o755); err != nil {
		t.Fatal(err)
	}
	legacy := `{"_format":"forgeos.chain-state.v1","status":"waiting_approval","current_stage":"review"}`
	if err := os.WriteFile(chainStatePath(root), []byte(legacy), 0o600); err != nil {
		t.Fatal(err)
	}
	first := humanChainWorkflow("review", "")
	if code, out := captureChainOutput(t, func() int {
		return execEngine(context.Background(), first, chainOpts(root))
	}); code != 1 || !strings.Contains(out, "unsupported chain state format") {
		t.Fatalf("legacy resume exit=%d output=%s", code, out)
	}
}
