package main

import (
	"strings"
	"testing"
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
