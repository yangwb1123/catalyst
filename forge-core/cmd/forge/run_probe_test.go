package main

import (
	"context"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
)

type probeMutationExecutor func()

func (f probeMutationExecutor) Execute(context.Context, asset.Phase, string) error {
	f()
	return nil
}

func TestRunProbeRefreshesAfterAgentMutationAndConvergenceReusesIt(t *testing.T) {
	tests := []struct {
		name, before, after string
	}{
		{"old PASS new FAIL", gate.StatusPass, gate.StatusFail},
		{"old FAIL new PASS", gate.StatusFail, gate.StatusPass},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			state, calls := tc.before, 0
			p := newRunProbe(t.TempDir())
			p.load = func(string) (map[string]string, map[string]string, error) {
				calls++
				return map[string]string{"lint": state}, map[string]string{"lint": "applicable"}, nil
			}

			if got := p.runGate("lint").Status; got != tc.before {
				t.Fatalf("initial gate status = %q, want old %q", got, tc.before)
			}
			exec := runProbeExecutor{next: probeMutationExecutor(func() { state = tc.after }), probe: p}
			if err := exec.Execute(context.Background(), asset.Phase{Name: "implementer"}, "engineering"); err != nil {
				t.Fatalf("agent execution: %v", err)
			}
			if got := p.runGate("lint").Status; got != tc.after {
				t.Fatalf("post-agent gate status = %q, want fresh %q", got, tc.after)
			}
			statuses, _ := p.current()
			if got := statuses["lint"]; got != tc.after {
				t.Errorf("convergence snapshot = %q, want gate snapshot %q", got, tc.after)
			}
			if calls != 2 {
				t.Errorf("probe calls = %d, want 2 (old gate + one post-write refresh; convergence must reuse)", calls)
			}
		})
	}
}

func TestRunProbeConvergenceRefreshesWhenAgentRunsAfterLastGate(t *testing.T) {
	state, calls := gate.StatusPass, 0
	p := newRunProbe(t.TempDir())
	p.load = func(string) (map[string]string, map[string]string, error) {
		calls++
		return map[string]string{"lint": state}, nil, nil
	}
	if got := p.runGate("lint").Status; got != gate.StatusPass {
		t.Fatalf("initial gate status = %q, want PASS", got)
	}
	exec := runProbeExecutor{
		next:  probeMutationExecutor(func() { state = gate.StatusFail }),
		probe: p,
	}
	if err := exec.Execute(context.Background(), asset.Phase{Name: "reviewer"}, "engineering"); err != nil {
		t.Fatalf("agent execution: %v", err)
	}
	statuses, _ := p.current()
	if got := statuses["lint"]; got != gate.StatusFail {
		t.Errorf("convergence status = %q, want fresh FAIL after final agent", got)
	}
	if calls != 2 {
		t.Errorf("probe calls = %d, want initial gate plus one pre-convergence refresh", calls)
	}
}

func TestRunProbeModeFilteredGateSetDrivesConvergence(t *testing.T) {
	const all = "lint,test,build,complexity,arch,security"
	wf, err := asset.LoadWorkflowJSON([]byte(`{
		"stage":"build",
		"phases":[{"name":"gates","agent":"harness","required_gates":[` +
		`"lint","test","build","complexity","arch","security"]}],
		"stop_condition":{"type":"conjunction","all_of":[
			{"metric":"gates_status","operator":"==","value":"green"}]}
	}`))
	if err != nil {
		t.Fatalf("load workflow %s: %v", all, err)
	}
	calls := 0
	p := newRunProbe(t.TempDir())
	p.load = func(string) (map[string]string, map[string]string, error) {
		calls++
		return map[string]string{
			"lint": gate.StatusPass, "build": gate.StatusPass,
			"test_pass": gate.StatusFail, "app_test_pass": gate.StatusFail,
			"security_findings": gate.StatusFail,
		}, nil, nil
	}
	eng := orchestrator.Engine{
		RunGate: p.runGate, Log: func(string) {},
		ModePolicy: mode.Effective("explorer", "idea"),
	}
	if err := eng.Run(wf, "explorer"); err != nil {
		t.Fatalf("mode-filtered run: %v", err)
	}
	statuses, categories := p.current()
	actual := p.actualGates()
	if len(actual) != 2 || actual[0] != "lint" || actual[1] != "build" {
		t.Fatalf("actual gates = %v, want [lint build]", actual)
	}
	sig := gatherSignals(p.root, wf, statuses, categories, "idea", false, nil, actual)
	if !sig.GatesGreen {
		t.Error("convergence must be green from executed [lint build]; excluded failing gates must not reappear")
	}
	if calls != 1 {
		t.Errorf("probe calls = %d, want 1 shared by gate execution and convergence", calls)
	}
}
