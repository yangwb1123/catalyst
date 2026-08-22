package orchestrator

import (
	"context"
	"errors"
	"reflect"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
)

func TestRequiredVerdictValidationFailureStopsApproveAndLoopBack(t *testing.T) {
	for _, token := range []string{reviewerApprove, reviewerRequestChanges} {
		t.Run(token, func(t *testing.T) {
			wf := loadVerdict(t)
			rec := &recorder{}
			cause := errors.New("bound receipt is stale")
			validated, completed, committed := 0, 0, 0
			engine := Engine{
				Exec: rec.executor(), RunGate: allOK, MaxLoopBack: 1,
				AgentVerdict: func(phase string) (string, bool) {
					return token, phase == "reviewer"
				},
				RequireAgentVerdict: requireReviewer,
				ValidateAgentVerdict: func(phase asset.Phase, got string) error {
					validated++
					if phase.Name != "reviewer" || got != token {
						t.Fatalf("verdict validation = %s %q", phase.Name, got)
					}
					return cause
				},
				PhaseComplete: func(asset.Phase) error {
					completed++
					return nil
				},
				OnRequiredVerdictApproved: func(asset.Phase) error {
					committed++
					return nil
				},
			}
			err := engine.RunFrom(wf, "balanced", 2)
			assertValidationError(t, err, validationAgentVerdict, cause)
			if validated != 1 || completed != 1 || committed != 0 {
				t.Fatalf("validated=%d completed=%d committed=%d", validated, completed, committed)
			}
			if strings.Join(rec.executed, ",") != "reviewer" {
				t.Fatalf("validation failure executed downstream: %v", rec.executed)
			}
		})
	}
}

func TestRequiredRequestChangesValidatesAndCompletesBeforeLoopBack(t *testing.T) {
	wf := loadVerdict(t)
	var events []string
	verdictCalls := 0
	engine := Engine{
		Exec: execFunc(func(_ context.Context, phase asset.Phase, _ string) error {
			events = append(events, "exec:"+phase.Name)
			return nil
		}),
		RunGate: allOK, MaxLoopBack: 1, RequireAgentVerdict: requireReviewer,
		AgentVerdict: func(phase string) (string, bool) {
			if phase != "reviewer" {
				return "", false
			}
			verdictCalls++
			if verdictCalls == 1 {
				return reviewerRequestChanges, true
			}
			return reviewerApprove, true
		},
		ValidateAgentVerdict: func(_ asset.Phase, token string) error {
			events = append(events, "validate:"+token)
			return nil
		},
		PhaseStart: func(phase asset.Phase) error {
			events = append(events, "phase-start:"+phase.Name)
			return nil
		},
		PhaseComplete: func(phase asset.Phase) error {
			events = append(events, "phase-complete:"+phase.Name)
			return nil
		},
	}
	if err := engine.RunFrom(wf, "balanced", 2); err != nil {
		t.Fatal(err)
	}
	wantPrefix := []string{
		"phase-start:reviewer", "exec:reviewer", "phase-complete:reviewer",
		"validate:REQUEST_CHANGES", "phase-start:implementer", "exec:implementer",
	}
	if len(events) < len(wantPrefix) || strings.Join(events[:len(wantPrefix)], ",") != strings.Join(wantPrefix, ",") {
		t.Fatalf("runtime boundary order = %v, want prefix %v", events, wantPrefix)
	}
	if verdictCalls != 2 || !contains(events, "validate:APPROVE") {
		t.Fatalf("verdict events=%v calls=%d", events, verdictCalls)
	}
}

func TestPhaseBoundariesWrapGateAgentButNotSkip(t *testing.T) {
	wf := asset.Workflow{Stage: "build", Phases: []asset.Phase{
		{Name: "agent", Agent: "worker"},
		{Name: "gate", Agent: "harness", RequiredGates: []string{"lint"}},
		{Name: "skip", Agent: "reviewer", RequiredWhen: "../policies/modes.yml#workflow_depth.reviewer"},
		{Name: "after", Agent: "worker"},
	}}
	var events []string
	engine := Engine{
		Exec: execFunc(func(_ context.Context, phase asset.Phase, _ string) error {
			events = append(events, "exec:"+phase.Name)
			return nil
		}),
		RunGate: func(name string) gate.Result {
			events = append(events, "gate:"+name)
			return gate.Result{Name: name, OK: true}
		},
		ModePolicy: mode.Effective("explorer", "idea"),
		PhaseStart: func(phase asset.Phase) error {
			events = append(events, "phase-start:"+phase.Name)
			return nil
		},
		ValidateAgentSpawn: func(phase asset.Phase) error {
			events = append(events, "agent-spawn:"+phase.Name)
			return nil
		},
		PhaseComplete: func(phase asset.Phase) error {
			events = append(events, "phase-complete:"+phase.Name)
			return nil
		},
	}
	if err := engine.Run(wf, "explorer"); err != nil {
		t.Fatal(err)
	}
	want := []string{
		"phase-start:agent", "agent-spawn:agent", "exec:agent", "phase-complete:agent",
		"phase-start:gate", "gate:lint", "phase-complete:gate",
		"phase-start:after", "agent-spawn:after", "exec:after", "phase-complete:after",
	}
	if strings.Join(events, ",") != strings.Join(want, ",") {
		t.Fatalf("runtime events = %v, want %v", events, want)
	}
}

