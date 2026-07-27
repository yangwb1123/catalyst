package main

import (
	"strings"
	"testing"
)

func TestPhaseOutputLedgerNormalizesTaskListAndExposesModelHint(t *testing.T) {
	ledger := newPhaseOutputLedger()
	ledger.record("planner", `notes
TASK_LIST:
- [ ] T002: second — acceptance: pass — files: b.go — depends_on: T001 — model: opus — roadmap: v1
- [ ] T001: first — acceptance: pass — files: a.go — depends_on: none — model: haiku — roadmap: v1`)

	if got := ledger.recommendedTaskModel(); got != "haiku" {
		t.Fatalf("recommendedTaskModel = %q, want first dependency-ready task's haiku", got)
	}
	ctx := ledger.context()
	first := strings.Index(ctx, "T001:")
	second := strings.Index(ctx, "T002:")
	if first < 0 || second < 0 || first >= second {
		t.Fatalf("normalized context must be topological:\n%s", ctx)
	}
}

func TestPhaseOutputLedgerInvalidTaskListHasNoRoutingHint(t *testing.T) {
	ledger := newPhaseOutputLedger()
	ledger.record("planner", "TASK_LIST:\n- malformed")
	if got := ledger.recommendedTaskModel(); got != "" {
		t.Fatalf("invalid plan model hint = %q, want empty", got)
	}
}
