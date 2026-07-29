package orchestrator

import (
	"context"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

func TestRunParallel_RejectsStrictQABeforeAnyExecution(t *testing.T) {
	executions := 0
	engine := Engine{Exec: execFunc(func(context.Context, asset.Phase, string) error {
		executions++
		return nil
	})}
	workflow := asset.Workflow{Stage: "build", Phases: []asset.Phase{
		{Name: "implementer", Agent: "implementer"},
		{
			Name: "qa", Agent: "qa", DependsOn: []string{"implementer"},
			VerdictContract: asset.VerdictContractQAV1,
			RequiredGates:   []string{"test"},
			OnFail:          &asset.OnFail{Action: "loop_back", TargetPhase: "implementer"},
		},
	}}

	err := engine.RunParallel(context.Background(), workflow, "balanced")
	if err == nil || !strings.Contains(err.Error(),
		`phase qa: verdict_contract "qa_v1" requires serial directed loop-back orchestration`) {
		t.Fatalf("RunParallel error = %v", err)
	}
	if executions != 0 {
		t.Fatalf("strict-QA parallel workflow executed %d phase(s), want 0", executions)
	}
}
