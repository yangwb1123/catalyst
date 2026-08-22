package orchestrator

import (
	"context"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/mode"
)

func requiredReviewerWorkflow() asset.Workflow {
	return asset.Workflow{Stage: "build", Phases: []asset.Phase{
		{Name: "implementer", Agent: "implementer"},
		{
			Name: "reviewer", Agent: "reviewer", Readonly: true, FreshContext: true,
			RequiredWhen:    "../policies/modes.yml#workflow_depth.reviewer",
			VerdictContract: asset.VerdictContractReviewerV1,
			OnFail:          &asset.OnFail{Action: "loop_back", TargetPhase: "implementer"},
		},
		{Name: "after", Agent: "observer"},
	}}
}

func TestRequiredVerdictCannotBeSkippedByMode(t *testing.T) {
	wf := requiredReviewerWorkflow()
	rec := &recorder{}
	eng := Engine{
		Exec: rec.executor(), RunGate: allOK, ModePolicy: mode.Effective("explorer", "idea"),
		RequireAgentVerdict: requireReviewer,
		AgentVerdict: func(phase string) (string, bool) {
			return reviewerApprove, phase == "reviewer"
		},
	}
	if err := eng.Run(wf, "explorer"); err != nil {
		t.Fatalf("required reviewer under explorer: %v", err)
	}
	if !contains(rec.executed, "reviewer") {
		t.Fatalf("explorer skipped required reviewer: %v", rec.executed)
	}
}

func TestRequiredVerdictCannotBeBypassedByRunFrom(t *testing.T) {
	wf := requiredReviewerWorkflow()
	rec := &recorder{}
	eng := Engine{
		Exec: rec.executor(), RunGate: allOK, RequireAgentVerdict: requireReviewer,
		AgentVerdict: func(string) (string, bool) { return reviewerApprove, true },
	}
	for _, start := range []int{2, len(wf.Phases)} {
		err := eng.RunFrom(wf, "balanced", start)
		if err == nil || !strings.Contains(err.Error(), "bypasses required verdict") {
			t.Errorf("RunFrom start=%d error=%v, want bypass rejection", start, err)
		}
	}
	if len(rec.executed) != 0 {
		t.Fatalf("bypassed RunFrom executed phases: %v", rec.executed)
	}
	if err := eng.RunFrom(wf, "balanced", 1); err != nil {
		t.Fatalf("RunFrom at required reviewer must remain valid: %v", err)
	}
}

func TestParallelRejectsExternallyRequiredVerdictBeforeExecution(t *testing.T) {
	wf := requiredReviewerWorkflow()
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, RequireAgentVerdict: requireReviewer}
	err := eng.RunParallel(context.Background(), wf, "balanced")
	if err == nil || !strings.Contains(err.Error(), "requires serial directed loop-back") {
		t.Fatalf("RunParallel required verdict error = %v", err)
	}
	if len(rec.executed) != 0 {
		t.Fatalf("RunParallel executed before required-verdict preflight: %v", rec.executed)
	}
}

func TestNilRequiredVerdictHookPreservesAdvisoryModeSkip(t *testing.T) {
	wf := requiredReviewerWorkflow()
	wf.Phases[1].VerdictContract = ""
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, ModePolicy: mode.Effective("explorer", "idea")}
	if err := eng.Run(wf, "explorer"); err != nil {
		t.Fatalf("legacy advisory explorer run: %v", err)
	}
	if contains(rec.executed, "reviewer") {
		t.Fatalf("legacy advisory reviewer no longer skips: %v", rec.executed)
	}
}
