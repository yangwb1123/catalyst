package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/memory"
	"forgeos/forge-core/internal/orchestrator"
)

// record stores the latest verdict per gate and preserves first-seen order; a
// re-recorded gate updates its status IN PLACE without changing its position (the
// loop-back / evolve-iteration case, where the same gate runs again).
func TestGateLedger_RecordUpdatesAndPreservesOrder(t *testing.T) {
	l := newGateLedger()
	l.record("test", "ok")
	l.record("complexity", "N/A")
	l.record("test", "FAILED") // same gate re-runs: update in place, keep position

	if l.status["test"] != "FAILED" {
		t.Errorf("re-recorded gate must take the latest verdict; got %q", l.status["test"])
	}
	if strings.Join(l.order, ",") != "test,complexity" {
		t.Errorf("order must be first-seen and stable; got %v", l.order)
	}
}

// An empty ledger renders no context block, and a nil ledger is safe (no panic,
// empty string) — both are the "no gate feedback to inject" case that keeps a
// prompt byte-for-byte unchanged. contextLines mirrors that (nil slice).
func TestGateLedger_ContextEmptyAndNil(t *testing.T) {
	if got := newGateLedger().context(); got != "" {
		t.Errorf("an empty ledger must render no context; got %q", got)
	}
	if got := newGateLedger().contextLines(); got != nil {
		t.Errorf("an empty ledger must yield no context lines; got %v", got)
	}
	var nilLedger *gateLedger
	if got := nilLedger.context(); got != "" {
		t.Errorf("a nil ledger must render no context (no panic); got %q", got)
	}
	nilLedger.record("test", "ok") // must not panic on a nil receiver
}

// A populated ledger renders each gate as "- <name>: <verdict>" in first-seen
// order, under the honest harness-results header; contextLines wraps it as one
// appendable element.
func TestGateLedger_ContextRendersGatesInOrder(t *testing.T) {
	l := newGateLedger()
	l.record("test", "ok")
	l.record("complexity", "ok")
	got := l.context()

	if !strings.Contains(got, "harness-gates") {
		t.Errorf("context must state these are harness-gates results; got %q", got)
	}
	// Ordered, exact per-gate lines.
	iTest := strings.Index(got, "- test: ok")
	iCplx := strings.Index(got, "- complexity: ok")
	if iTest < 0 || iCplx < 0 {
		t.Fatalf("context must list each gate verdict; got %q", got)
	}
	if iTest > iCplx {
		t.Errorf("gates must render in first-seen order (test before complexity); got %q", got)
	}
	if lines := l.contextLines(); len(lines) != 1 || lines[0] != got {
		t.Errorf("contextLines must wrap context() as a single element; got %v", lines)
	}
}

// phaseOutputLedger.record stores the latest output per phase and preserves first-seen
// order; a re-recorded phase (the loop-back / evolve-iteration case) updates its summary
// IN PLACE without changing position — the EXACT mirror of gateLedger.record.
func TestPhaseOutputLedger_RecordUpdatesAndPreservesOrder(t *testing.T) {
	l := newPhaseOutputLedger()
	l.record("planner", "plan v1")
	l.record("discover", "findings")
	l.record("planner", "plan v2") // same phase re-runs: update in place, keep position

	if l.summary["planner"] != "plan v2" {
		t.Errorf("re-recorded phase must take the latest output; got %q", l.summary["planner"])
	}
	if strings.Join(l.order, ",") != "planner,discover" {
		t.Errorf("order must be first-seen and stable; got %v", l.order)
	}
}

// An empty ledger renders no block, and a nil ledger is safe (no panic, empty string) —
// the "no feeds_forward output to inject" case that keeps a prompt byte-for-byte
// unchanged. contextLines mirrors that (nil slice). record on a nil receiver is a no-op.
func TestPhaseOutputLedger_ContextEmptyAndNil(t *testing.T) {
	if got := newPhaseOutputLedger().context(); got != "" {
		t.Errorf("an empty ledger must render no context; got %q", got)
	}
	if got := newPhaseOutputLedger().contextLines(); got != nil {
		t.Errorf("an empty ledger must yield no context lines; got %v", got)
	}
	var nilLedger *phaseOutputLedger
	if got := nilLedger.context(); got != "" {
		t.Errorf("a nil ledger must render no context (no panic); got %q", got)
	}
	nilLedger.record("planner", "x") // must not panic on a nil receiver
}

