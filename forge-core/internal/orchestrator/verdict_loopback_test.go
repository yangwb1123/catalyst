package orchestrator

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

// This file isolates the REVIEWER-VERDICT loop-back tests — the agent-phase twin of
// loopback_test.go's gate loop-back. They share the in-package helpers from
// orchestrator_test.go (recorder, contains, containsLine, allOK) and live in their own
// file so the suite stays within the harness's max_file_lines=500 cap. The mechanism
// under test: after a CLEAN reviewer agent phase, Engine.agentOutcome consults the
// injected AgentVerdict puller; a REQUEST_CHANGES drives a DIRECTED loop-back to the
// reviewer phase's on_fail.target (implementer), while APPROVE / no-verdict / nil proceed.

// verdictWorkflow mirrors build.yml's reviewer shape: planner(agent) →
// implementer(agent) → reviewer(AGENT, on_fail loop_back to implementer) → qa(agent).
// EVERY phase here is an agent phase (no gates), so the reviewer's verdict is the ONLY
// thing that can trigger a loop-back — isolating the agent-verdict path from the gate
// path. The reviewer's on_fail TARGET is implementer (index 1), NOT planner (index 0),
// so a loop-back re-runs implementer but must NOT re-run planner — that asymmetry proves
// the jump is DIRECTED. qa's presence in recorder.executed proves the run PROCEEDED past
// an APPROVE (or a healed REQUEST_CHANGES); its absence (with an abort) would prove a stop.
const verdictWorkflow = `{
  "stage": "build",
  "phases": [
    {"name": "planner", "agent": "planner", "readonly": true, "required_gates": []},
    {"name": "implementer", "agent": "implementer", "readonly": false, "required_gates": []},
    {"name": "reviewer", "agent": "reviewer", "readonly": true, "required_gates": [],
     "on_fail": {"action": "loop_back", "target_phase": "implementer"}},
    {"name": "qa", "agent": "qa", "readonly": true, "required_gates": []}
  ],
  "stop_condition": {"type": "conjunction", "all_of": [], "anti_pattern": "round_count"}
}`

func loadVerdict(t *testing.T) asset.Workflow {
	t.Helper()
	wf, err := asset.LoadWorkflowJSON([]byte(verdictWorkflow))
	if err != nil {
		t.Fatalf("load verdict fixture: %v", err)
	}
	return wf
}

// flakyVerdict is the agent-verdict counterpart to loopback_test.go's flakyGate: it
// returns REQUEST_CHANGES for the named phase the first changesUntil times that phase is
// asked, then APPROVE — the fake "request-changes-then-approve" reviewer that proves the
// engine looped back rather than proceeding. Any other phase yields ok=false (no verdict,
// the common case for non-reviewer phases). calls counts reviewer verdict reads.
type flakyVerdict struct {
	phase        string
	changesUntil int
	calls        int
}

func (v *flakyVerdict) verdict(phase string) (string, bool) {
	if phase != v.phase {
		return "", false // non-reviewer phase: no machine-readable verdict.
	}
	v.calls++
	if v.calls <= v.changesUntil {
		return reviewerRequestChanges, true
	}
	return "APPROVE", true
}

