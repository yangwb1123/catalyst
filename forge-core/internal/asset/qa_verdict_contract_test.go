package asset

import (
	"strings"
	"testing"
)

func strictQAPhase() Phase {
	return Phase{
		Name: "qa", Agent: "qa", VerdictContract: VerdictContractQAV1,
		RequiredGates: []string{"test"},
		OnFail:        &OnFail{Action: "loop_back", TargetPhase: "implementer"},
	}
}

func TestValidateWorkflowStructure_StrictBuildQAContract(t *testing.T) {
	if err := ValidateWorkflowStructure(Workflow{
		Stage:  "build",
		Phases: []Phase{{Name: "implementer", Agent: "implementer"}, strictQAPhase()},
	}); err != nil {
		t.Fatalf("valid strict Build QA rejected: %v", err)
	}
	if err := ValidateWorkflowStructure(Workflow{
		Stage: "evolve", Phases: []Phase{{Name: "evaluate", Agent: "qa"}},
	}); err != nil {
		t.Fatalf("Evolve QA without a Build verdict contract changed: %v", err)
	}
}

func TestValidateWorkflowStructure_RejectsBuildQADowngradeAndInvalidContracts(t *testing.T) {
	missingOnFail := strictQAPhase()
	missingOnFail.OnFail = nil
	wrongAction := strictQAPhase()
	wrongAction.OnFail = &OnFail{Action: "continue", TargetPhase: "implementer"}
	blankTarget := strictQAPhase()
	blankTarget.OnFail = &OnFail{Action: "loop_back", TargetPhase: " \t "}
	modeConditional := strictQAPhase()
	modeConditional.RequiredWhen = "../policies/modes.yml#workflow_depth.qa"
	modeOptional := strictQAPhase()
	modeOptional.OptionalFor = []string{"explorer"}
	missingTestGate := strictQAPhase()
	missingTestGate.RequiredGates = []string{"build"}
	wrongStage := strictQAPhase()
	wrongAgent := strictQAPhase()
	wrongAgent.Agent = "reviewer"
	tests := []struct {
		name string
		wf   Workflow
		want string
	}{
		{"missing contract", Workflow{Stage: "build", Phases: []Phase{{Name: "qa", Agent: "qa"}}}, `requires verdict_contract "qa_v1"`},
		{
			"unknown contract",
			Workflow{Stage: "build", Phases: []Phase{{
				Name: "qa", Agent: "qa", VerdictContract: "qa_v2",
			}}},
			`unsupported verdict_contract "qa_v2"`,
		},
		{"wrong stage", Workflow{Stage: "evolve", Phases: []Phase{wrongStage}}, "requires stage build and agent qa"},
		{"wrong agent", Workflow{Stage: "build", Phases: []Phase{wrongAgent}}, "requires stage build and agent qa"},
		{"missing on_fail", Workflow{Stage: "build", Phases: []Phase{missingOnFail}}, "requires on_fail.loop_back"},
		{"wrong action", Workflow{Stage: "build", Phases: []Phase{wrongAction}}, "requires on_fail.loop_back"},
		{"blank target", Workflow{Stage: "build", Phases: []Phase{blankTarget}}, "requires on_fail.loop_back"},
		{"required_when bypass", Workflow{Stage: "build", Phases: []Phase{modeConditional}}, "must not be mode-skippable"},
		{"optional_for bypass", Workflow{Stage: "build", Phases: []Phase{modeOptional}}, "must not be mode-skippable"},
		{"missing test gate", Workflow{Stage: "build", Phases: []Phase{missingTestGate}}, "requires the independent test gate"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			err := ValidateWorkflowStructure(tc.wf)
			if err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("validation error = %v, want substring %q", err, tc.want)
			}
		})
	}
}

func TestValidateWorkflowStructure_StrictQATargetMustBePriorImplementer(t *testing.T) {
	targetedQA := func(target string) Phase {
		qa := strictQAPhase()
		qa.OnFail.TargetPhase = target
		return qa
	}
	tests := []struct {
		name   string
		phases []Phase
		want   string
	}{
		{"missing target", []Phase{{Name: "implementer", Agent: "implementer"}, targetedQA("missing")}, "does not exist"},
		{
			"self target",
			[]Phase{{Name: "implementer", Agent: "implementer"}, targetedQA("qa")},
			"must be an earlier phase",
		},
		{
			"forward target",
			[]Phase{targetedQA("implementer"), {Name: "implementer", Agent: "implementer"}},
			"must be an earlier phase",
		},
		{
			"prior non-implementer",
			[]Phase{{Name: "reviewer", Agent: "reviewer"}, targetedQA("reviewer")},
			"must use agent implementer",
		},
		{
			"readonly implementer",
			[]Phase{{Name: "implementer", Agent: "implementer", Readonly: true}, targetedQA("implementer")},
			"must be writable",
		},
		{
			"mode-skippable implementer",
			[]Phase{{
				Name: "implementer", Agent: "implementer", OptionalFor: []string{"explorer"},
			}, targetedQA("implementer")},
			"must not be mode-skippable",
		},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			err := ValidateWorkflowStructure(Workflow{Stage: "build", Phases: tc.phases})
			if err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("validation error = %v, want substring %q", err, tc.want)
			}
		})
	}
}

func TestLoadWorkflowJSON_PreservesQAVerdictContract(t *testing.T) {
	wf, err := LoadWorkflowJSON([]byte(`{
		"stage":"build",
		"phases":[
			{"name":"implementer","agent":"implementer"},
			{"name":"qa","agent":"qa","verdict_contract":"qa_v1",
			 "required_gates":["test"],
			 "on_fail":{"action":"loop_back","target_phase":"implementer"}}
		]
	}`))
	if err != nil {
		t.Fatalf("LoadWorkflowJSON: %v", err)
	}
	if got := wf.Phases[1].VerdictContract; got != VerdictContractQAV1 {
		t.Fatalf("VerdictContract = %q, want %q", got, VerdictContractQAV1)
	}
}
