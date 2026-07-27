package orchestrator

import (
	"context"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

func TestRunFromRejectsAmbiguousWorkflowBeforeExecution(t *testing.T) {
	executions := 0
	engine := Engine{Exec: execFunc(func(context.Context, asset.Phase, string) error {
		executions++
		return nil
	})}
	workflow := asset.Workflow{Stage: "build", Phases: []asset.Phase{
		{Name: "same", Agent: "implementer"},
		{Name: "same", Agent: "reviewer"},
	}}

	err := engine.RunFrom(workflow, "balanced", 0)
	if err == nil || !strings.Contains(err.Error(), `duplicates phase name "same"`) {
		t.Fatalf("RunFrom error = %v, want duplicate phase identity", err)
	}
	if executions != 0 {
		t.Fatalf("malformed serial workflow executed %d phases, want 0", executions)
	}
}

func TestRunParallelRejectsAmbiguousWorkflowBeforeExecution(t *testing.T) {
	executions := 0
	engine := Engine{Exec: execFunc(func(context.Context, asset.Phase, string) error {
		executions++
		return nil
	})}
	workflow := asset.Workflow{Stage: "discover", Phases: []asset.Phase{{
		Name: "scan", Agent: "explorer", Emits: []string{"b.md", "a/../b.md"},
	}}}

	err := engine.RunParallel(context.Background(), workflow, "balanced")
	if err == nil || !strings.Contains(err.Error(), "duplicates normalized target") {
		t.Fatalf("RunParallel error = %v, want duplicate emit identity", err)
	}
	if executions != 0 {
		t.Fatalf("malformed parallel workflow executed %d phases, want 0", executions)
	}
}
