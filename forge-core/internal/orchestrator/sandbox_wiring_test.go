package orchestrator

import (
	"context"
	"errors"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/orchestrator/sandbox"
)

// fakeRunner records the argv/timeout it received and returns scripted
// output, exit code, and error.
type fakeRunner struct {
	argv    []string
	timeout time.Duration
	output  string
	code    int
	err     error
	calls   int
}

func (f *fakeRunner) Run(
	_ context.Context,
	argv []string,
	timeout time.Duration,
) (string, int, error) {
	f.calls++
	f.argv = argv
	f.timeout = timeout
	return f.output, f.code, f.err
}

func sandboxedExecutor(runner sandbox.Runner) CommandExecutor {
	return CommandExecutor{
		Build: func(_ asset.Phase, _ string) []string {
			return []string{"agent", "-p", "prompt"}
		},
		Sandbox: &SandboxConfig{
			Type:       "firecracker",
			TimeoutSec: 7,
			Runner:     runner,
		},
	}
}

func TestSandboxRunnerReceivesArgvAndTimeout(t *testing.T) {
	runner := &fakeRunner{output: "guest output", code: 0}
	executor := sandboxedExecutor(runner)
	err := executor.Execute(context.Background(), asset.Phase{Name: "sandbox-phase"}, "run")
	if err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if runner.calls != 1 {
		t.Fatalf("runner calls = %d, want 1", runner.calls)
	}
	if got := strings.Join(runner.argv, " "); got != "agent -p prompt" {
		t.Fatalf("runner argv = %q", got)
	}
	if runner.timeout != 7*time.Second {
		t.Fatalf("runner timeout = %v, want 7s", runner.timeout)
	}
}

func TestSandboxCleanNonZeroExitIsKindFailed(t *testing.T) {
	runner := &fakeRunner{output: "agent said no", code: 3}
	executor := sandboxedExecutor(runner)
	err := executor.Execute(context.Background(), asset.Phase{Name: "sandbox-phase"}, "run")
	var execErr *ExecError
	if !errors.As(err, &execErr) {
		t.Fatalf("expected ExecError, got %v", err)
	}
	if execErr.Kind != KindFailed {
		t.Fatalf("kind = %v, want KindFailed", execErr.Kind)
	}
}

func TestSandboxInfraFaultIsKindConfig(t *testing.T) {
	runner := &fakeRunner{err: errors.New("sandbox firecracker: /dev/kvm unavailable")}
	executor := sandboxedExecutor(runner)
	err := executor.Execute(context.Background(), asset.Phase{Name: "sandbox-phase"}, "run")
	var execErr *ExecError
	if !errors.As(err, &execErr) {
		t.Fatalf("expected ExecError, got %v", err)
	}
	if execErr.Kind != KindConfig {
		t.Fatalf("kind = %v, want KindConfig", execErr.Kind)
	}
}

func TestSandboxTimeoutIsKindTimeout(t *testing.T) {
	runner := &fakeRunner{err: errors.New("firecracker guest timed out after 7s")}
	executor := sandboxedExecutor(runner)
	err := executor.Execute(context.Background(), asset.Phase{Name: "sandbox-phase"}, "run")
	var execErr *ExecError
	if !errors.As(err, &execErr) {
		t.Fatalf("expected ExecError, got %v", err)
	}
	if execErr.Kind != KindTimeout {
		t.Fatalf("kind = %v, want KindTimeout", execErr.Kind)
	}
}

func TestDeclaredSandboxWithoutRunnerFailsClosed(t *testing.T) {
	executor := CommandExecutor{
		Build: func(_ asset.Phase, _ string) []string {
			return []string{"agent"}
		},
		Sandbox: &SandboxConfig{Type: "firecracker"},
	}
	err := executor.Execute(context.Background(), asset.Phase{Name: "sandbox-phase"}, "run")
	var execErr *ExecError
	if !errors.As(err, &execErr) {
		t.Fatalf("expected ExecError, got %v", err)
	}
	if execErr.Kind != KindConfig {
		t.Fatalf("kind = %v, want KindConfig", execErr.Kind)
	}
	if !strings.Contains(err.Error(), "refusing host execution") {
		t.Fatalf("error must refuse host execution: %v", err)
	}
}
