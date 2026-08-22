package asset

import (
	"strings"
	"testing"
)

func boundReviewerPhase() Phase {
	phase := strictReviewerPhase()
	phase.VerdictContract = VerdictContractReviewerV2
	return phase
}

func boundBuildWorkflow() Workflow {
	wf := reviewerWorkflow(boundReviewerPhase())
	wf.Phases[2].Readonly = true
	wf.OutputBindingContract = OutputBindingContractLocalDigestV1
	return wf
}

func TestValidateWorkflowStructureAcceptsBoundReviewerV2(t *testing.T) {
	wf := boundBuildWorkflow()
	if err := ValidateWorkflowStructure(wf); err != nil {
		t.Fatalf("valid bound Build rejected: %v", err)
	}
	loaded, err := LoadWorkflowJSON([]byte(`{
		"stage":"build","output_binding_contract":"local_digest_v1","phases":[
		{"name":"implementer","agent":"implementer"},
		{"name":"reviewer","agent":"reviewer","readonly":true,"fresh_context":true,
		 "required_when":"../policies/modes.yml#workflow_depth.reviewer",
		 "verdict_contract":"reviewer_v2","on_fail":{"action":"loop_back","target_phase":"implementer"}},
		{"name":"qa","agent":"qa","readonly":true,"verdict_contract":"qa_v1","required_gates":["test"],
		 "on_fail":{"action":"loop_back","target_phase":"implementer"}}]}`))
	if err != nil {
		t.Fatalf("LoadWorkflowJSON: %v", err)
	}
	if loaded.OutputBindingContract != OutputBindingContractLocalDigestV1 ||
		loaded.Phases[1].VerdictContract != VerdictContractReviewerV2 {
		t.Fatalf("bound contract roundtrip = %#v", loaded)
	}
}

func TestValidateWorkflowStructureRejectsUnsafeBindingSelectors(t *testing.T) {
	tests := []struct {
		name string
		edit func(*Workflow)
		want string
	}{
		{"unknown selector", func(w *Workflow) { w.OutputBindingContract = "local_digest_v2" }, "unsupported output_binding_contract"},
		{"v2 without selector", func(w *Workflow) { w.OutputBindingContract = "" }, "requires output_binding_contract"},
		{"bound build missing v2", func(w *Workflow) { w.Phases[1].VerdictContract = VerdictContractReviewerV1 }, "exactly one reviewer_v2"},
		{"bound build duplicate v2", func(w *Workflow) {
			second := w.Phases[1]
			second.Name = "second-reviewer"
			w.Phases = append(w.Phases[:2], append([]Phase{second}, w.Phases[2:]...)...)
		}, "exactly one reviewer_v2"},
		{"bound build missing QA", func(w *Workflow) { w.Phases = w.Phases[:2] }, "at least one qa_v1"},
		{"post-review writer", func(w *Workflow) { w.Phases[2].Readonly = false }, "after reviewer_v2 must be readonly"},
		{"ambiguous artifact owner", func(w *Workflow) {
			w.Phases[0].Emits = []string{"report.md"}
			w.Phases[2].Emits = []string{"report.md"}
		}, "ambiguous owners"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			wf := boundBuildWorkflow()
			tc.edit(&wf)
			err := ValidateWorkflowStructure(wf)
			if err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("validation error = %v, want %q", err, tc.want)
			}
		})
	}
}

func TestValidateWorkflowStructureReviewerV2KeepsReviewerShape(t *testing.T) {
	wf := boundBuildWorkflow()
	wf.Phases[1].Readonly = false
	if err := ValidateWorkflowStructure(wf); err == nil || !strings.Contains(err.Error(), "requires readonly fresh context") {
		t.Fatalf("unsafe reviewer_v2 error = %v", err)
	}
}