func TestAgentSpawnValidationSeesGateMutationAndPreventsExecution(t *testing.T) {
	cause := errors.New("approval receipt became stale")
	mutated := false
	var events []string
	engine := Engine{
		Exec: execFunc(func(context.Context, asset.Phase, string) error {
			events = append(events, "exec")
			return nil
		}),
		RunGate: func(name string) gate.Result {
			events = append(events, "gate:"+name)
			mutated = true
			return gate.Result{Name: name, OK: true}
		},
		PhaseStart: func(asset.Phase) error {
			events = append(events, "phase-start")
			return nil
		},
		ValidateAgentSpawn: func(asset.Phase) error {
			events = append(events, "agent-spawn")
			if mutated {
				return cause
			}
			return nil
		},
	}
	wf := asset.Workflow{Stage: "build", Phases: []asset.Phase{{
		Name: "current", Agent: "worker", RequiredGates: []string{"test"},
	}}}
	err := engine.Run(wf, "balanced")
	assertValidationError(t, err, validationAgentSpawn, cause)
	want := []string{"phase-start", "gate:test", "agent-spawn"}
	if !reflect.DeepEqual(events, want) {
		t.Fatalf("pre-spawn ordering/execution = %v, want %v", events, want)
	}
}

func TestAgentSpawnValidationRunsBeforeEveryRetryAttempt(t *testing.T) {
	validations, executions := 0, 0
	engine := Engine{
		MaxRetries: 1,
		ValidateAgentSpawn: func(asset.Phase) error {
			validations++
			return nil
		},
		Exec: execFunc(func(context.Context, asset.Phase, string) error {
			executions++
			if executions == 1 {
				return &ExecError{Kind: KindTimeout, Err: context.DeadlineExceeded}
			}
			return nil
		}),
	}
	wf := asset.Workflow{Stage: "build", Phases: []asset.Phase{{Name: "agent", Agent: "worker"}}}
	if err := engine.Run(wf, "balanced"); err != nil {
		t.Fatal(err)
	}
	if validations != 2 || executions != 2 {
		t.Fatalf("validations/executions = %d/%d, want 2/2", validations, executions)
	}
}

func TestWorkflowCompleteValidationFailsBeforeStopReport(t *testing.T) {
	cause := errors.New("terminal receipt is stale")
	stopReported := false
	engine := Engine{
		Exec: recExecutor(),
		WorkflowComplete: func(wf asset.Workflow) error {
			if wf.Stage != "build" {
				t.Fatalf("workflow stage = %q", wf.Stage)
			}
			return cause
		},
		Log: func(line string) {
			if strings.HasPrefix(line, "stop:") {
				stopReported = true
			}
		},
	}
	wf := asset.Workflow{Stage: "build", Phases: []asset.Phase{{Name: "agent", Agent: "worker"}}}
	err := engine.Run(wf, "balanced")
	var failure *ExecError
	if !errors.As(err, &failure) || failure.Kind != KindRuntimeValidation || failure.Phase != "build" {
		t.Fatalf("workflow completion error = %#v (%v)", failure, err)
	}
	if !errors.Is(err, cause) || !strings.Contains(err.Error(), string(validationWorkflowComplete)) {
		t.Fatalf("workflow completion error = %v", err)
	}
	if stopReported {
		t.Fatal("failed workflow completion validation reported a successful stop")
	}
}

func TestPhaseBoundaryFailureStopsAllDownstreamExecution(t *testing.T) {
	tests := []struct {
		name      string
		phase     asset.Phase
		boundary  string
		label     runtimeValidationBoundary
		wantExec  int
		wantGates int
	}{
		{name: "agent start", phase: asset.Phase{Name: "current", Agent: "worker"}, boundary: "start", label: validationPhaseStart},
		{name: "agent complete", phase: asset.Phase{Name: "current", Agent: "worker"}, boundary: "complete", label: validationPhaseComplete, wantExec: 1},
		{name: "gate start", phase: asset.Phase{Name: "current", Agent: "harness", RequiredGates: []string{"test"}}, boundary: "start", label: validationPhaseStart},
		{name: "gate complete", phase: asset.Phase{Name: "current", Agent: "harness", RequiredGates: []string{"test"}}, boundary: "complete", label: validationPhaseComplete, wantGates: 1},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			runBoundaryFailureCase(t, test.phase, test.boundary, test.label, test.wantExec, test.wantGates)
		})
	}
}

