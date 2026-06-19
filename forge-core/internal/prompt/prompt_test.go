package prompt

import (
	"strings"
	"testing"
)

func TestBuild_IncludesHeaderCardAndContext(t *testing.T) {
	got := Build("reviewer", "reviewer", "balanced", "opus",
		"# Agent: reviewer\nfresh-context, 只判不写", []string{"ADRs:\n- ADR 0001"})
	for _, want := range []string{`"reviewer" agent`, "phase=reviewer", "tier=opus",
		"## Role card", "只判不写", "## Project context", "ADR 0001"} {
		if !strings.Contains(got, want) {
			t.Errorf("prompt missing %q", want)
		}
	}
}

func TestBuild_EmptyContextOmitsSection(t *testing.T) {
	if got := Build("x", "x", "m", "sonnet", "card", nil); strings.Contains(got, "## Project context") {
		t.Errorf("empty context must omit the section; got: %s", got)
	}
}

// Gather reads the real repo: ADR titles and hard constraints must surface so
// a real agent prompt carries actual project ground truth, not just a role.
func TestGather_RealRepoHasADRsAndConstraints(t *testing.T) {
	joined := strings.Join(Gather("/home/u1/catalyst"), "\n")
	if !strings.Contains(joined, "ADR 0001") {
		t.Errorf("expected ADR titles in context; got: %.200s", joined)
	}
	if !strings.Contains(joined, "500") {
		t.Errorf("expected hard constraints (<=500 lines); got: %.300s", joined)
	}
}

func TestGather_MissingRepoDegradesQuietly(t *testing.T) {
	if ctx := Gather("/nonexistent-xyz-forge"); len(ctx) != 0 {
		t.Errorf("missing repo should yield no context, got %v", ctx)
	}
}
