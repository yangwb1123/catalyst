package asset

import (
	"strings"
	"testing"
)

func TestLoadWorkflowJSONDecodesEvolveScanContract(t *testing.T) {
	wf, err := LoadWorkflowJSON([]byte(`{"stage":"evolve","phases":[{
		"name":"inventory","agent":"explorer","readonly":true,"effect":"observe",
		"feeds_forward":true,"scan_contract":"evolve_scan_v1"
	}]}`))
	if err != nil {
		t.Fatalf("load scan contract: %v", err)
	}
	if got := wf.Phases[0].ScanContract; got != ScanContractEvolveV1 {
		t.Fatalf("ScanContract = %q, want %q", got, ScanContractEvolveV1)
	}
}

func TestValidateWorkflowStructureRejectsInvalidScanContracts(t *testing.T) {
	valid := Phase{
		Name: "inventory", Agent: "explorer", Readonly: true, Effect: "observe",
		FeedsForward: true, ScanContract: ScanContractEvolveV1,
	}
	tests := []struct {
		name  string
		stage string
		edit  func(*Phase)
		want  string
	}{
		{"unknown", "evolve", func(p *Phase) { p.ScanContract = "evolve_scan_v2" }, "unsupported"},
		{"wrong stage", "build", func(*Phase) {}, "requires stage evolve"},
		{"writable", "evolve", func(p *Phase) { p.Readonly = false }, "readonly=true"},
		{"wrong effect", "evolve", func(p *Phase) { p.Effect = "propose" }, "effect=observe"},
		{"gate-only harness", "evolve", func(p *Phase) { p.Agent = "harness" }, "non-harness"},
		{"gated", "evolve", func(p *Phase) { p.RequiredGates = []string{"test"} }, "required_gates=[]"},
		{"emitting", "evolve", func(p *Phase) { p.Emits = []string{"scan.md"} }, "must not grant emits"},
		{"ADR write", "evolve", func(p *Phase) { p.WritesADR = &WritesADR{Condition: "always"} }, "writes_adr"},
		{"not forwarded", "evolve", func(p *Phase) { p.FeedsForward = false }, "feeds_forward=true"},
		{"optional", "evolve", func(p *Phase) { p.OptionalFor = []string{"explorer"} }, "must not be mode-skippable"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			phase := valid
			tc.edit(&phase)
			err := ValidateWorkflowStructure(Workflow{Stage: tc.stage, Phases: []Phase{phase}})
			if err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("error = %v, want substring %q", err, tc.want)
			}
		})
	}
}

func TestValidateWorkflowStructureRequiresSafeScanDependencyGraph(t *testing.T) {
	scan := Phase{
		Name: "inventory", Agent: "explorer", Readonly: true, Effect: "observe",
		FeedsForward: true, ScanContract: ScanContractEvolveV1,
	}
	wf := Workflow{Stage: "evolve", Phases: []Phase{
		scan,
		{Name: "gap", Agent: "architect", DependsOn: []string{"inventory"}},
		{Name: "implement", Agent: "implementer"},
	}}
	err := ValidateWorkflowStructure(wf)
	if err == nil || !strings.Contains(err.Error(), "must transitively depend") {
		t.Fatalf("unsafe dependency graph error = %v", err)
	}
	wf.Phases[2].DependsOn = []string{"gap"}
	if err := ValidateWorkflowStructure(wf); err != nil {
		t.Fatalf("safe dependency graph: %v", err)
	}
}

func TestValidateWorkflowStructureRequiresContractedScanFirst(t *testing.T) {
	scan := Phase{
		Name: "inventory", Agent: "explorer", Readonly: true, Effect: "observe",
		FeedsForward: true, ScanContract: ScanContractEvolveV1,
	}
	err := ValidateWorkflowStructure(Workflow{Stage: "evolve", Phases: []Phase{
		{Name: "implement", Agent: "implementer", Effect: "mutate"},
		scan,
	}})
	if err == nil || !strings.Contains(err.Error(), "must be the first phase") {
		t.Fatalf("late scan contract error = %v", err)
	}
}

func TestValidateWorkflowStructureRejectsDuplicateScanContracts(t *testing.T) {
	phase := Phase{
		Name: "scan-a", Agent: "explorer", Readonly: true, Effect: "observe",
		FeedsForward: true, ScanContract: ScanContractEvolveV1,
	}
	second := phase
	second.Name = "scan-b"
	err := ValidateWorkflowStructure(Workflow{Stage: "evolve", Phases: []Phase{phase, second}})
	if err == nil || !strings.Contains(err.Error(), "declared by both") {
		t.Fatalf("duplicate scan contract error = %v", err)
	}
}

func TestValidateWorkflowStructureLeavesLegacyEvolveUnchanged(t *testing.T) {
	wf := Workflow{Stage: "evolve", Phases: []Phase{{
		Name: "legacy-scan", Agent: "explorer", Readonly: true, Effect: "observe",
	}}}
	if err := ValidateWorkflowStructure(wf); err != nil {
		t.Fatalf("legacy workflow without scan_contract: %v", err)
	}
}
