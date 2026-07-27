package orchestrator

import (
	"context"
	"reflect"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/mode"
)

var proposalPhaseNames = []string{"scan", "gap-analysis", "roadmap-update"}

func evolvePolicyWorkflow(withDependencies bool) asset.Workflow {
	phases := []asset.Phase{
		{Name: "scan", Agent: "explorer", Readonly: true, Effect: evolveEffectObserve},
		{Name: "gap-analysis", Agent: "architect", Readonly: true, Effect: evolveEffectPropose},
		{Name: "roadmap-update", Agent: "planner", Readonly: true, Effect: evolveEffectPropose},
		{Name: "renamed-implementation-phase", Agent: "custom-writer", Effect: evolveEffectMutate},
		{Name: "harness-gates", Agent: "harness", Readonly: true, Effect: evolveEffectVerify, RequiredGates: []string{"test"}},
		{Name: "review", Agent: "reviewer", Readonly: true, Effect: evolveEffectVerify},
		{Name: "evaluate", Agent: "qa", Readonly: true, Effect: evolveEffectVerify},
	}
	if withDependencies {
		for i := 1; i < len(phases); i++ {
			phases[i].DependsOn = []string{phases[i-1].Name}
		}
	}
	return asset.Workflow{Stage: "evolve", Phases: phases, Stop: externalStop()}
}

func fullEvolveAgentNames() []string {
	return []string{
		"scan", "gap-analysis", "roadmap-update",
		"renamed-implementation-phase", "review", "evaluate",
	}
}

func TestRun_EvolveMachineCutoffPolicyMatrix(t *testing.T) {
	tests := []struct {
		name   string
		policy mode.Policy
		want   []string
		cutoff bool
	}{
		{"advisory", mode.Effective("cto", "idea"), proposalPhaseNames, true},
		{"opportunistic", mode.Effective("explorer", "idea"), proposalPhaseNames, true},
		{"build halt survives production", mode.Effective("cto", "production"), proposalPhaseNames, true},
		{"build halt survives unknown lifecycle", mode.Effective("cto", "typo"), proposalPhaseNames, true},
		{"explorer unknown lifecycle remains proposal-only", mode.Effective("explorer", "typo"), proposalPhaseNames, true},
		{"standard", mode.Effective("balanced", "mvp"), fullEvolveAgentNames(), false},
		{"thorough", mode.Effective("engineering", "mvp"), fullEvolveAgentNames(), false},
		{"production deepens but cannot grant explorer mutation authority", mode.Effective("explorer", "production"), proposalPhaseNames, true},
		{"zero policy back compatibility", mode.Policy{}, fullEvolveAgentNames(), false},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			rec := &recorder{}
			eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log, ModePolicy: tc.policy}
			if err := eng.Run(evolvePolicyWorkflow(false), tc.policy.Mode); err != nil {
				t.Fatalf("Run: %v", err)
			}
			if !reflect.DeepEqual(rec.executed, tc.want) {
				t.Errorf("executed = %v, want %v", rec.executed, tc.want)
			}
			if got := containsLine(rec.logs, "evolve stage cutoff"); got != tc.cutoff {
				t.Errorf("cutoff log present = %v, want %v; logs=%v", got, tc.cutoff, rec.logs)
			}
		})
	}
}

func TestRun_EvolveProposalBoundaryShapeFailsClosed(t *testing.T) {
	base := evolvePolicyWorkflow(false)
	tests := []struct {
		name string
		edit func(*asset.Workflow)
		want string
	}{
		{"missing mutate boundary", func(wf *asset.Workflow) {
			wf.Phases[3].Effect, wf.Phases[3].Readonly = evolveEffectVerify, true
		}, "no explicit effect=mutate"},
		{"multiple mutate boundaries", func(wf *asset.Workflow) {
			wf.Phases[4].Effect, wf.Phases[4].Readonly = evolveEffectMutate, false
		}, "multiple effect=mutate"},
		{"writable proposal phase", func(wf *asset.Workflow) {
			wf.Phases[2].Readonly = false
		}, "unrestricted writer"},
		{"missing effect", func(wf *asset.Workflow) {
			wf.Phases[0].Effect = ""
		}, "missing/unknown effect"},
		{"unknown effect", func(wf *asset.Workflow) {
			wf.Phases[0].Effect = "shell"
		}, "missing/unknown effect"},
		{"readonly mutation boundary", func(wf *asset.Workflow) {
			wf.Phases[3].Readonly = true
		}, "effect=mutate but is readonly"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			wf := base
			wf.Phases = append([]asset.Phase(nil), base.Phases...)
			tc.edit(&wf)
			assertInvalidEvolveBoundary(t, wf, tc.want)
		})
	}
}

func TestRun_EvolveProposalBoundaryRejectsWritesADR(t *testing.T) {
	wf := evolvePolicyWorkflow(false)
	wf.Phases[1].WritesADR = &asset.WritesADR{
		Condition: "mode in [cto]", Target: "src/",
	}
	rec := &recorder{}
	eng := Engine{
		Exec: rec.executor(), RunGate: allOK,
		ModePolicy: mode.Effective("cto", "idea"),
	}
	err := eng.Run(wf, "cto")
	if err == nil || !strings.Contains(err.Error(), "forbids directory-scoped writes_adr") {
		t.Fatalf("proposal writes_adr error = %v, want fail-closed rejection", err)
	}
	if len(rec.executed) != 0 {
		t.Fatalf("proposal writes_adr executed phases before rejection: %v", rec.executed)
	}
}

