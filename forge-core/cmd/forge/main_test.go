package main

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/memory"
	"forgeos/forge-core/internal/orchestrator"
)

// buildPrompt must embed the role, phase, routed tier, and the agent's card so
// a real `claude -p` invocation has the full instruction. reviewer floors to
// opus regardless of mode.
func TestBuildPrompt_EmbedsRolePhaseTier(t *testing.T) {
	p := asset.Phase{Name: "reviewer", Agent: "reviewer"}
	got := buildPrompt("/home/u1/catalyst", p, "balanced", unbudgetedTier("balanced"), nil, nil, nil, nil)
	for _, want := range []string{`"reviewer" agent`, "phase=reviewer", "tier=opus"} {
		if !strings.Contains(got, want) {
			t.Errorf("prompt missing %q", want)
		}
	}
}

// A missing card must not break prompt assembly — it degrades to a marker.
func TestBuildPrompt_MissingCardDegrades(t *testing.T) {
	p := asset.Phase{Name: "ghost", Agent: "no-such-agent"}
	got := buildPrompt("/home/u1/catalyst", p, "balanced", unbudgetedTier("balanced"), nil, nil, nil, nil)
	if !strings.Contains(got, "no role card found") {
		t.Errorf("expected missing-card marker; got: %.80s", got)
	}
}

// Backward-compat: the Context Engine upgrade must not drop the hard-constraint
// injection. A prompt built from the REAL repo must still carry the leading
// AGENTS.md constraints (the 500-line cap), exactly as before retrieval+memory.
func TestBuildPrompt_StillInjectsHardConstraints(t *testing.T) {
	got := buildPrompt("/home/u1/catalyst", asset.Phase{Name: "reviewer", Agent: "reviewer"}, "balanced", unbudgetedTier("balanced"), nil, nil, nil, nil)
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
	got := buildPrompt(root, asset.Phase{Name: "build", Agent: "implementer"}, "balanced", unbudgetedTier("balanced"), nil, nil, nil, nil)
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
	got := buildPrompt(root, asset.Phase{Name: "build", Agent: "implementer"}, "balanced", unbudgetedTier("balanced"), nil, nil, nil, nil)
	if strings.Contains(got, "Project memory") {
		t.Errorf("cold start must omit the memory block; got: %.300s", got)
	}
	if strings.Contains(got, "UNREADABLE") {
		t.Errorf("a missing store must not be reported as unreadable; got: %.300s", got)
	}
}

// buildPrompt must inject the ledger's gate verdicts when the ledger carries any,
// so a downstream phase (e.g. the reviewer) sees the objective results in its
// prompt instead of trying to re-run the checks itself. (Lives here, not in
// prompt_context_test.go, because asserting through buildPrompt needs asset.Phase —
// and main_test.go already imports asset, so it adds no internal/asset fan-in.)
func TestBuildPrompt_InjectsGateLedger(t *testing.T) {
	l := newGateLedger()
	l.record("test", "ok")
	l.record("complexity", "ok")
	got := buildPrompt("/home/u1/catalyst", asset.Phase{Name: "reviewer", Agent: "reviewer"}, "balanced", unbudgetedTier("balanced"), nil, l, nil, nil)

	if !strings.Contains(got, "前序闸门结果") || !strings.Contains(got, "- test: ok") {
		t.Errorf("prompt must carry the prior gate results when the ledger is populated; got: %.500s", got)
	}
}

// Back-compat: with a nil ledger (and with an empty one) buildPrompt must NOT add
// the gate-results block — the prompt is exactly the pre-feedback one.
func TestBuildPrompt_NilOrEmptyLedgerOmitsBlock(t *testing.T) {
	p := asset.Phase{Name: "reviewer", Agent: "reviewer"}
	nilGot := buildPrompt("/home/u1/catalyst", p, "balanced", unbudgetedTier("balanced"), nil, nil, nil, nil)
	emptyGot := buildPrompt("/home/u1/catalyst", p, "balanced", unbudgetedTier("balanced"), nil, newGateLedger(), nil, nil)
	for _, got := range []string{nilGot, emptyGot} {
		if strings.Contains(got, "前序闸门结果") {
			t.Errorf("a nil/empty ledger must omit the gate-results block; got: %.300s", got)
		}
	}
}

