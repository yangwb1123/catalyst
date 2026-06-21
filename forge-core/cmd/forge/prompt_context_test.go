package main

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
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
	if !strings.Contains(implPrompt, "上一轮 fresh-context Reviewer") || !strings.Contains(implPrompt, "nil deref") {
		t.Errorf("the implementer (loop-back target) must receive the reviewer findings; got: %.500s", implPrompt)
	}
	// The reviewer, re-running, MUST NOT receive them — fresh-context independence held.
	revPrompt := buildPrompt("/home/u1/catalyst", asset.Phase{Name: "reviewer", Agent: "reviewer"}, "balanced", unbudgetedTier("balanced"), nil, nil, nil, f)
	if strings.Contains(revPrompt, "上一轮 fresh-context Reviewer") || strings.Contains(revPrompt, "nil deref") {
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
