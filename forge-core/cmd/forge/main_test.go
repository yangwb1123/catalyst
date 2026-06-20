package main

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/memory"
	"forgeos/forge-core/internal/orchestrator"
)

// buildPrompt must embed the role, phase, routed tier, and the agent's card so
// a real `claude -p` invocation has the full instruction. reviewer floors to
// opus regardless of mode.
func TestBuildPrompt_EmbedsRolePhaseTier(t *testing.T) {
	p := asset.Phase{Name: "reviewer", Agent: "reviewer"}
	got := buildPrompt("/home/u1/catalyst", p, "balanced")
	for _, want := range []string{`"reviewer" agent`, "phase=reviewer", "tier=opus"} {
		if !strings.Contains(got, want) {
			t.Errorf("prompt missing %q", want)
		}
	}
}

// A missing card must not break prompt assembly — it degrades to a marker.
func TestBuildPrompt_MissingCardDegrades(t *testing.T) {
	p := asset.Phase{Name: "ghost", Agent: "no-such-agent"}
	got := buildPrompt("/home/u1/catalyst", p, "balanced")
	if !strings.Contains(got, "no role card found") {
		t.Errorf("expected missing-card marker; got: %.80s", got)
	}
}

// Backward-compat: the Context Engine upgrade must not drop the hard-constraint
// injection. A prompt built from the REAL repo must still carry the leading
// AGENTS.md constraints (the 500-line cap), exactly as before retrieval+memory.
func TestBuildPrompt_StillInjectsHardConstraints(t *testing.T) {
	got := buildPrompt("/home/u1/catalyst", asset.Phase{Name: "reviewer", Agent: "reviewer"}, "balanced")
	if !strings.Contains(got, "Engineering constraints") || !strings.Contains(got, "500") {
		t.Errorf("hard constraints must still inject after the Context Engine upgrade; got: %.400s", got)
	}
}

// memoryContext: a prompt built in a repo with a seeded memory store must surface
// the recorded gaps/decisions/lessons, so a real agent sees what prior iterations
// learned instead of rediscovering it.
func TestBuildPrompt_IncludesMemoryEntries(t *testing.T) {
	root := t.TempDir()
	seed := []memory.Entry{
		{Kind: memory.KindGap, Topic: "build", Detail: "missing retry on flaky gate", Iteration: 1, CreatedAtUnix: 1},
		{Kind: memory.KindDecision, Topic: "build", Detail: "chose JSONL for the memory store", Iteration: 2, CreatedAtUnix: 2},
	}
	for _, e := range seed {
		if err := memory.Append(memoryPath(root), e); err != nil {
			t.Fatalf("seed memory: %v", err)
		}
	}
	got := buildPrompt(root, asset.Phase{Name: "build", Agent: "implementer"}, "balanced")
	if !strings.Contains(got, "Project memory") {
		t.Errorf("prompt must carry a Project memory block; got: %.400s", got)
	}
	for _, want := range []string{"missing retry on flaky gate", "chose JSONL for the memory store"} {
		if !strings.Contains(got, want) {
			t.Errorf("prompt missing memory detail %q; got: %.500s", want, got)
		}
	}
}

// Cold start: a repo with NO memory store must build a prompt without error and
// without a memory block — absence is the normal first-run case, never a failure.
func TestBuildPrompt_MissingMemoryIsColdStart(t *testing.T) {
	root := t.TempDir() // no .forge/memory.jsonl
	got := buildPrompt(root, asset.Phase{Name: "build", Agent: "implementer"}, "balanced")
	if strings.Contains(got, "Project memory") {
		t.Errorf("cold start must omit the memory block; got: %.300s", got)
	}
	if strings.Contains(got, "UNREADABLE") {
		t.Errorf("a missing store must not be reported as unreadable; got: %.300s", got)
	}
}

func TestRun_NoArgsIsUsageError(t *testing.T) {
	if code := run(nil); code != 2 {
		t.Errorf("run(nil) = %d, want 2", code)
	}
}

// repoRoot finds the ForgeOS repo root (the dir holding harness/yaml2json.py),
// or "" when the test is not running inside the repo.
func repoRoot() string {
	dir, err := os.Getwd()
	if err != nil {
		return ""
	}
	for {
		if _, err := os.Stat(filepath.Join(dir, "harness", "yaml2json.py")); err == nil {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			return ""
		}
		dir = parent
	}
}