// buildPrompt must inject the phase-output ledger when it carries a fed-forward output,
// so a downstream phase (e.g. the reviewer) sees the planner's task split in its prompt.
func TestBuildPrompt_InjectsPhaseOutput(t *testing.T) {
	po := newPhaseOutputLedger()
	po.record("planner", "Sprint split: implement X; acceptance: gate test green")
	got := buildPrompt("/home/u1/catalyst", asset.Phase{Name: "reviewer", Agent: "reviewer"}, "balanced", unbudgetedTier("balanced"), nil, nil, po, nil)

	if !strings.Contains(got, "planner 的任务拆分") || !strings.Contains(got, "Sprint split: implement X") {
		t.Errorf("prompt must carry the prior planning output when the ledger is populated; got: %.500s", got)
	}
}

// Back-compat: with a nil phase-output ledger (and with an empty one) buildPrompt must
// NOT add the planning-output block — the prompt is exactly the pre-feed-forward one.
func TestBuildPrompt_NilOrEmptyPhaseOutputOmitsBlock(t *testing.T) {
	p := asset.Phase{Name: "implementer", Agent: "implementer"}
	nilGot := buildPrompt("/home/u1/catalyst", p, "balanced", unbudgetedTier("balanced"), nil, nil, nil, nil)
	emptyGot := buildPrompt("/home/u1/catalyst", p, "balanced", unbudgetedTier("balanced"), nil, nil, newPhaseOutputLedger(), nil)
	for _, got := range []string{nilGot, emptyGot} {
		if strings.Contains(got, "前序规划阶段产出") {
			t.Errorf("a nil/empty phase-output ledger must omit the planning block; got: %.300s", got)
		}
	}
}

// feedsForwardOf bridges asset.FeedsForward to the executor's name-only Observe seam:
// it reports true for a phase that declares feeds_forward, false for one that does not,
// and false for an unknown name — so only the planner's output is ever fed forward.
func TestFeedsForwardOf(t *testing.T) {
	wf := asset.Workflow{Phases: []asset.Phase{
		{Name: "planner", Agent: "planner", FeedsForward: true},
		{Name: "reviewer", Agent: "reviewer"},
	}}
	pred := feedsForwardOf(wf)
	if !pred("planner") {
		t.Error("planner declares feeds_forward -> predicate must be true")
	}
	if pred("reviewer") {
		t.Error("reviewer does not declare feeds_forward -> predicate must be false (no fresh-context pollution)")
	}
	if pred("ghost") {
		t.Error("an unknown phase -> predicate must be false")
	}
}

// The feed-forward record path, exercised through the Observe sink agentExecutor installs:
// when feedsForward(phase) is true, the phase's output is recorded into the phase-output
// ledger (unwrapped from a claude envelope), so the NEXT phase's prompt carries it. A phase
// for which feedsForward is false records NOTHING — the reviewer never feeds itself forward.
// Driven directly (no real spawn) by invoking the installed Observe with fixtures.
func TestAgentExecutor_ObserveRecordsFeedsForwardOutput(t *testing.T) {
	po := newPhaseOutputLedger()
	feeds := func(phase string) bool { return phase == "planner" }
	ex := agentExecutor(runOpts{executor: "command", agentCmd: "echo", root: t.TempDir()}, func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, po, feeds, nil, nil, nil)
	ce, ok := ex.(orchestrator.CommandExecutor)
	if !ok {
		t.Fatalf("executor=command must yield a CommandExecutor, got %T", ex)
	}
	if ce.Observe == nil {
		t.Fatal("with a phase-output ledger present, even an echo executor must install an Observe sink (feed-forward works under echo)")
	}
	ce.Observe("planner", "task split: A, B, C", 0)
	ce.Observe("reviewer", "I, the reviewer, looked at the diff", 0) // not fed forward

	if po.summary["planner"] != "task split: A, B, C" {
		t.Errorf("a feeds_forward phase's output must be recorded; got %q", po.summary["planner"])
	}
	if _, recorded := po.summary["reviewer"]; recorded {
		t.Error("a non-feeds_forward phase (reviewer) must NOT be recorded — no fresh-context pollution")
	}
}