// ★ THE proof of agent-verdict DIRECTED loop-back ★: the reviewer returns
// REQUEST_CHANGES once then APPROVE, and the reviewer phase declares on_fail loop_back to
// implementer. With a MaxLoopBack budget of 3 the engine must JUMP BACK to implementer
// (re-running implementer→reviewer) once, then on the second reviewer run the verdict is
// APPROVE and the run continues to qa — NOT abort, NOT a whole-workflow replay (planner
// runs exactly once; implementer runs 1 + 1 loop-back = 2 times).
func TestRun_ReviewerRequestChangesLoopsBackThenApproves(t *testing.T) {
	wf := loadVerdict(t)
	rec := &recorder{}
	fv := &flakyVerdict{phase: "reviewer", changesUntil: 1}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log, MaxLoopBack: 3, AgentVerdict: fv.verdict}

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("Run should heal via the reviewer loop-back and complete; got %v", err)
	}
	// planner once (NOT replayed — proves the jump targets implementer, not phase 0),
	// implementer twice (initial + one loop-back), reviewer twice (request-changes then
	// approve), qa once after approval.
	want := []string{"planner", "implementer", "reviewer", "implementer", "reviewer", "qa"}
	if strings.Join(rec.executed, ",") != strings.Join(want, ",") {
		t.Errorf("reviewer loop-back executed %v, want %v", rec.executed, want)
	}
	if fv.calls != 2 {
		t.Errorf("reviewer verdict reads = %d, want 2 (one REQUEST_CHANGES + one APPROVE)", fv.calls)
	}
	if !containsLine(rec.logs, "reviewer verdict REQUEST_CHANGES, loop-back 1/3 to implementer") {
		t.Errorf("the loop-back must log the directed jump with the verdict reason; logs=%v", rec.logs)
	}
	if !containsLine(rec.logs, "stop: condition declared") {
		t.Errorf("a healed run must complete and report stop; logs=%v", rec.logs)
	}
}

// APPROVE on the FIRST review proceeds straight through with NO loop-back: implementer
// runs once, reviewer once, qa runs — the happy path that proves an approving verdict
// never bounces the run.
func TestRun_ReviewerApproveProceedsNoLoopBack(t *testing.T) {
	wf := loadVerdict(t)
	rec := &recorder{}
	// changesUntil 0 => the very first reviewer read returns APPROVE.
	fv := &flakyVerdict{phase: "reviewer", changesUntil: 0}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log, MaxLoopBack: 3, AgentVerdict: fv.verdict}

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("an APPROVE verdict must complete cleanly; got %v", err)
	}
	want := []string{"planner", "implementer", "reviewer", "qa"}
	if strings.Join(rec.executed, ",") != strings.Join(want, ",") {
		t.Errorf("APPROVE path executed %v, want %v (no loop-back)", rec.executed, want)
	}
	if containsLine(rec.logs, "loop-back") {
		t.Errorf("an APPROVE verdict must log no loop-back; logs=%v", rec.logs)
	}
}

// FAIL-OPEN on budget exhaustion is the KEY semantic difference from a gate: a reviewer
// that keeps returning REQUEST_CHANGES past the loop-back budget must NOT abort — once
// the budget is spent the run PROCEEDS forward to qa (the harness gates + qa are the
// fail-closed backstop; the reviewer is a spec-tier check). Here the reviewer always
// requests changes and the budget is 2, so after 2 loop-backs the run proceeds to qa.
func TestRun_ReviewerBudgetExhaustedFailsOpenProceeds(t *testing.T) {
	wf := loadVerdict(t)
	rec := &recorder{}
	fv := &flakyVerdict{phase: "reviewer", changesUntil: 99} // never approves
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log, MaxLoopBack: 2, AgentVerdict: fv.verdict}

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("reviewer budget exhaustion must FAIL OPEN (proceed), not abort; got %v", err)
	}
	// implementer: 1 initial + 2 loop-backs = 3; reviewer: 3 reads; then qa runs (proceed).
	if got := strings.Count(strings.Join(rec.executed, ","), "implementer"); got != 3 {
		t.Errorf("implementer must run 3 times (initial + 2 loop-backs); executed=%v", rec.executed)
	}
	if !contains(rec.executed, "qa") {
		t.Errorf("a budget-exhausted reviewer must FAIL OPEN — qa must still run; executed=%v", rec.executed)
	}
	if !containsLine(rec.logs, "loop-back budget spent") {
		t.Errorf("budget exhaustion must log the fail-open reason; logs=%v", rec.logs)
	}
	// Pin the honest outcome suffix: the reviewer/agent path is fail-OPEN (qa ran
	// above), so the log must say "proceeding (fail-open)". A regression that
	// re-hardcodes "aborting (fail-closed)" — a fail-closed abort that never happens
	// here — breaks this test instead of silently lying about what the run did.
	if !containsLine(rec.logs, "proceeding (fail-open)") {
		t.Errorf("reviewer budget exhaustion is fail-open; the log must say so, not abort; logs=%v", rec.logs)
	}
	if !containsLine(rec.logs, "stop: condition declared") {
		t.Errorf("a fail-open run must still complete and report stop; logs=%v", rec.logs)
	}
}

