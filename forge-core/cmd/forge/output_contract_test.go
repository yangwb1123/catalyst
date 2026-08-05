package main

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/artifact"
	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
)

func TestPhaseOutputContractValidatesPlannerAndArtifact(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, ".agent", "CURRENT_SPRINT.md")
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte("# Sprint\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	wf := asset.Workflow{Stage: "build", Phases: []asset.Phase{{
		Name: "planner", Agent: "planner", Emits: []string{".agent/CURRENT_SPRINT.md"},
	}}}
	output := `TASK_LIST:
- [ ] T001: task — acceptance: pass — files: a.go — depends_on: none — model: sonnet — roadmap: v1`
	if err := phaseOutputContract(root, wf)("planner", output); err != nil {
		t.Fatalf("valid contract: %v", err)
	}
}

func TestPhaseOutputContractRejectsMissingOrEmptyArtifact(t *testing.T) {
	root := t.TempDir()
	wf := asset.Workflow{Stage: "review", Phases: []asset.Phase{{
		Name: "security", Agent: "security-engineer", Emits: []string{"docs/review/security.md"},
	}}}
	validate := phaseOutputContract(root, wf)
	if err := validate("security", "done"); err == nil || !strings.Contains(err.Error(), "missing") {
		t.Fatalf("missing artifact error = %v", err)
	}
	path := filepath.Join(root, "docs", "review", "security.md")
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, nil, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := validate("security", "done"); err == nil || !strings.Contains(err.Error(), "empty") {
		t.Fatalf("empty artifact error = %v", err)
	}
}

func TestPhaseOutputContractCapturesRootLevelArtifact(t *testing.T) {
	root := t.TempDir()
	phase := asset.Phase{
		Name: "evaluate", Agent: "qa", Emits: []string{"eval-scorecard.md"},
	}
	wf := asset.Workflow{Stage: "evolve", Phases: []asset.Phase{phase}}
	writeContractArtifact(t, root, "eval-scorecard.md", "quality: accepted\n")
	prov := newArtifactProvenance(root, wf.Stage, "trace-run")
	prov.recordBuild(phase, "sonnet", "evaluate this run", "", nil)
	if err := phaseOutputContract(root, wf, prov)("evaluate", "evaluation"); err != nil {
		t.Fatalf("root artifact contract: %v", err)
	}
	records, err := artifact.Load(root)
	if err != nil || len(records) != 1 || records[0].Path != "eval-scorecard.md" {
		t.Fatalf("root artifact manifest = %+v, %v; want one traced record", records, err)
	}
}

func TestArtifactProvenanceFreezesBuildModelAndPromptHash(t *testing.T) {
	f := newFrozenModelFixture(t)
	_ = f.command.Build(f.phase, "balanced")
	f.command.Observe(f.phase.Name, realClaudeJSON, 0)
	if f.calls != 1 {
		t.Fatalf("tier resolver calls during Build = %d, want exactly 1", f.calls)
	}
	if f.recomputed != 0 || f.stampedModel != "sonnet" {
		t.Fatalf("cost model was recomputed after Build: calls=%d stamped=%q", f.recomputed, f.stampedModel)
	}
	if err := f.command.ValidateOutput(f.phase.Name, "done"); err != nil {
		t.Fatalf("ValidateOutput: %v", err)
	}
	records, err := artifact.Load(f.root)
	if err != nil || len(records) != 1 {
		t.Fatalf("Load = %d records, %v", len(records), err)
	}
	got := records[0]
	if got.RunID != "trace-run-123" || got.Workflow != "review" ||
		got.Agent != "reviewer" || got.Model != "sonnet" {
		t.Fatalf("frozen metadata = %+v", got)
	}
	if got.PromptSHA256 != artifact.Digest([]byte(f.hookedPrompt)) {
		t.Fatalf("prompt_sha256 = %q, want hash of exact Build hook text", got.PromptSHA256)
	}
}

type frozenModelFixture struct {
	root         string
	phase        asset.Phase
	command      orchestrator.CommandExecutor
	prov         *artifactProvenance
	hookedPrompt string
	stampedModel string
	calls        int
	recomputed   int
}

func newFrozenModelFixture(t *testing.T) *frozenModelFixture {
	t.Helper()
	root := t.TempDir()
	writeContractArtifact(t, root, "docs/review/result.md", "reviewed\n")
	phase := asset.Phase{
		Name: "review", Agent: "reviewer", Emits: []string{"docs/review/result.md"},
	}
	wf := asset.Workflow{Stage: "review", Phases: []asset.Phase{phase}}
	prov := newArtifactProvenance(root, wf.Stage, "trace-run-123")
	f := &frozenModelFixture{root: root, phase: phase, prov: prov}
	hooks := executorHooks{
		ValidateOutput: phaseOutputContract(root, wf, prov),
		OnBuild:        f.onBuild,
		ModelFor:       prov.modelFor,
	}
	ex := agentExecutor(runOpts{
		executor: "command", agentCmd: "claude", root: root,
	}, func(string) {}, f.costSink, f.tierOf, f.recomputeModel,
		nil, nil, nil, nil, nil, nil, nil, nil, hooks)
	f.command = ex.(orchestrator.CommandExecutor)
	return f
}