// A populated ledger renders each phase as a labeled block in first-seen order, under
// the honest "planning output" header; contextLines wraps it as one appendable element.
func TestPhaseOutputLedger_ContextRendersPhasesInOrder(t *testing.T) {
	l := newPhaseOutputLedger()
	l.record("planner", "split: task A, task B")
	got := l.context()

	if !strings.Contains(got, "planner 的任务拆分") {
		t.Errorf("context must state this is the planner's task split; got %q", got)
	}
	if !strings.Contains(got, "### planner") || !strings.Contains(got, "split: task A, task B") {
		t.Errorf("context must carry the phase's labeled output; got %q", got)
	}
	if lines := l.contextLines(); len(lines) != 1 || lines[0] != got {
		t.Errorf("contextLines must wrap context() as a single element; got %v", lines)
	}
}

// verdictLedger.record/get round-trips the latest verdict per phase; an unrecorded phase
// reports ok=false (no signal) and a nil ledger is panic-safe — the back-compat path that
// lets the orchestrator proceed (fail-open) when no verdict source is wired.
func TestVerdictLedger_RecordGetAndNilSafe(t *testing.T) {
	l := newVerdictLedger()
	if _, ok := l.get("reviewer"); ok {
		t.Error("an unrecorded phase must report ok=false (no verdict)")
	}
	l.record("reviewer", VerdictRequestChanges)
	l.record("reviewer", VerdictApprove) // a re-review's NEWEST verdict wins
	if v, ok := l.get("reviewer"); !ok || v != VerdictApprove {
		t.Errorf("get must return the latest recorded verdict; got (%q,%v)", v, ok)
	}
	var nilLedger *verdictLedger
	if _, ok := nilLedger.get("reviewer"); ok {
		t.Error("a nil ledger must report ok=false (no panic)")
	}
	nilLedger.record("reviewer", VerdictApprove) // must not panic on a nil receiver
}

// ★ THE one-direction edge proof ★: a reviewer's findings are recorded keyed by the
// LOOP-BACK TARGET (the implementer). buildPrompt then injects them into the IMPLEMENTER's
// prompt, but the REVIEWER — re-running with p.Name="reviewer" != the target — receives
// NOTHING. This pins the D3/AGENTS red line: a fresh-context reviewer is never fed its own
// prior findings, so the findings flow strictly reviewer → implementer, never back.
func TestReviewFindingsLedger_OneDirectionImplementerReceivesReviewerDoesNot(t *testing.T) {
	f := newReviewFindingsLedger()
	// The reviewer phase's on_fail.target is "implementer", so findings are keyed there.
	f.record("implementer", "## Review\n- main.go:42 HIGH: nil deref — guard it\nVERDICT: REQUEST_CHANGES")

	// The implementer (the loop-back recipient) MUST receive the findings block.
	implPrompt := buildPrompt("/home/u1/catalyst", asset.Phase{Name: "implementer", Agent: "implementer"}, "balanced", unbudgetedTier("balanced"), nil, nil, nil, f)
	if !strings.Contains(implPrompt, "上游审查/验收角色") || !strings.Contains(implPrompt, "nil deref") {
		t.Errorf("the implementer (loop-back target) must receive the reviewer findings; got: %.500s", implPrompt)
	}
	// The reviewer, re-running, MUST NOT receive them — fresh-context independence held.
	revPrompt := buildPrompt("/home/u1/catalyst", asset.Phase{Name: "reviewer", Agent: "reviewer"}, "balanced", unbudgetedTier("balanced"), nil, nil, nil, f)
	if strings.Contains(revPrompt, "上游审查/验收角色") || strings.Contains(revPrompt, "nil deref") {
		t.Errorf("the reviewer must NEVER receive its own prior findings (fresh-context); got: %.500s", revPrompt)
	}
}