// BACK-COMPAT: a nil AgentVerdict puller (the dry/echo path, or any run that wires no
// verdict source) must behave EXACTLY as before this field existed — every agent phase
// proceeds, no loop-back, no panic. The reviewer's on_fail is simply inert.
func TestRun_NilAgentVerdictIsBackCompat(t *testing.T) {
	wf := loadVerdict(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log, MaxLoopBack: 3} // AgentVerdict nil
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("a nil AgentVerdict must not panic; got %v", r)
		}
	}()

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("a nil AgentVerdict must complete cleanly (back-compat); got %v", err)
	}
	want := []string{"planner", "implementer", "reviewer", "qa"}
	if strings.Join(rec.executed, ",") != strings.Join(want, ",") {
		t.Errorf("nil-verdict path executed %v, want %v (no loop-back)", rec.executed, want)
	}
	if containsLine(rec.logs, "loop-back") {
		t.Errorf("a nil AgentVerdict must log no loop-back; logs=%v", rec.logs)
	}
}

// A verdict puller that returns ok=false (an unparsable/absent verdict — e.g. the
// reviewer's last line was malformed) is treated as NO signal: the run proceeds, exactly
// like APPROVE. This pins parseReviewerVerdict's fail-open contract at the engine seam.
func TestRun_UnparsableVerdictProceeds(t *testing.T) {
	wf := loadVerdict(t)
	rec := &recorder{}
	// Always ok=false: the puller never recognized a verdict.
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log, MaxLoopBack: 3,
		AgentVerdict: func(string) (string, bool) { return "", false }}

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("an ok=false verdict must proceed (fail-open), not abort; got %v", err)
	}
	if !contains(rec.executed, "qa") {
		t.Errorf("an unrecognized verdict must fail open — qa must run; executed=%v", rec.executed)
	}
	if containsLine(rec.logs, "loop-back") {
		t.Errorf("an ok=false verdict must log no loop-back; logs=%v", rec.logs)
	}
}

// A non-loop_back on_fail target asymmetry guard at the agent layer: even with a parsed
// REQUEST_CHANGES, a reviewer phase that declares NO on_fail must proceed (no jump
// target), proving loopBackTo's declaration check governs the agent path too.
func TestRun_ReviewerRequestChangesNoOnFailProceeds(t *testing.T) {
	const noOnFail = `{
	  "stage": "build",
	  "phases": [
	    {"name": "implementer", "agent": "implementer", "readonly": false, "required_gates": []},
	    {"name": "reviewer", "agent": "reviewer", "readonly": true, "required_gates": []},
	    {"name": "qa", "agent": "qa", "readonly": true, "required_gates": []}
	  ],
	  "stop_condition": {"type": "external", "all_of": [], "anti_pattern": "round_count"}
	}`
	wf, err := asset.LoadWorkflowJSON([]byte(noOnFail))
	if err != nil {
		t.Fatalf("load no-on_fail fixture: %v", err)
	}
	rec := &recorder{}
	fv := &flakyVerdict{phase: "reviewer", changesUntil: 99} // would loop if a target existed
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log, MaxLoopBack: 3, AgentVerdict: fv.verdict}

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("a REQUEST_CHANGES with no on_fail target must proceed, not abort; got %v", err)
	}
	if !contains(rec.executed, "qa") {
		t.Errorf("with no on_fail the run must proceed to qa; executed=%v", rec.executed)
	}
	if containsLine(rec.logs, "loop-back") {
		t.Errorf("no on_fail must log no loop-back even on REQUEST_CHANGES; logs=%v", rec.logs)
	}
}