func (f *frozenModelFixture) tierOf(asset.Phase) string {
	f.calls++
	if f.calls == 1 {
		return "sonnet"
	}
	return "haiku"
}

func (f *frozenModelFixture) recomputeModel(string) string {
	f.recomputed++
	return "haiku"
}

func (f *frozenModelFixture) costSink(_, model string, _ float64, _ time.Duration) {
	f.stampedModel = model
}

func (f *frozenModelFixture) onBuild(p asset.Phase, model, promptText, _ string, _ map[string]string) {
	f.hookedPrompt = promptText
	f.prov.recordBuild(p, model, promptText, "", nil)
}

func TestArtifactProvenanceRecordsEveryLoopbackProduction(t *testing.T) {
	root := t.TempDir()
	const emitted = "docs/review/result.md"
	writeContractArtifact(t, root, emitted, "attempt one\n")
	phase := asset.Phase{Name: "review", Agent: "reviewer", Emits: []string{emitted}}
	wf := asset.Workflow{Stage: "review", Phases: []asset.Phase{phase}}
	prov := newArtifactProvenance(root, wf.Stage, "trace-loop")
	validate := phaseOutputContract(root, wf, prov)

	prov.recordBuild(phase, "sonnet", "prompt one", "", nil)
	if err := validate(phase.Name, "first"); err != nil {
		t.Fatalf("first validation: %v", err)
	}
	writeContractArtifact(t, root, emitted, "attempt two\n")
	prov.recordBuild(phase, "opus", "prompt two", "", nil)
	if err := validate(phase.Name, "second"); err != nil {
		t.Fatalf("second validation: %v", err)
	}

	records, err := artifact.Load(root)
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	got := artifact.Query(records, artifact.Filter{RunID: "trace-loop", Phase: "review"})
	if len(got) != 2 {
		t.Fatalf("loop-back records = %d, want 2", len(got))
	}
	if got[0].SHA256 == got[1].SHA256 || got[0].PromptSHA256 == got[1].PromptSHA256 {
		t.Fatalf("loop-back attempts were collapsed: %+v", got)
	}
	if got[0].Model != "sonnet" || got[1].Model != "opus" {
		t.Fatalf("models = %q, %q", got[0].Model, got[1].Model)
	}
	raw, err := os.ReadFile(filepath.Join(root, ".forge", "artifacts.jsonl"))
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(raw), "prompt one") || strings.Contains(string(raw), "prompt two") {
		t.Fatal("manifest must retain prompt hashes, never raw prompt text")
	}
}

func TestBuildRunEngineThreadsTraceRunIDIntoManifest(t *testing.T) {
	root := t.TempDir()
	writeContractArtifact(t, root, "docs/result.md", "result\n")
	phase := asset.Phase{
		Name: "implementer", Agent: "implementer", Emits: []string{"docs/result.md"},
	}
	wf := asset.Workflow{Stage: "build", Phases: []asset.Phase{phase}}
	o := runOpts{root: root, mode: "balanced", executor: "command", agentCmd: "true"}
	eng, _, _ := buildRunEngine(
		wf, o, func(string) {}, nil,
		func(string) gate.Result { return gate.Result{OK: true} },
		mode.Policy{}, &runBudget{}, "", nil, nil, nil, "trace-runtime-id",
	)
	if err := eng.Exec.Execute(context.Background(), phase, "balanced"); err != nil {
		t.Fatalf("Execute: %v", err)
	}
	records, err := artifact.Load(root)
	if err != nil || len(records) != 1 {
		t.Fatalf("Load = %d records, %v", len(records), err)
	}
	if records[0].RunID != "trace-runtime-id" {
		t.Fatalf("run_id = %q, want tracer id", records[0].RunID)
	}
}

func TestArtifactManifestAppendFailureFailsOutputContract(t *testing.T) {
	root := t.TempDir()
	writeContractArtifact(t, root, "docs/result.md", "valid artifact\n")
	if err := os.WriteFile(filepath.Join(root, ".forge"), []byte("collision"), 0o600); err != nil {
		t.Fatal(err)
	}
	phase := asset.Phase{
		Name: "implementer", Agent: "implementer", Emits: []string{"docs/result.md"},
	}
	wf := asset.Workflow{Stage: "build", Phases: []asset.Phase{phase}}
	prov := newArtifactProvenance(root, wf.Stage, "trace-run")
	hooks := executorHooks{
		ValidateOutput: phaseOutputContract(root, wf, prov),
		OnBuild:        prov.recordBuild,
	}
	ex := agentExecutor(runOpts{
		executor: "command", agentCmd: "true", root: root,
	}, func(string) {}, nil, unbudgetedTier("balanced"), nil, nil, nil, nil, nil, nil, nil, nil, nil, hooks)
	err := ex.Execute(context.Background(), phase, "balanced")
	var execErr *orchestrator.ExecError
	if err == nil || !errors.As(err, &execErr) || execErr.Kind != orchestrator.KindFailed ||
		!strings.Contains(err.Error(), "artifact provenance") {
		t.Fatalf("append failure error = %v", err)
	}
}

func writeContractArtifact(t *testing.T, root, rel, content string) {
	t.Helper()
	path := filepath.Join(root, filepath.FromSlash(rel))
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}
