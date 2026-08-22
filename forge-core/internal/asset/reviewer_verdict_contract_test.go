package asset

import (
	"strings"
	"testing"
)

func strictReviewerPhase() Phase {
	return Phase{
		Name: "reviewer", Agent: "reviewer", Readonly: true, FreshContext: true,
		VerdictContract: VerdictContractReviewerV1,
		RequiredWhen:    "../policies/modes.yml#workflow_depth.reviewer",
		OnFail:          &OnFail{Action: "loop_back", TargetPhase: "implementer"},
	}
}

func reviewerWorkflow(reviewer Phase) Workflow {
	return Workflow{Stage: "build", Phases: []Phase{
		{Name: "implementer", Agent: "implementer"}, reviewer, strictQAPhase(),
	}}
}

func TestValidateWorkflowStructureReviewerV1Shape(t *testing.T) {
	if err := ValidateWorkflowStructure(reviewerWorkflow(strictReviewerPhase())); err != nil {
		t.Fatalf("valid reviewer_v1 rejected: %v", err)
	}
	loaded, err := LoadWorkflowJSON([]byte(`{
		"stage":"build","phases":[
		{"name":"implementer","agent":"implementer"},
		{"name":"reviewer","agent":"reviewer","readonly":true,"fresh_context":true,
		 "required_when":"../policies/modes.yml#workflow_depth.reviewer",
		 "verdict_contract":"reviewer_v1","on_fail":{"action":"loop_back","target_phase":"implementer"}},
		{"name":"qa","agent":"qa","verdict_contract":"qa_v1","required_gates":["test"],
		 "on_fail":{"action":"loop_back","target_phase":"implementer"}}]}`))
	if err != nil || loaded.Phases[1].VerdictContract != VerdictContractReviewerV1 {
		t.Fatalf("reviewer_v1 JSON roundtrip = %q, %v", loaded.Phases[1].VerdictContract, err)
	}
}

func TestValidateWorkflowStructureRejectsUnsafeReviewerV1Shapes(t *testing.T) {
	tests := []struct {
		name string
		edit func(*Workflow, *Phase)
		want string
	}{
		{"wrong stage", func(w *Workflow, _ *Phase) { w.Stage = "review" }, "requires stage build and agent reviewer"},
		{"wrong agent", func(_ *Workflow, p *Phase) { p.Agent = "qa" }, "requires stage build and agent reviewer"},
		{"not readonly", func(_ *Workflow, p *Phase) { p.Readonly = false }, "requires readonly fresh context"},
		{"not fresh", func(_ *Workflow, p *Phase) { p.FreshContext = false }, "requires readonly fresh context"},
		{"feeds forward", func(_ *Workflow, p *Phase) { p.FeedsForward = true }, "requires readonly fresh context"},
		{"emits", func(_ *Workflow, p *Phase) { p.Emits = []string{"review.md"} }, "requires readonly fresh context"},
		{"writes ADR", func(_ *Workflow, p *Phase) { p.WritesADR = &WritesADR{} }, "requires readonly fresh context"},
		{"optional", func(_ *Workflow, p *Phase) { p.OptionalFor = []string{"explorer"} }, "must not be mode-skippable"},
		{"bad required_when", func(_ *Workflow, p *Phase) { p.RequiredWhen = "../policies/modes.yml#other" }, "must not be mode-skippable"},
		{"missing loop", func(_ *Workflow, p *Phase) { p.OnFail = nil }, "requires on_fail.loop_back"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			phase := strictReviewerPhase()
			wf := reviewerWorkflow(phase)
			tc.edit(&wf, &wf.Phases[1])
			err := ValidateWorkflowStructure(wf)
			if err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("validation error = %v, want %q", err, tc.want)
			}
		})
	}
}

func TestValidateWorkflowStructureRejectsReviewerV1TargetAndOrderBypass(t *testing.T) {
	tests := []struct {
		name string
		edit func(*Workflow)
		want string
	}{
		{"missing target", func(w *Workflow) { w.Phases[1].OnFail.TargetPhase = "missing" }, "does not exist"},
		{"forward target", func(w *Workflow) { w.Phases[1].OnFail.TargetPhase = "qa" }, "must be an earlier phase"},
		{"readonly target", func(w *Workflow) { w.Phases[0].Readonly = true }, "must be writable"},
		{"skippable target", func(w *Workflow) { w.Phases[0].OptionalFor = []string{"explorer"} }, "must not be mode-skippable"},
		{"reviewer after QA", func(w *Workflow) { w.Phases[1], w.Phases[2] = w.Phases[2], w.Phases[1] }, "must precede Build QA"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			wf := reviewerWorkflow(strictReviewerPhase())
			tc.edit(&wf)
			err := ValidateWorkflowStructure(wf)
			if err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("validation error = %v, want %q", err, tc.want)
			}
		})
	}
}

func TestValidateWorkflowStructureReviewerMustPrecedeEarliestQA(t *testing.T) {
	reviewer := strictReviewerPhase()
	firstQA := strictQAPhase()
	firstQA.Name = "early-qa"
	lastQA := strictQAPhase()
	lastQA.Name = "late-qa"
	wf := Workflow{Stage: "build", Phases: []Phase{
		{Name: "implementer", Agent: "implementer"}, firstQA, reviewer, lastQA,
	}}
	if err := ValidateWorkflowStructure(wf); err == nil || !strings.Contains(err.Error(), "must precede Build QA") {
		t.Fatalf("reviewer between two QA phases error = %v", err)
	}
}
