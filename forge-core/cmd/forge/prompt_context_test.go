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