// contextLines is honest about provenance and nil/empty-safe: an unrecorded phase (and a
// nil ledger) yield no block, so a prompt is byte-for-byte unchanged when there is nothing
// to inject; a recorded one is labeled as the reviewer's findings, explicitly not a gate.
func TestReviewFindingsLedger_ContextLinesProvenanceAndEmpty(t *testing.T) {
	var nilLedger *reviewFindingsLedger
	if got := nilLedger.contextLines("implementer"); got != nil {
		t.Errorf("a nil ledger must yield no context lines; got %v", got)
	}
	f := newReviewFindingsLedger()
	if got := f.contextLines("implementer"); got != nil {
		t.Errorf("an unrecorded phase must yield no context lines; got %v", got)
	}
	f.record("implementer", "concrete finding text")
	lines := f.contextLines("implementer")
	if len(lines) != 1 || !strings.Contains(lines[0], "非闸门结果") || !strings.Contains(lines[0], "concrete finding text") {
		t.Errorf("a recorded finding must render one honest (non-gate) block; got %v", lines)
	}
	// A different phase still gets nothing — the gate is by phase name.
	if got := f.contextLines("qa"); got != nil {
		t.Errorf("only the keyed target receives findings; qa got %v", got)
	}
}

// ★ THE FreshContext red-line proof ★: appendFeedbackLanes' `if p.FreshContext { return
// ctx }` early-return is a hard engineering rule (AGENTS.md/BOOTSTRAP.md) — a fresh-context
// Reviewer phase must NEVER see prior phase output, gate verdicts, review findings, or
// cross-session memory. This seeds ALL FOUR feedback lanes with real, distinctive content
// plus an emits artifact (which is NOT gated by FreshContext — asset-runtime-gap §1.2/§1.3
// draws that line explicitly), builds the prompt for a FreshContext:true phase, and asserts
// none of the feedback content leaked in while the artifact content still did — proving the
// gate is selective (feedback lanes only), not a blanket suppression of all context.
func TestBuildPrompt_FreshContextOmitsFeedbackLanes(t *testing.T) {
	root := t.TempDir()
	if err := memory.Append(memoryPath(root), memory.Entry{
		Kind: memory.KindGap, Topic: "reviewer", Detail: "SECRET-MEMORY-DETAIL-12345", Iteration: 1, CreatedAtUnix: 1,
	}); err != nil {
		t.Fatalf("seed memory: %v", err)
	}
	gates := newGateLedger()
	gates.record("test", "SECRET-GATE-VERDICT-67890")
	phaseOut := newPhaseOutputLedger()
	phaseOut.record("planner", "SECRET-PHASE-OUTPUT-24680")
	// Findings keyed to THIS phase name — the loop-back-target case.
	findings := newReviewFindingsLedger()
	findings.record("reviewer", "SECRET-FINDINGS-DETAIL-13579")
	// An emits artifact is NOT gated by FreshContext, so it must still appear.
	artifactPath := filepath.Join(root, "task-plan.md")
	if err := os.WriteFile(artifactPath, []byte("ARTIFACT-CONTENT-99999"), 0o644); err != nil {
		t.Fatalf("seed emits artifact: %v", err)
	}

	p := asset.Phase{Name: "reviewer", Agent: "reviewer", FreshContext: true}
	got := buildPromptWithEmits(root, p, "balanced", unbudgetedTier("balanced"), nil, gates, phaseOut, findings, []string{"task-plan.md"})

	for _, secret := range []string{
		"SECRET-MEMORY-DETAIL-12345", "SECRET-GATE-VERDICT-67890",
		"SECRET-PHASE-OUTPUT-24680", "SECRET-FINDINGS-DETAIL-13579",
		"前序闸门结果", "Project memory", "planner 的任务拆分", "非闸门结果",
	} {
		if strings.Contains(got, secret) {
			t.Errorf("FreshContext phase must NEVER see feedback-lane content; leaked %q into: %.800s", secret, got)
		}
	}
	// The sibling emits lane is NOT FreshContext-gated — proving selective suppression.
	if !strings.Contains(got, "ARTIFACT-CONTENT-99999") {
		t.Errorf("a FreshContext phase must still receive its declared emits artifact; got: %.800s", got)
	}
}

// onFailTargetOf is the data-driven (phase -> loop-back target) lookup the Observe sink
// uses to route findings: it returns a reviewer phase's on_fail.target with zero
// hard-coded agent name, and ok=false for a phase carrying no loop_back on_fail.
func TestOnFailTargetOf_ReadsLoopBackTargetDataDriven(t *testing.T) {
	wf, err := asset.LoadWorkflowJSON([]byte(`{
	  "stage": "build",
	  "phases": [
	    {"name": "implementer", "agent": "implementer", "required_gates": []},
	    {"name": "reviewer", "agent": "reviewer", "required_gates": [],
	     "on_fail": {"action": "loop_back", "target_phase": "implementer"}}
	  ],
	  "stop_condition": {"type": "external", "all_of": []}
	}`))
	if err != nil {
		t.Fatalf("load fixture: %v", err)
	}
	lookup := onFailTargetOf(wf)
	if target, ok := lookup("reviewer"); !ok || target != "implementer" {
		t.Errorf("reviewer on_fail.target must resolve to implementer; got (%q,%v)", target, ok)
	}
	if _, ok := lookup("implementer"); ok {
		t.Error("a phase with no on_fail must report ok=false")
	}
}

