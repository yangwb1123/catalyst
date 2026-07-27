package main

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

func TestPhaseOutputContractRejectsDuplicatePhaseIdentity(t *testing.T) {
	workflow := asset.Workflow{Stage: "build", Phases: []asset.Phase{
		{Name: "same", Agent: "implementer"},
		{Name: "same", Agent: "reviewer"},
	}}

	err := phaseOutputContract(t.TempDir(), workflow)("same", "done")
	if err == nil || !strings.Contains(err.Error(), `duplicates phase name "same"`) {
		t.Fatalf("output contract error = %v, want duplicate phase identity", err)
	}
}

func TestPhaseOutputContractRejectsDuplicateNormalizedEmitIdentity(t *testing.T) {
	workflow := asset.Workflow{Stage: "build", Phases: []asset.Phase{{
		Name: "implement", Agent: "implementer", Emits: []string{"b.md", "a/../b.md"},
	}}}

	err := phaseOutputContract(t.TempDir(), workflow)("implement", "done")
	if err == nil || !strings.Contains(err.Error(), "duplicates normalized target") {
		t.Fatalf("output contract error = %v, want duplicate emit identity", err)
	}
}