func TestRun_EvolveProposalBoundaryRejectsHostGates(t *testing.T) {
	wf := evolvePolicyWorkflow(false)
	wf.Phases[1].RequiredGates = []string{"test"}
	rec := &recorder{}
	eng := Engine{
		Exec: rec.executor(), RunGate: allOK,
		ModePolicy: mode.Effective("explorer", "idea"),
	}
	err := eng.Run(wf, "explorer")
	if err == nil || !strings.Contains(err.Error(), "forbids host required_gates") {
		t.Fatalf("proposal host-gate error = %v, want fail-closed rejection", err)
	}
	if len(rec.executed) != 0 {
		t.Fatalf("proposal host gate executed phases before rejection: %v", rec.executed)
	}
}

func assertInvalidEvolveBoundary(t *testing.T, wf asset.Workflow, want string) {
	t.Helper()
	for _, posture := range []struct {
		name   string
		mode   string
		policy mode.Policy
	}{
		{"proposal-only", "explorer", mode.Effective("explorer", "idea")},
		{"auto-act", "engineering", mode.Effective("engineering", "production")},
	} {
		t.Run(posture.name, func(t *testing.T) {
			rec := &recorder{}
			eng := Engine{Exec: rec.executor(), RunGate: allOK, ModePolicy: posture.policy}
			err := eng.Run(wf, posture.mode)
			if err == nil || !strings.Contains(err.Error(), want) {
				t.Fatalf("Run error = %v, want %q", err, want)
			}
			if len(rec.executed) != 0 {
				t.Fatalf("invalid boundary executed phases %v before failing closed", rec.executed)
			}
		})
	}
}

func TestRunParallel_EvolveCutoffMatchesSerialPolicy(t *testing.T) {
	for _, tc := range []struct {
		name   string
		policy mode.Policy
		want   []string
	}{
		{"build halt plus production", mode.Effective("cto", "production"), proposalPhaseNames},
		{"standard runs full loop", mode.Effective("balanced", "mvp"), fullEvolveAgentNames()},
	} {
		t.Run(tc.name, func(t *testing.T) {
			rec := &safeRec{}
			eng := Engine{Exec: rec, RunGate: allOK, ModePolicy: tc.policy}
			if err := eng.RunParallel(context.Background(), evolvePolicyWorkflow(true), tc.policy.Mode); err != nil {
				t.Fatalf("RunParallel: %v", err)
			}
			if !reflect.DeepEqual(rec.executed, tc.want) {
				t.Errorf("executed = %v, want %v", rec.executed, tc.want)
			}
		})
	}
}

func TestRunFrom_EvolveCutoffCannotResumeIntoImplementation(t *testing.T) {
	wf := evolvePolicyWorkflow(false)
	policy := mode.Effective("cto", "production")
	for _, tc := range []struct {
		name  string
		start int
		want  []string
		err   bool
	}{
		{"resume proposal", 2, []string{"roadmap-update"}, false},
		{"resume at boundary", 3, nil, false},
		{"resume after boundary rejected", 5, nil, true},
	} {
		t.Run(tc.name, func(t *testing.T) {
			rec := &recorder{}
			eng := Engine{Exec: rec.executor(), RunGate: allOK, ModePolicy: policy}
			err := eng.RunFrom(wf, "cto", tc.start)
			if tc.err {
				if err == nil || !strings.Contains(err.Error(), "outside executable range") {
					t.Fatalf("RunFrom error = %v, want executable-range rejection", err)
				}
				return
			}
			if err != nil {
				t.Fatalf("RunFrom: %v", err)
			}
			if !reflect.DeepEqual(rec.executed, tc.want) {
				t.Errorf("executed = %v, want %v", rec.executed, tc.want)
			}
		})
	}
}

func TestLoop_EvolveCutoffAppliesEverySerialAndParallelIteration(t *testing.T) {
	for _, parallel := range []bool{false, true} {
		t.Run(map[bool]string{false: "serial", true: "parallel"}[parallel], func(t *testing.T) {
			rec := &safeRec{}
			policy := mode.Effective("explorer", "idea")
			loop := NewLoopEngine(
				Engine{Exec: rec, RunGate: allOK, ModePolicy: policy},
				externalStop(),
				func() converge.Signals { return converge.Signals{} },
				2, 0, nil,
			)
			loop.Parallel = parallel
			out, err := loop.Run(evolvePolicyWorkflow(parallel), "explorer")
			if err != nil {
				t.Fatalf("Loop Run: %v", err)
			}
			if !out.Converged || out.Iterations != 2 ||
				(!strings.Contains(out.Reason, "safety bound") && !strings.Contains(out.Reason, "no gaps found")) {
				t.Fatalf("outcome = %+v, want clean external stop after two iterations", out)
			}
			want := append(append([]string{}, proposalPhaseNames...), proposalPhaseNames...)
			if !reflect.DeepEqual(rec.executed, want) {
				t.Errorf("executed = %v, want proposal prefix in each iteration %v", rec.executed, want)
			}
		})
	}
}