func runBoundaryFailureCase(
	t *testing.T,
	phase asset.Phase,
	boundary string,
	label runtimeValidationBoundary,
	wantExec, wantGates int,
) {
	t.Helper()
	cause := errors.New("source freshness changed")
	executions, gates := 0, 0
	engine := Engine{
		Exec: execFunc(func(context.Context, asset.Phase, string) error {
			executions++
			return nil
		}),
		RunGate: func(name string) gate.Result {
			gates++
			return gate.Result{Name: name, OK: true}
		},
		PhaseStart: func(got asset.Phase) error {
			if got.Name == phase.Name && boundary == "start" {
				return cause
			}
			return nil
		},
		PhaseComplete: func(got asset.Phase) error {
			if got.Name == phase.Name && boundary == "complete" {
				return cause
			}
			return nil
		},
	}
	wf := asset.Workflow{Stage: "build", Phases: []asset.Phase{
		phase, {Name: "downstream", Agent: "worker"},
	}}
	err := engine.Run(wf, "balanced")
	assertValidationError(t, err, label, cause)
	if executions != wantExec || gates != wantGates {
		t.Fatalf("executions=%d gates=%d, want %d/%d", executions, gates, wantExec, wantGates)
	}
}

func TestRunParallelRejectsRuntimeHooksBeforeAnyExecution(t *testing.T) {
	tests := []struct {
		name      string
		configure func(*Engine)
	}{
		{name: "phase start", configure: func(e *Engine) {
			e.PhaseStart = func(asset.Phase) error { return nil }
		}},
		{name: "phase complete", configure: func(e *Engine) {
			e.PhaseComplete = func(asset.Phase) error { return nil }
		}},
		{name: "agent spawn", configure: func(e *Engine) {
			e.ValidateAgentSpawn = func(asset.Phase) error { return nil }
		}},
		{name: "agent verdict", configure: func(e *Engine) {
			e.ValidateAgentVerdict = func(asset.Phase, string) error { return nil }
		}},
		{name: "workflow complete", configure: func(e *Engine) {
			e.WorkflowComplete = func(asset.Workflow) error { return nil }
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			executions, gates := 0, 0
			engine := Engine{
				Exec: execFunc(func(context.Context, asset.Phase, string) error {
					executions++
					return nil
				}),
				RunGate: func(gateName string) gate.Result {
					gates++
					return gate.Result{Name: gateName, OK: true}
				},
			}
			test.configure(&engine)
			wf := parallelWF(
				asset.Phase{Name: "gate", Agent: "harness", RequiredGates: []string{"test"}},
				asset.Phase{Name: "agent", Agent: "worker"},
			)
			err := engine.RunParallel(context.Background(), wf, "balanced")
			if err == nil || !strings.Contains(err.Error(), "require serial orchestration") {
				t.Fatalf("parallel hook error = %v", err)
			}
			if executions != 0 || gates != 0 {
				t.Fatalf("parallel rejection executed agents=%d gates=%d", executions, gates)
			}
		})
	}
}

func TestStrictQAVerdictUsesValidationHookWithoutExternalRequirement(t *testing.T) {
	qa := asset.Phase{
		Name: "qa", Agent: "qa", VerdictContract: asset.VerdictContractQAV1,
		RequiredGates: []string{"test"},
		OnFail:        &asset.OnFail{Action: "loop_back", TargetPhase: "implementer"},
	}
	wf := asset.Workflow{Stage: "build", Phases: []asset.Phase{
		{Name: "implementer", Agent: "implementer"}, qa,
	}}
	validated := 0
	engine := Engine{
		Exec: recExecutor(), RunGate: allOK,
		AgentVerdict: func(phase string) (string, bool) {
			return reviewerApprove, phase == "qa"
		},
		ValidateAgentVerdict: func(phase asset.Phase, token string) error {
			if phase.Name != "qa" || token != reviewerApprove {
				t.Fatalf("strict QA validation = %s %q", phase.Name, token)
			}
			validated++
			return nil
		},
	}
	if err := engine.Run(wf, "balanced"); err != nil {
		t.Fatal(err)
	}
	if validated != 1 {
		t.Fatalf("strict QA validations = %d", validated)
	}
}

func recExecutor() AgentExecutor {
	return execFunc(func(context.Context, asset.Phase, string) error { return nil })
}

func assertValidationError(
	t *testing.T,
	err error,
	label runtimeValidationBoundary,
	cause error,
) {
	t.Helper()
	var failure *ExecError
	if !errors.As(err, &failure) || failure.Kind != KindRuntimeValidation || failure.Phase != "current" && failure.Phase != "reviewer" {
		t.Fatalf("runtime validation error = %#v (%v)", failure, err)
	}
	if !strings.Contains(err.Error(), string(label)) {
		t.Fatalf("runtime validation error = %v, want boundary %s", err, label)
	}
	if !errors.Is(err, cause) {
		t.Fatalf("runtime validation error did not wrap cause: %v", err)
	}
}
