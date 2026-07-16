package doctor

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// TestGovernance_EmptyRoot covers a bare directory: every governance asset
// dir is absent (Count=0, Age=""), no build/go.mod/submodule/review.yml
// present, so every ADR mark reads "not implemented" (❌).
func TestGovernance_EmptyRoot(t *testing.T) {
	root := t.TempDir()
	rep := Governance(root)

	if len(rep.Dirs) != 5 {
		t.Fatalf("Dirs = %+v, want exactly 5 tracked governance directories", rep.Dirs)
	}
	for _, d := range rep.Dirs {
		if d.Count != 0 {
			t.Errorf("dir %q Count = %d, want 0 on an empty root", d.Label, d.Count)
		}
	}
	if rep.Evolving {
		t.Error("Evolving = true, want false with zero changes")
	}
	if len(rep.ADRs) != 4 {
		t.Fatalf("ADRs = %+v, want exactly 4 tracked ADRs", rep.ADRs)
	}
	for _, a := range rep.ADRs {
		if a.Mark != "❌" {
			t.Errorf("ADR %q mark = %q, want ❌ on a bare root", a.Label, a.Mark)
		}
	}
}

// TestGovernance_DirsWithFiles covers a directory report actually counting
// files and picking up a non-zero age once something is present.
func TestGovernance_DirsWithFiles(t *testing.T) {
	root := t.TempDir()
	agentsDir := filepath.Join(root, ".agent", "agents")
	if err := os.MkdirAll(agentsDir, 0o755); err != nil {
		t.Fatalf("mkdir: %v", err)
	}
	if err := os.WriteFile(filepath.Join(agentsDir, "planner.md"), []byte("# planner"), 0o644); err != nil {
		t.Fatalf("seed agent card: %v", err)
	}

	rep := Governance(root)
	var agentsReport *DirReport
	for i := range rep.Dirs {
		if rep.Dirs[i].Label == ".agent/agents/" {
			agentsReport = &rep.Dirs[i]
		}
	}
	if agentsReport == nil {
		t.Fatal(".agent/agents/ not present in report")
	}
	if agentsReport.Count != 1 {
		t.Errorf("Count = %d, want 1", agentsReport.Count)
	}
	if agentsReport.Age != "today" {
		t.Errorf("Age = %q, want %q for a just-written file", agentsReport.Age, "today")
	}
}

// TestGovernance_ADR0004_ReviewWorkflowPresent covers the one ADR check that
// is pure filesystem presence (no exec.Command involved): review.yml
// existing flips ADR-0004 to ✅.
func TestGovernance_ADR0004_ReviewWorkflowPresent(t *testing.T) {
	root := t.TempDir()
	workflowsDir := filepath.Join(root, ".agent", "workflows")
	if err := os.MkdirAll(workflowsDir, 0o755); err != nil {
		t.Fatalf("mkdir: %v", err)
	}
	if err := os.WriteFile(filepath.Join(workflowsDir, "review.yml"), []byte("id: review\n"), 0o644); err != nil {
		t.Fatalf("seed review.yml: %v", err)
	}

	rep := Governance(root)
	for _, a := range rep.ADRs {
		if strings.HasPrefix(a.Label, "ADR-0004") && a.Mark != "✅" {
			t.Errorf("ADR-0004 mark = %q, want ✅ once review.yml exists", a.Mark)
		}
	}
}

// TestGovernance_ADR0002_PolyglotRuntimeDetected covers checkADR0002's
// three-way branch: go.mod present + no forge-ai sibling = ✅ (Go-only).
func TestGovernance_ADR0002_GoOnly(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "forge-core"), 0o755); err != nil {
		t.Fatalf("mkdir forge-core: %v", err)
	}
	if err := os.WriteFile(filepath.Join(root, "forge-core", "go.mod"), []byte("module x\n"), 0o644); err != nil {
		t.Fatalf("seed go.mod: %v", err)
	}

	rep := Governance(root)
	for _, a := range rep.ADRs {
		if strings.HasPrefix(a.Label, "ADR-0002") && a.Mark != "✅" {
			t.Errorf("ADR-0002 mark = %q, want ✅ (Go-only, no forge-ai sibling)", a.Mark)
		}
	}
}