// agentExecutor must give a claude-family agent --permission-mode (so it can write
// files headlessly under --executor=command) and must NOT pass that claude-only
// flag to a stub like echo. This is what made real ignition actually WRITE code.
func TestAgentExecutor_PermissionModeOnlyForClaude(t *testing.T) {
	mk := func(cmd string) string {
		ex := agentExecutor(runOpts{executor: "command", agentCmd: cmd, agentPermission: "acceptEdits", root: t.TempDir()}, func(string) {}, nil)
		ce, ok := ex.(orchestrator.CommandExecutor)
		if !ok {
			t.Fatalf("executor=command must yield a CommandExecutor, got %T", ex)
		}
		return strings.Join(ce.Build(asset.Phase{Name: "implementer", Agent: "implementer"}, "balanced"), " ")
	}
	if !strings.Contains(mk("claude"), "--permission-mode acceptEdits") {
		t.Error("claude must carry --permission-mode acceptEdits to write files headlessly")
	}
	if strings.Contains(mk("echo"), "--permission-mode") {
		t.Error("echo (a stub) must NOT receive the claude-only flag")
	}
}

// An empty agent-permission disables the flag even for claude (operator opt-out).
func TestAgentExecutor_EmptyPermissionDisablesFlag(t *testing.T) {
	ex := agentExecutor(runOpts{executor: "command", agentCmd: "claude", agentPermission: "", root: t.TempDir()}, func(string) {}, nil)
	ce := ex.(orchestrator.CommandExecutor)
	argv := strings.Join(ce.Build(asset.Phase{Name: "p", Agent: "implementer"}, "balanced"), " ")
	if strings.Contains(argv, "--permission-mode") {
		t.Errorf("empty agent-permission must omit the flag; got: %s", argv)
	}
}

// agentExecutor must pass --model <routed-tier> to a claude-family command so a
// real run honors ForgeOS's routing (the opus floor for reviewer/architect/cto),
// and must NOT pass that claude-only flag to a stub like echo.
func TestAgentExecutor_ModelTierForClaude(t *testing.T) {
	mk := func(cmd, agent string) string {
		ex := agentExecutor(runOpts{executor: "command", agentCmd: cmd, root: t.TempDir()}, func(string) {}, nil)
		ce := ex.(orchestrator.CommandExecutor)
		return strings.Join(ce.Build(asset.Phase{Name: agent, Agent: agent}, "balanced"), " ")
	}
	if got := mk("claude", "reviewer"); !strings.Contains(got, "--model opus") {
		t.Errorf("claude reviewer must route to --model opus (the safety floor); got: %s", got)
	}
	if strings.Contains(mk("echo", "reviewer"), "--model") {
		t.Error("echo (a stub) must NOT receive the claude-only --model flag")
	}
}

// agentExecutor passes claude's --max-budget-usd only when set, and only to a
// claude-family command — the per-phase dollar ceiling that complements
// --max-agent-calls (phase count) and --timeout (wall-clock).
func TestAgentExecutor_MaxBudgetForClaude(t *testing.T) {
	mk := func(cmd, budget string) string {
		ex := agentExecutor(runOpts{executor: "command", agentCmd: cmd, agentMaxBudgetUSD: budget, root: t.TempDir()}, func(string) {}, nil)
		ce := ex.(orchestrator.CommandExecutor)
		return strings.Join(ce.Build(asset.Phase{Name: "p", Agent: "implementer"}, "balanced"), " ")
	}
	if got := mk("claude", "0.50"); !strings.Contains(got, "--max-budget-usd 0.50") {
		t.Errorf("claude with a budget must pass --max-budget-usd; got: %s", got)
	}
	if strings.Contains(mk("claude", ""), "--max-budget-usd") {
		t.Error("empty budget must omit --max-budget-usd")
	}
	if strings.Contains(mk("echo", "0.50"), "--max-budget-usd") {
		t.Error("echo (a stub) must NOT receive the claude-only --max-budget-usd")
	}
}

