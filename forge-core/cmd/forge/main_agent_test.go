package main

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/orchestrator"
)

// This file holds the CLI-dispatch, agentExecutor-argv, and end-to-end tests split
// out of main_test.go (which keeps the buildPrompt/feed-forward tests) purely to stay
// under the harness's per-file line budget — same package, no behavior change.

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
		ex := agentExecutor(runOpts{executor: "command", agentCmd: cmd, agentPermission: "acceptEdits", root: t.TempDir()}, func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil, nil, nil, nil, nil, nil)
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
	ex := agentExecutor(runOpts{executor: "command", agentCmd: "claude", agentPermission: "", root: t.TempDir()}, func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil, nil, nil, nil, nil, nil)
	ce := ex.(orchestrator.CommandExecutor)
	argv := strings.Join(ce.Build(asset.Phase{Name: "p", Agent: "implementer"}, "balanced"), " ")
	if strings.Contains(argv, "--permission-mode") {
		t.Errorf("empty agent-permission must omit the flag; got: %s", argv)
	}
}

// buildAgentArgv builds the argv for an agent phase via the installed CommandExecutor —
// the shared helper for the --allowedTools assertions below.
func buildAgentArgv(t *testing.T, o runOpts) string {
	t.Helper()
	o.executor, o.root = "command", t.TempDir()
	ex := agentExecutor(o, func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil, nil, nil, nil, nil, nil)
	ce, ok := ex.(orchestrator.CommandExecutor)
	if !ok {
		t.Fatalf("executor=command must yield a CommandExecutor, got %T", ex)
	}
	return strings.Join(ce.Build(asset.Phase{Name: "implementer", Agent: "implementer"}, "balanced"), " ")
}

// A claude-family agent must carry --allowedTools pre-granting the read-only
// self-verification commands (node --test + node harness/gate.mjs) so a print-mode (-p)
// implementer can self-check its code and honestly tick a ROADMAP [x] — without it, Bash
// awaits a human approval that never comes headless and convergence's RoadmapCompletion
// stalls at 0%. ★The argv MUST NOT contain `forge`: a whitelisted forge would let the
// agent fork another agent outside the FORGE_AGENT_DEPTH recursion guard (a fork-bomb).★
func TestAgentExecutor_AllowedToolsForClaude(t *testing.T) {
	argv := buildAgentArgv(t, runOpts{agentCmd: "claude", agentAllowedTools: defaultAgentAllowedTools})
	if !strings.Contains(argv, "--allowedTools") {
		t.Errorf("claude must carry --allowedTools so a -p agent can self-verify; got: %s", argv)
	}
	if !strings.Contains(argv, "node --test") {
		t.Errorf("the whitelist must pre-grant `node --test` (the completion-discipline self-check); got: %s", argv)
	}
	if !strings.Contains(argv, "node harness/gate.mjs") {
		t.Errorf("the whitelist must pre-grant `node harness/gate.mjs` (the gate self-check); got: %s", argv)
	}
	// THE recursion-safety assertion: the read-only validators must never reach a command
	// that can re-spawn an agent. `forge` on the argv would bypass the depth guard entirely.
	if strings.Contains(argv, "forge") {
		t.Errorf("★recursion guard★: the --allowedTools whitelist must NOT contain `forge` (a fork-bomb escape outside FORGE_AGENT_DEPTH); got: %s", argv)
	}
}

// --agent-allowed-tools overrides the default node whitelist (so a non-node project can
// grant pytest/vitest instead) — and the override path must STILL stay recursion-safe.
func TestAgentExecutor_AllowedToolsOverridesDefault(t *testing.T) {
	argv := buildAgentArgv(t, runOpts{agentCmd: "claude", agentAllowedTools: "Bash(pytest*) Bash(vitest run*)"})
	if !strings.Contains(argv, "pytest") || !strings.Contains(argv, "vitest run") {
		t.Errorf("--agent-allowed-tools must override the default whitelist verbatim; got: %s", argv)
	}
	if strings.Contains(argv, "node --test") {
		t.Errorf("an explicit override must REPLACE the node default, not append it; got: %s", argv)
	}
	if strings.Contains(argv, "forge") {
		t.Errorf("★recursion guard★: even an overridden whitelist must not carry `forge`; got: %s", argv)
	}
}

// An empty agent-allowed-tools omits the flag (operator opt-out); a stub like echo never
// gets the claude-only flag (back-compat: only the claude-family path is touched).
func TestAgentExecutor_AllowedToolsEmptyAndNonClaude(t *testing.T) {
	if argv := buildAgentArgv(t, runOpts{agentCmd: "claude", agentAllowedTools: ""}); strings.Contains(argv, "--allowedTools") {
		t.Errorf("empty agent-allowed-tools must omit the flag; got: %s", argv)
	}
	if argv := buildAgentArgv(t, runOpts{agentCmd: "echo", agentAllowedTools: defaultAgentAllowedTools}); strings.Contains(argv, "--allowedTools") {
		t.Errorf("echo (a stub) must NOT receive the claude-only --allowedTools; got: %s", argv)
	}
}

// The default whitelist constant is recursion-safe by construction: read-only validators
// only, no `forge`/`evolve` token that could re-spawn an agent past the depth guard. This
// pins the INVARIANT at the source so a future whitelist edit cannot silently regress it.
func TestDefaultAgentAllowedTools_IsRecursionSafe(t *testing.T) {
	for _, banned := range []string{"forge", "evolve", "--executor"} {
		if strings.Contains(defaultAgentAllowedTools, banned) {
			t.Errorf("★recursion guard★: default whitelist must not contain %q (an agent-spawn escape outside FORGE_AGENT_DEPTH); got: %s", banned, defaultAgentAllowedTools)
		}
	}
	for _, want := range []string{"node --test", "node harness/gate.mjs"} {
		if !strings.Contains(defaultAgentAllowedTools, want) {
			t.Errorf("default whitelist must pre-grant the completion-discipline self-check %q; got: %s", want, defaultAgentAllowedTools)
		}
	}
}

// agentExecutor must pass --model <routed-tier> to a claude-family command so a
// real run honors ForgeOS's routing (the opus floor for reviewer/architect/cto),
// and must NOT pass that claude-only flag to a stub like echo.
func TestAgentExecutor_ModelTierForClaude(t *testing.T) {
	mk := func(cmd, agent string) string {
		ex := agentExecutor(runOpts{executor: "command", agentCmd: cmd, root: t.TempDir()}, func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil, nil, nil, nil, nil, nil)
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
		ex := agentExecutor(runOpts{executor: "command", agentCmd: cmd, agentMaxBudgetUSD: budget, root: t.TempDir()}, func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil, nil, nil, nil, nil, nil)
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