// A claude envelope handed to the feed-forward Observe sink must be UNWRAPPED to its
// human-readable result before being remembered, so the downstream prompt carries the
// planner's actual plan text, not the raw JSON envelope.
func TestAgentExecutor_ObserveUnwrapsClaudeOutputForFeedForward(t *testing.T) {
	po := newPhaseOutputLedger()
	feeds := func(string) bool { return true }
	ex := agentExecutor(runOpts{executor: "command", agentCmd: "claude", root: t.TempDir()}, func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, po, feeds, nil, nil, nil)
	ce := ex.(orchestrator.CommandExecutor)
	ce.Observe("planner", realClaudeJSON, 0)
	if got := po.summary["planner"]; got != "done editing main.go" {
		t.Errorf("a claude envelope must be unwrapped to its result for feed-forward; got %q", got)
	}
}

// buildPrompt must inject the AI-SDLC template content when the phase declares
// uses_template (eighth-wave-adr-decay.md §方向2: uses_template 字段代码化).
// The template content appears as a [context:template:...] block in the prompt.
func TestBuildPrompt_UsesTemplateInjectsContent(t *testing.T) {
	root := t.TempDir()
	tmplDir := filepath.Join(root, ".ai", "prompts")
	if err := os.MkdirAll(tmplDir, 0o755); err != nil {
		t.Fatalf("MkdirAll: %v", err)
	}
	tmplPath := filepath.Join(tmplDir, "02-security-rfc-review.md")
	tmplContent := "# Security RFC Review Template\n\n## STRIDE Analysis\n- Spoofing\n- Tampering\n"
	if err := os.WriteFile(tmplPath, []byte(tmplContent), 0o644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
	p := asset.Phase{Name: "security-review", Agent: "security-engineer", UsesTemplate: ".ai/prompts/02-security-rfc-review.md"}
	got := buildPromptWithEmits(root, p, "balanced", unbudgetedTier("balanced"), nil, nil, nil, nil, nil)
	if !strings.Contains(got, "[context:template:") {
		t.Errorf("buildPrompt with uses_template must inject a [context:template:...] block, got:\n%s", got)
	}
	if !strings.Contains(got, "STRIDE Analysis") {
		t.Errorf("buildPrompt must inject template content, got:\n%s", got)
	}
	// Without uses_template, the template block must be absent.
	plain := buildPromptWithEmits(root, asset.Phase{Name: "security-review", Agent: "security-engineer"}, "balanced", unbudgetedTier("balanced"), nil, nil, nil, nil, nil)
	if strings.Contains(plain, "[context:template:") {
		t.Errorf("buildPrompt without uses_template must not inject a template block, got:\n%s", plain)
	}
}

// buildPrompt must WARN (via stderr) but NOT fail when uses_template references
// a missing file — the phase still runs, just without specialized guidance.
func TestBuildPrompt_UsesTemplateMissingFileWarns(t *testing.T) {
	root := t.TempDir()
	// Capture stderr via a pipe.
	oldStderr := os.Stderr
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatalf("pipe: %v", err)
	}
	os.Stderr = w

	p := asset.Phase{Name: "security-review", Agent: "security-engineer", UsesTemplate: ".ai/prompts/nonexistent.md"}
	got := buildPromptWithEmits(root, p, "balanced", unbudgetedTier("balanced"), nil, nil, nil, nil, nil)
	// Restore stderr before reading the pipe.
	w.Close()
	os.Stderr = oldStderr
	var stderrBuf bytes.Buffer
	if _, err := stderrBuf.ReadFrom(r); err != nil {
		t.Fatalf("read stderr pipe: %v", err)
	}
	if !strings.Contains(stderrBuf.String(), "WARNING uses_template") {
		t.Errorf("missing uses_template file must produce a WARNING on stderr, got: %q", stderrBuf.String())
	}
	// The prompt must still be valid (no crash, no template block injected).
	if strings.Contains(got, "[context:template:") {
		t.Errorf("missing template must not inject a template block, got:\n%s", got)
	}
}

