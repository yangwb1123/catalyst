package main

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

func TestTaskAwareTierResolverRaisesButNeverLowers(t *testing.T) {
	plans := newPhaseOutputLedger()
	plans.record("planner", `TASK_LIST:
- [ ] T001: critical task — acceptance: pass — files: a.go — depends_on: none — model: opus — roadmap: v1`)
	resolver := taskAwareTierResolver(func(asset.Phase) string { return "sonnet" }, plans, nil)
	if got := resolver(asset.Phase{Name: "implementer", Agent: "implementer"}); got != "opus" {
		t.Fatalf("task hint did not raise tier: %q", got)
	}
	if got := resolver(asset.Phase{Name: "reviewer", Agent: "reviewer"}); got != "sonnet" {
		t.Fatalf("non-implementer tier changed: %q", got)
	}

	cheap := newPhaseOutputLedger()
	cheap.record("planner", `TASK_LIST:
- [ ] T001: easy task — acceptance: pass — files: a.go — depends_on: none — model: haiku — roadmap: v1`)
	resolver = taskAwareTierResolver(func(asset.Phase) string { return "sonnet" }, cheap, nil)
	if got := resolver(asset.Phase{Name: "implementer", Agent: "implementer"}); got != "sonnet" {
		t.Fatalf("cheap hint lowered safety/base tier: %q", got)
	}
}

func TestLoopbackVerdictAdaptsNegativeExecutiveOutcomes(t *testing.T) {
	for _, outcome := range []string{VerdictRedesign, VerdictDelay, VerdictReject} {
		t.Run(outcome, func(t *testing.T) {
			ledger := newVerdictLedger()
			ledger.record("executive-review", outcome)
			got, ok := loopbackVerdict(ledger)("executive-review")
			if !ok || got != VerdictRequestChanges {
				t.Fatalf("loop-back verdict = (%q, %v)", got, ok)
			}
			if exact, _ := ledger.get("executive-review"); exact != outcome {
				t.Fatalf("reporting verdict was mutated: %q", exact)
			}
		})
	}
}

func TestExecutiveRejectionFeedsDeclaredLoopbackTarget(t *testing.T) {
	findings := newReviewFindingsLedger()
	target := func(phase string) (string, bool) {
		return "security-review", phase == "executive-review"
	}
	recordLoopbackFindings("executive-review", VerdictRedesign, "risks\nVERDICT: REDESIGN", findings, target)
	context := strings.Join(findings.contextLines("security-review"), "\n")
	if !strings.Contains(context, "risks") || !strings.Contains(context, VerdictRedesign) {
		t.Fatalf("negative executive findings not routed: %q", context)
	}

	approved := newReviewFindingsLedger()
	recordLoopbackFindings("executive-review", VerdictApprove, "VERDICT: APPROVE", approved, target)
	if got := approved.contextLines("security-review"); len(got) != 0 {
		t.Fatalf("approved verdict created repair findings: %v", got)
	}
}