// End to end: load the REAL build.yml via the yaml2json shim + asset loader and
// assert the typed criteria evaluate per-criterion as expected. build.yml's
// all_of items are objects ({metric, operator, threshold/value}), so this proves
// the typed UnmarshalJSON + converge dispatch works on the production asset.
// Skips when python3 is unavailable or not inside the repo.
func TestEndToEnd_BuildYmlCriteria(t *testing.T) {
	if _, err := exec.LookPath("python3"); err != nil {
		t.Skip("python3 not available")
	}
	root := repoRoot()
	if root == "" {
		t.Skip("not running inside the ForgeOS repo (no harness/yaml2json.py)")
	}
	wf, err := loadWorkflow(root, "build")
	if err != nil {
		t.Fatalf("load build.yml: %v", err)
	}
	if wf.Stop.Type != "conjunction" {
		t.Fatalf("build.yml stop type = %q, want conjunction", wf.Stop.Type)
	}
	if len(wf.Stop.AllOf) != 2 {
		t.Fatalf("build.yml all_of = %d criteria, want 2 (objects)", len(wf.Stop.AllOf))
	}
	// They must be parsed as typed objects, not bare strings.
	if wf.Stop.AllOf[0].Metric != "roadmap_completion" || wf.Stop.AllOf[0].Raw != "" {
		t.Errorf("criterion[0] = %+v, want typed roadmap_completion object", wf.Stop.AllOf[0])
	}
	if wf.Stop.AllOf[1].Metric != "gates_status" || wf.Stop.AllOf[1].Value != "green" {
		t.Errorf("criterion[1] = %+v, want gates_status==green", wf.Stop.AllOf[1])
	}

	// Fully met: 100% roadmap + green gates => all criteria met.
	met, allMet := converge.Evaluate(wf.Stop.AllOf, converge.Signals{RoadmapCompletion: 1.0, GatesGreen: true})
	if !allMet || !met[0].Met || !met[1].Met {
		t.Errorf("100%%+green should meet every criterion; got %+v", met)
	}
	// Partial roadmap, green gates => roadmap unmet, gate met, not converged.
	mixed, conv := converge.Evaluate(wf.Stop.AllOf, converge.Signals{RoadmapCompletion: 0.5, GatesGreen: true})
	if conv || mixed[0].Met || !mixed[1].Met {
		t.Errorf("50%%+green: roadmap unmet & gate met & not converged; got %+v", mixed)
	}
	// 100% roadmap, red gates => roadmap met, gate unmet, not converged.
	red, conv2 := converge.Evaluate(wf.Stop.AllOf, converge.Signals{RoadmapCompletion: 1.0, GatesGreen: false})
	if conv2 || !red[0].Met || red[1].Met {
		t.Errorf("100%%+red: roadmap met & gate unmet & not converged; got %+v", red)
	}
}

// --max-retries must parse on both run and evolve (IntVar): a valid value is
// accepted and the command proceeds to a clean stop; a non-integer is a parse
// error (exit 2), not silently ignored. The default (omitted) is 0 == no retries.
func TestMaxRetriesFlagParses(t *testing.T) {
	if _, err := exec.LookPath("python3"); err != nil {
		t.Skip("python3 not available")
	}
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	// Valid value on evolve: a clean external-stop loop (dry executor never errors).
	if code := cmdEvolve([]string{"evolve", "--root", root, "--max-retries", "2", "--max-iter", "1"}); code != 0 {
		t.Errorf("evolve --max-retries 2 should run to a clean stop; exit=%d", code)
	}
	// Valid value on run: a single agent phase, dry executor, exits 0.
	runRoot := fakeRepo(t, "build", externalAgentWorkflow)
	if code := cmdRun([]string{"build", "--root", runRoot, "--max-retries", "3"}); code != 0 {
		t.Errorf("run --max-retries 3 should complete cleanly; exit=%d", code)
	}
	// A non-integer must be rejected at flag parse (exit 2), never ignored.
	if code := cmdEvolve([]string{"evolve", "--root", root, "--max-retries", "notanint"}); code != 2 {
		t.Errorf("malformed --max-retries must be a parse error; exit=%d, want 2", code)
	}
}

// --- test helpers ------------------------------------------------------------

func mkdir(t *testing.T, dir string) {
	t.Helper()
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatalf("mkdir %s: %v", dir, err)
	}
}

func writeFile(t *testing.T, path, content string) {
	t.Helper()
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatalf("write %s: %v", path, err)
	}
}