// The Observe sink end-to-end: feeding the verdict ledger a reviewer's REQUEST_CHANGES
// output must (1) record the normalized verdict for the phase, and (2) stash the findings
// for the phase's on_fail TARGET — so the engine reads REQUEST_CHANGES back AND the
// implementer's next prompt carries the findings. Driven directly via the installed sink.
func TestObserveFor_VerdictAndFindingsRouting(t *testing.T) {
	verdicts := newVerdictLedger()
	findings := newReviewFindingsLedger()
	target := func(phase string) (string, bool) {
		if phase == "reviewer" {
			return "implementer", true
		}
		return "", false
	}
	sink := observeFor(false, nil, nil, nil, nil, verdicts, findings, target)
	if sink == nil {
		t.Fatal("with verdict/findings ledgers wired, observeFor must return a sink")
	}
	sink("reviewer", "## Review\n- x.go:1 bug\nVERDICT: REQUEST_CHANGES", 0)

	if v, ok := verdicts.get("reviewer"); !ok || v != VerdictRequestChanges {
		t.Errorf("the verdict ledger must record REQUEST_CHANGES; got (%q,%v)", v, ok)
	}
	if got := findings.contextLines("implementer"); len(got) != 1 || !strings.Contains(got[0], "x.go:1 bug") {
		t.Errorf("the findings must be routed to the implementer (on_fail target); got %v", got)
	}
	// An APPROVE records the verdict but stashes NO findings (nothing to repair).
	sink("reviewer", "VERDICT: APPROVE", 0)
	if v, _ := verdicts.get("reviewer"); v != VerdictApprove {
		t.Errorf("a later APPROVE must overwrite the verdict; got %q", v)
	}
}

// The CTO's executive-review 5-way contract must record into the SAME verdictLedger
// as the reviewer's binary contract, reached only after parseReviewerVerdict fails to
// match (asset-runtime-gap: review.yml's review_status was always "" because nothing
// captured the CTO's verdict). No findings side effect: review.yml's executive-review
// phase carries no on_fail.loop_back, so onFailTarget correctly finds nothing to route.
func TestObserveFor_ExecutiveVerdictRoutesIntoSameLedger(t *testing.T) {
	verdicts := newVerdictLedger()
	findings := newReviewFindingsLedger()
	sink := observeFor(false, nil, nil, nil, nil, verdicts, findings, func(string) (string, bool) { return "", false })
	if sink == nil {
		t.Fatal("with verdict/findings ledgers wired, observeFor must return a sink")
	}
	sink("executive-review", "综合裁决...\nVERDICT: REDESIGN", 0)
	if v, ok := verdicts.get("executive-review"); !ok || v != VerdictRedesign {
		t.Errorf("the executive 5-way verdict must land in the SAME ledger; got (%q,%v)", v, ok)
	}
	if got := findings.contextLines("executive-review"); len(got) != 0 {
		t.Errorf("the executive verdict must stash NO findings (no on_fail target); got %v", got)
	}

	// APPROVE is shared vocabulary: it matches the BINARY parser first (both parsers
	// normalize it to the identical VerdictApprove token, so dispatch order is
	// unobservable for this one token) — proving reviewStatus (gates.go) never has
	// to know or care which parser actually produced it.
	sink("executive-review", "VERDICT: APPROVE", 0)
	if v, _ := verdicts.get("executive-review"); v != VerdictApprove {
		t.Errorf("VERDICT: APPROVE must normalize to VerdictApprove regardless of dispatch path; got %q", v)
	}

	// A build-stage reviewer's plain REQUEST_CHANGES must be completely unaffected by
	// the new executive fallback (it matches the binary parser and never reaches
	// parseExecutiveVerdict) — the existing behavior this task must not change.
	sink("reviewer", "VERDICT: REQUEST_CHANGES", 0)
	if v, ok := verdicts.get("reviewer"); !ok || v != VerdictRequestChanges {
		t.Errorf("an ordinary binary reviewer verdict must be unaffected; got (%q,%v)", v, ok)
	}
}

