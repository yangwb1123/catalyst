package orchestrator

import (
	"context"
	"errors"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
)

func TestCommandExecutorDispatchesHostOnce(t *testing.T) {
	t.Setenv(agentDepthEnv, "")
	var phases []string
	executor := CommandExecutor{
		Build:      func(asset.Phase, string) []string { return []string{"true"} },
		OnDispatch: func(phase string) { phases = append(phases, phase) },
	}
	if err := executor.Execute(context.Background(), asset.Phase{Name: "implement"}, "balanced"); err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if len(phases) != 1 || phases[0] != "implement" {
		t.Fatalf("dispatch phases = %v, want [implement]", phases)
	}
}

func TestCommandExecutorDispatchesSandboxOnceBeforeRunner(t *testing.T) {
	runner := &fakeRunner{output: "done"}
	dispatches := 0
	executor := sandboxedExecutor(runner)
	executor.OnDispatch = func(phase string) {
		dispatches++
		if phase != "isolated" || runner.calls != 0 {
			t.Fatalf("dispatch phase=%q runner calls=%d", phase, runner.calls)
		}
	}
	if err := executor.Execute(context.Background(), asset.Phase{Name: "isolated"}, "balanced"); err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if dispatches != 1 || runner.calls != 1 {
		t.Fatalf("dispatches=%d runner calls=%d, want 1/1", dispatches, runner.calls)
	}
}

func TestCommandExecutorRefusalsDoNotDispatch(t *testing.T) {
	invalidSandbox := sandboxedExecutor(&fakeRunner{output: "unused"})
	invalidSandbox.MaxOutputBytes = -1
	cases := []struct {
		name string
		exec CommandExecutor
	}{
		{"nil-build", CommandExecutor{}},
		{"empty-argv", CommandExecutor{Build: func(asset.Phase, string) []string { return nil }}},
		{"invalid-output-cap", CommandExecutor{Build: func(asset.Phase, string) []string { return []string{"true"} }, MaxOutputBytes: -1}},
		{"invalid-sandbox-output-cap", invalidSandbox},
		{"config", CommandExecutor{ValidateConfig: func(asset.Phase, string) error { return errors.New("denied") }, Build: func(asset.Phase, string) []string { return []string{"true"} }}},
		{"finalize", CommandExecutor{Build: func(asset.Phase, string) []string { return []string{"true"} }, FinalizeCommand: func(asset.Phase, string, []string) ([]string, error) { return nil, errors.New("stale binding") }}},
		{"input", CommandExecutor{Build: func(asset.Phase, string) []string { return []string{"agent", "prompt"} }, PromptViaStdin: true}},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			calls := 0
			test.exec.OnDispatch = func(string) { calls++ }
			if err := test.exec.Execute(context.Background(), asset.Phase{Name: "blocked"}, "balanced"); err == nil {
				t.Fatal("refused command unexpectedly succeeded")
			}
			if calls != 0 {
				t.Fatalf("refused command dispatched %d times", calls)
			}
		})
	}
}

func TestCommandExecutorRestoreDoesNotDispatch(t *testing.T) {
	dispatches := 0
	executor := CommandExecutor{
		OnDispatch:     func(string) { dispatches++ },
		ValidateOutput: func(string, string) error { return nil },
	}
	if err := executor.RestoreValidatedOutput(asset.Phase{Name: "restored"}, "durable output"); err != nil {
		t.Fatal(err)
	}
	if dispatches != 0 {
		t.Fatalf("restored output dispatched %d commands", dispatches)
	}
}

func TestCommandExecutorRecursionRefusalDoesNotDispatch(t *testing.T) {
	t.Setenv(agentDepthEnv, "1")
	calls := 0
	executor := CommandExecutor{
		Build: func(asset.Phase, string) []string { return []string{"true"} }, MaxDepth: 1,
		OnDispatch: func(string) { calls++ },
	}
	if err := executor.Execute(context.Background(), asset.Phase{Name: "recursive"}, "balanced"); err == nil {
		t.Fatal("recursion refusal unexpectedly succeeded")
	}
	if calls != 0 {
		t.Fatalf("recursion refusal dispatched %d times", calls)
	}
}

func TestCommandExecutorRetryDispatchesEveryAttempt(t *testing.T) {
	t.Setenv(agentDepthEnv, "")
	dispatches := 0
	executor := CommandExecutor{
		Build:            func(asset.Phase, string) []string { return []string{"false"} },
		ClassifyOverload: func(string) bool { return true },
		OnDispatch:       func(string) { dispatches++ },
	}
	engine := Engine{Exec: executor, MaxRetries: 2, Sleep: func(time.Duration) {}}
	wf := asset.Workflow{Stage: "build", Phases: []asset.Phase{{Name: "implement", Agent: "implementer"}}}
	if err := engine.Run(wf, "balanced"); err == nil {
		t.Fatal("persistent overload unexpectedly succeeded")
	}
	if dispatches != 3 {
		t.Fatalf("dispatches=%d, want one per three actual attempts", dispatches)
	}
}