// ── usage() text must match actual CLI behavior ───────────────────────────
//
// A fresh-context review found usage() had drifted from the real CLI: (1) the
// grouped line implied `forge preflight [--root DIR]` needs no positional, but
// cmdPreflight/parsePreflightFlags require a leading <workflow> (splitPositional
// returns exit 2 "exactly one <workflow> required" without one); (2) `approve`
// is registered in the subcommands dispatch table (main.go) but was entirely
// absent from usage(); (3) route's usage line was missing --scorecard even
// though route.go's flag set still defines and honors it. These assertions pin
// usage() to the real behavior so a future edit can't silently reintroduce any
// of the three drifts.
func TestUsage_MatchesActualCLIBehavior(t *testing.T) {
	out := captureUsageStderr(t)

	if !strings.Contains(out, "forge preflight <workflow>") {
		t.Errorf("usage() must show preflight's required positional <workflow>, got:\n%s", out)
	}
	if strings.Contains(out, "scorecard|validate|memory-prune|status|doctor|preflight") {
		t.Errorf("preflight must NOT be grouped with the no-positional-arg commands (it requires <workflow>), got:\n%s", out)
	}
	if !strings.Contains(out, "forge approve") {
		t.Errorf("usage() must mention the registered `approve` subcommand, got:\n%s", out)
	}
	if !strings.Contains(out, "--scorecard") {
		t.Errorf("usage() must mention route's --scorecard flag (route.go still defines/honors it), got:\n%s", out)
	}
}

// TestUsage_PreflightPositionalIsActuallyRequired backs assertion (1) above with the
// real CLI behavior it documents: `forge preflight` with no <workflow> exits 2 and
// names the missing positional arg, never silently defaulting.
func TestUsage_PreflightPositionalIsActuallyRequired(t *testing.T) {
	_, code, ok := parsePreflightFlags(nil)
	if ok || code != 2 {
		t.Errorf("forge preflight with no <workflow> must fail (code=2, ok=false); got code=%d ok=%v", code, ok)
	}
}

// TestUsage_ApproveIsDispatchable backs assertion (2): `approve` really is a
// live subcommand (registered in the subcommands table), not a stale usage claim.
func TestUsage_ApproveIsDispatchable(t *testing.T) {
	if _, ok := subcommands["approve"]; !ok {
		t.Error("`approve` must be registered in the subcommands dispatch table for usage() to document it")
	}
}

// captureUsageStderr runs usage() with os.Stderr redirected to a pipe and returns
// what it printed.
func captureUsageStderr(t *testing.T) string {
	t.Helper()
	oldStderr := os.Stderr
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatalf("pipe: %v", err)
	}
	os.Stderr = w
	usage()
	w.Close()
	os.Stderr = oldStderr
	var buf bytes.Buffer
	if _, err := buf.ReadFrom(r); err != nil {
		t.Fatalf("read stderr pipe: %v", err)
	}
	return buf.String()
}

// --- test helpers ------------------------------------------------------------

// unbudgetedTier is the tierOf a buildPrompt test passes when budget is irrelevant: it
// wraps orchestrator.PhaseTier with NO near-budget adjustment (the byte-identical, ratio-0
// path the production resolver takes when spend is under the 0.80 gate or there is no cap).
// Using the real PhaseTier keeps these prompt assertions pinned to the actual routed tier.
func unbudgetedTier(mode string) func(p asset.Phase) string {
	return func(p asset.Phase) string { return orchestrator.PhaseTier(p, mode) }
}

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