// The product-manager's numeric requirement-discovery contract must land in the SAME
// verdictLedger, reached only as the THIRD fallback tier — after BOTH
// parseReviewerVerdict and parseExecutiveVerdict fail to match (neither the reviewer's
// binary token nor the CTO's five-way token is a "CONFIDENCE: <N>" line). This is the
// gap-fix mirror of TestObserveFor_ExecutiveVerdictRoutesIntoSameLedger: before this
// wire, discover.yml's requirement_confidence stayed permanently 0 because nothing
// captured the product-manager's self-reported score.
func TestObserveFor_ConfidenceScoreRoutesIntoSameLedger(t *testing.T) {
	verdicts := newVerdictLedger()
	findings := newReviewFindingsLedger()
	sink := observeFor(false, nil, nil, nil, nil, verdicts, findings, func(string) (string, bool) { return "", false })
	if sink == nil {
		t.Fatal("with verdict/findings ledgers wired, observeFor must return a sink")
	}
	sink("requirement-discovery", "需求分析...\nCONFIDENCE: 85", 0)
	if v, ok := verdicts.get("requirement-discovery"); !ok || v != "85" {
		t.Errorf("the confidence score must land in the SAME ledger as the numeric string; got (%q,%v)", v, ok)
	}
	if got := findings.contextLines("requirement-discovery"); len(got) != 0 {
		t.Errorf("the confidence tier must stash NO findings (no on_fail target); got %v", got)
	}

	// A malformed/out-of-range confidence records NOTHING (no tier matches) — the
	// ledger keeps its prior, still-valid value rather than being overwritten with
	// nothing, and certainly never a fabricated one.
	sink("requirement-discovery", "CONFIDENCE: 150", 0)
	if v, _ := verdicts.get("requirement-discovery"); v != "85" {
		t.Errorf("an out-of-range confidence must not overwrite the prior recorded value; got %q", v)
	}

	// A build-stage reviewer's plain APPROVE must be completely unaffected by the new
	// confidence fallback (it matches the binary parser first and never reaches
	// parseConfidenceScore) — the existing behavior this task must not change.
	sink("reviewer", "VERDICT: APPROVE", 0)
	if v, ok := verdicts.get("reviewer"); !ok || v != VerdictApprove {
		t.Errorf("an ordinary binary reviewer verdict must be unaffected; got (%q,%v)", v, ok)
	}
}

// A long output is TRUNCATED to the cap (with an ellipsis marker) so a verbose planner
// cannot bloat downstream prompts; a short output is recorded whole. Truncation is
// rune-based, so a multi-byte boundary is never split mid-character.
func TestPhaseOutputLedger_TruncatesLongOutput(t *testing.T) {
	long := strings.Repeat("世", phaseOutputSummaryCap+50) // 850 multi-byte runes
	l := newPhaseOutputLedger()
	l.record("planner", long)
	got := l.summary["planner"]

	if !strings.Contains(got, "已截断") {
		t.Errorf("an over-cap output must carry the truncation marker; got %.40q", got)
	}
	// The retained prefix must be exactly the cap in RUNES (never split a UTF-8 char).
	kept := strings.Split(got, " …")[0]
	if n := len([]rune(kept)); n != phaseOutputSummaryCap {
		t.Errorf("retained prefix = %d runes, want the %d-rune cap", n, phaseOutputSummaryCap)
	}
	// A short output is stored verbatim, no marker.
	l.record("discover", "brief")
	if got := l.summary["discover"]; got != "brief" {
		t.Errorf("a short output must be stored whole; got %q", got)
	}
}

// requiresToolsGuard: discover.yml's requires_tools degrade-and-flag (market-
// research: [web_search, web_fetch]). Empty RequiresTools is always a pure no-op.
func TestRequiresToolsGuard_EmptyIsNoOp(t *testing.T) {
	var logs []string
	logln := func(s string) { logs = append(logs, s) }
	got := requiresToolsGuard(asset.Phase{Name: "implementer"}, true, true, "WebSearch WebFetch", logln, "PROMPT")
	if got != "PROMPT" || len(logs) != 0 {
		t.Errorf("empty RequiresTools must be a pure no-op; got %q, logs=%v", got, logs)
	}
}

