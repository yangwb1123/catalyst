package orchestrator

import (
	"testing"

	"forgeos/forge-core/internal/asset"
)

// Uses a real subprocess (echo) to prove the executor actually runs a command
// and captures its output — not a mock.
func TestCommandExecutor_RunsRealProcess(t *testing.T) {
	rec := &recorder{}
	ex := CommandExecutor{
		Build: func(p asset.Phase, mode string) []string { return []string{"echo", p.Name, mode} },
		Log:   rec.log,
	}
	if err := ex.Execute(asset.Phase{Name: "planner"}, "balanced"); err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if !containsLine(rec.logs, "planner balanced") {
		t.Errorf("expected the command's output to be captured; logs=%v", rec.logs)
	}
}

// A non-zero exit must surface as an error — fail closed, never silent success.
func TestCommandExecutor_FailingCommandErrors(t *testing.T) {
	ex := CommandExecutor{Build: func(asset.Phase, string) []string { return []string{"false"} }}
	if err := ex.Execute(asset.Phase{Name: "x"}, "m"); err == nil {
		t.Error("a non-zero exit must return an error (fail closed)")
	}
}

// An empty argv is a misconfiguration and must fail closed, not no-op.
func TestCommandExecutor_EmptyArgvErrors(t *testing.T) {
	ex := CommandExecutor{Build: func(asset.Phase, string) []string { return nil }}
	if err := ex.Execute(asset.Phase{Name: "x"}, "m"); err == nil {
		t.Error("empty argv must fail closed")
	}
}

// P22: a nil Build must return an error, never panic on the nil call.
func TestCommandExecutor_NilBuildFailsClosed(t *testing.T) {
	ex := CommandExecutor{Build: nil}
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("nil Build must not panic; got %v", r)
		}
	}()
	if err := ex.Execute(asset.Phase{Name: "x"}, "m"); err == nil {
		t.Error("a nil Build must return an error (fail closed)")
	}
}
