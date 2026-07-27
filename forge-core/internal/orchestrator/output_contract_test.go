package orchestrator

import (
	"context"
	"errors"
	"os/exec"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

func TestCommandExecutorValidateOutputFailsClosed(t *testing.T) {
	printf, err := exec.LookPath("printf")
	if err != nil {
		t.Skip("printf not available")
	}
	ex := CommandExecutor{
		Build: func(asset.Phase, string) []string { return []string{printf, "malformed"} },
		ValidateOutput: func(phase, output string) error {
			if output != "expected" {
				return errors.New("missing expected contract")
			}
			return nil
		},
	}
	err = ex.Execute(context.Background(), asset.Phase{Name: "planner"}, "balanced")
	if err == nil {
		t.Fatal("invalid output contract must fail the phase")
	}
	var execErr *ExecError
	if !errors.As(err, &execErr) || execErr.Kind != KindFailed {
		t.Fatalf("error = %T %v, want terminal KindFailed", err, err)
	}
	if !strings.Contains(err.Error(), "output contract") {
		t.Fatalf("error lacks contract context: %v", err)
	}
}

func TestCommandExecutorValidateOutputAcceptsValidOutput(t *testing.T) {
	printf, err := exec.LookPath("printf")
	if err != nil {
		t.Skip("printf not available")
	}
	ex := CommandExecutor{
		Build:          func(asset.Phase, string) []string { return []string{printf, "expected"} },
		ValidateOutput: func(_, output string) error { return nil },
	}
	if err := ex.Execute(context.Background(), asset.Phase{Name: "planner"}, "balanced"); err != nil {
		t.Fatalf("valid output: %v", err)
	}
}

func TestCommandExecutorRequestedSandboxFailsClosed(t *testing.T) {
	buildCalled := false
	ex := CommandExecutor{
		Build: func(asset.Phase, string) []string {
			buildCalled = true
			return []string{"true"}
		},
		Sandbox: &SandboxConfig{Type: "firecracker"},
	}
	err := ex.Execute(context.Background(), asset.Phase{Name: "implementer"}, "balanced")
	var execErr *ExecError
	if !errors.As(err, &execErr) || execErr.Kind != KindConfig {
		t.Fatalf("requested unavailable sandbox = %T %v, want KindConfig", err, err)
	}
	if buildCalled {
		t.Fatal("sandbox refusal must happen before command construction or host execution")
	}
	if !strings.Contains(err.Error(), "refusing host execution") {
		t.Fatalf("error must explain the fail-closed boundary: %v", err)
	}
}

func TestCommandExecutorExplicitNoSandboxRunsNormally(t *testing.T) {
	ex := CommandExecutor{
		Build:   func(asset.Phase, string) []string { return []string{"true"} },
		Sandbox: &SandboxConfig{Type: "none"},
	}
	if err := ex.Execute(context.Background(), asset.Phase{Name: "implementer"}, "balanced"); err != nil {
		t.Fatalf("explicit sandbox=none should select host execution: %v", err)
	}
}
