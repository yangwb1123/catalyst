package orchestrator

import (
	"errors"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

func requireReviewer(p asset.Phase) bool {
	return p.Name == "reviewer"
}

func TestRun_RequiredVerdictMissingFailsClosed(t *testing.T) {
	wf := loadVerdict(t)
	rec := &recorder{}
	eng := Engine{
		Exec: rec.executor(), RunGate: allOK, Log: rec.log, MaxLoopBack: 3,
		AgentVerdict:        func(string) (string, bool) { return "", false },
		RequireAgentVerdict: requireReviewer,
	}

	err := eng.Run(wf, "balanced")
	if err == nil || !strings.Contains(err.Error(), "missing or malformed") {
		t.Fatalf("required missing verdict error = %v", err)
	}
	if contains(rec.executed, "qa") {
		t.Fatalf("required missing verdict proceeded to qa: %v", rec.executed)
	}
}

func TestRun_RequiredVerdictUnknownFailsClosed(t *testing.T) {
	wf := loadVerdict(t)
	rec := &recorder{}
	eng := Engine{
		Exec: rec.executor(), RunGate: allOK, MaxLoopBack: 3,
		AgentVerdict:        func(phase string) (string, bool) { return "MAYBE", phase == "reviewer" },
		RequireAgentVerdict: requireReviewer,
	}

	err := eng.Run(wf, "balanced")
	if err == nil || !strings.Contains(err.Error(), `unsupported required agent verdict "MAYBE"`) {
		t.Fatalf("required unknown verdict error = %v", err)
	}
	if contains(rec.executed, "qa") {
		t.Fatalf("required unknown verdict proceeded to qa: %v", rec.executed)
	}
}

func TestRun_RequiredRequestChangesExhaustionFailsClosed(t *testing.T) {
	wf := loadVerdict(t)
	rec := &recorder{}
	fv := &flakyVerdict{phase: "reviewer", changesUntil: 99}
	eng := Engine{
		Exec: rec.executor(), RunGate: allOK, Log: rec.log, MaxLoopBack: 1,
		AgentVerdict: fv.verdict, RequireAgentVerdict: requireReviewer,
	}

	err := eng.Run(wf, "balanced")
	if err == nil || !strings.Contains(err.Error(), "could not take its required directed loop-back") {
		t.Fatalf("required exhausted verdict error = %v", err)
	}
	if contains(rec.executed, "qa") {
		t.Fatalf("required exhausted verdict proceeded to qa: %v", rec.executed)
	}
	if !containsLine(rec.logs, "aborting (fail-closed)") {
		t.Fatalf("required exhaustion did not log fail-closed outcome: %v", rec.logs)
	}
}

func TestRun_RequiredRequestChangesThenApproveCompletes(t *testing.T) {
	wf := loadVerdict(t)
	rec := &recorder{}
	fv := &flakyVerdict{phase: "reviewer", changesUntil: 1}
	eng := Engine{
		Exec: rec.executor(), RunGate: allOK, MaxLoopBack: 2,
		AgentVerdict: fv.verdict, RequireAgentVerdict: requireReviewer,
	}

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("required verdict should heal then approve: %v", err)
	}
	if !contains(rec.executed, "qa") {
		t.Fatalf("approved required verdict did not proceed: %v", rec.executed)
	}
}

func TestRun_RequiredApproveEvidenceCommitIsEnforced(t *testing.T) {
	wf := loadVerdict(t)
	rec := &recorder{}
	commits := 0
	eng := Engine{
		Exec: rec.executor(), RunGate: allOK,
		AgentVerdict: func(phase string) (string, bool) {
			return reviewerApprove, phase == "reviewer"
		},
		RequireAgentVerdict: requireReviewer,
		OnRequiredVerdictApproved: func(asset.Phase) error {
			commits++
			return errors.New("receipt unavailable")
		},
	}
	err := eng.Run(wf, "balanced")
	if err == nil || !strings.Contains(err.Error(), "commit required approval evidence") {
		t.Fatalf("required approval evidence error = %v", err)
	}
	if commits != 1 || contains(rec.executed, "qa") {
		t.Fatalf("evidence failure commits=%d executed=%v", commits, rec.executed)
	}
}

func TestRun_StrictQAContractIsIntrinsicWithoutCallerRequirementHook(t *testing.T) {
	qa := asset.Phase{
		Name: "qa", Agent: "qa", VerdictContract: asset.VerdictContractQAV1,
		RequiredGates: []string{"test"},
		OnFail:        &asset.OnFail{Action: "loop_back", TargetPhase: "implementer"},
	}
	wf := asset.Workflow{Stage: "build", Phases: []asset.Phase{
		{Name: "implementer", Agent: "implementer"}, qa,
		{Name: "after-qa", Agent: "observer"},
	}}
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK}

	err := eng.Run(wf, "balanced")
	if err == nil || !strings.Contains(err.Error(), "required agent verdict is unavailable") {
		t.Fatalf("intrinsic strict-QA error = %v", err)
	}
	if contains(rec.executed, "after-qa") {
		t.Fatalf("strict QA without a caller hook failed open: %v", rec.executed)
	}
}