// Every scenario that cannot AFFIRMATIVELY confirm tool access must degrade with
// an honest reason (dry-run, non-claude, no allowlist, or a partial allowlist) —
// never guessed either way — logging one visible ⚠ line naming the phase.
func TestRequiresToolsGuard_DegradesWhenUnconfirmed(t *testing.T) {
	tests := []struct {
		name, allowed, wantReason string
		isCommandExec, isClaude   bool
	}{
		{"dry-run narrates only", "", "dry-run", false, false},
		{"non-claude command", "", "non-claude", true, false},
		{"no allowedTools", "", "no --allowedTools", true, true},
		{"partial allowlist misses web_fetch", "WebSearch", "web_fetch", true, true},
	}
	for _, tc := range tests {
		p := asset.Phase{Name: "market-research", RequiresTools: []string{"web_search", "web_fetch"}}
		var logs []string
		logln := func(s string) { logs = append(logs, s) }
		got := requiresToolsGuard(p, tc.isCommandExec, tc.isClaude, tc.allowed, logln, "PROMPT")
		if !strings.Contains(got, "[context:requires_tools]") || !strings.Contains(got, tc.wantReason) {
			t.Errorf("%s: want degrade note containing %q; got %q", tc.name, tc.wantReason, got)
		}
		if len(logs) != 1 || !strings.Contains(logs[0], "⚠") || !strings.Contains(logs[0], "market-research") {
			t.Errorf("%s: want one visible ⚠ log line naming the phase; got %v", tc.name, logs)
		}
	}
}

// Confirmed path: every required tool's alias (snake_case collapsed to one word,
// case-insensitive) is present in --allowedTools — unchanged text, nothing logged.
func TestRequiresToolsGuard_ConfirmedProceedsUnchanged(t *testing.T) {
	var logs []string
	logln := func(s string) { logs = append(logs, s) }
	p := asset.Phase{Name: "market-research", RequiresTools: []string{"web_search", "web_fetch"}}
	got := requiresToolsGuard(p, true, true, "Bash(node --test*) WebSearch WebFetch", logln, "PROMPT")
	if got != "PROMPT" || len(logs) != 0 {
		t.Errorf("confirmed requires_tools must be a pure no-op; got %q, logs=%v", got, logs)
	}
}

// Live wiring: agentExecutor's --executor=command Build closure routes the REAL
// assembled prompt through requiresToolsGuard, so a live claude spawn's argv
// carries the degrade note end-to-end — proving this is wired, not a dead helper.
func TestAgentExecutor_RequiresToolsWiredIntoRealPrompt(t *testing.T) {
	var logs []string
	logln := func(s string) { logs = append(logs, s) }
	o := runOpts{root: "/home/u1/catalyst", executor: "command", agentCmd: "claude"} // agentAllowedTools unset
	ex := agentExecutor(o, logln, nil, unbudgetedTier("balanced"), nil, nil, nil, nil, nil, nil, nil, nil, nil)
	ce, ok := ex.(orchestrator.CommandExecutor)
	if !ok {
		t.Fatalf("--executor=command must select orchestrator.CommandExecutor, got %T", ex)
	}
	argv := ce.Build(asset.Phase{Name: "market-research", Agent: "product-manager", RequiresTools: []string{"web_search", "web_fetch"}}, "balanced")
	if promptArg := argv[len(argv)-1]; !strings.Contains(promptArg, "[context:requires_tools]") {
		t.Errorf("unconfirmed requires_tools must carry the degrade note in the spawned prompt; got tail %.200q", promptArg)
	}
	warned := false
	for _, l := range logs {
		warned = warned || (strings.Contains(l, "⚠") && strings.Contains(l, "market-research"))
	}
	if !warned {
		t.Errorf("degrade must log a visible ⚠ line naming the phase; got %v", logs)
	}

	logs = nil // a phase with NO RequiresTools must be completely unaffected
	argv2 := ce.Build(asset.Phase{Name: "implementer", Agent: "implementer"}, "balanced")
	if promptArg2 := argv2[len(argv2)-1]; strings.Contains(promptArg2, "requires_tools") {
		t.Errorf("a phase without RequiresTools must never carry a requires_tools note; got tail %.200q", promptArg2)
	}
	if len(logs) != 0 {
		t.Errorf("a phase without RequiresTools must log nothing about requires_tools; got %v", logs)
	}
}
