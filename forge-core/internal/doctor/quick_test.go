package doctor

import (
	"os"
	"path/filepath"
	"testing"

	"forgeos/forge-core/internal/persist"
)

// TestQuickChecks_NoForgeDir covers the first-run short-circuit: a single
// "preflight" ok entry and nothing else.
func TestQuickChecks_NoForgeDir(t *testing.T) {
	root := t.TempDir()
	checks := QuickChecks(root)
	if len(checks) != 1 || checks[0].Name != "preflight" || checks[0].Status != "ok" {
		t.Fatalf("QuickChecks = %+v, want a single ok preflight entry", checks)
	}
}

// TestQuickChecks_FreshForgeDir_OnlyClosingEntry covers the "silent unless
// there's something to say" convention: an empty .forge/ produces exactly the
// closing "preflight complete" entry, none of the per-file checks fire.
func TestQuickChecks_FreshForgeDir_OnlyClosingEntry(t *testing.T) {
	root := t.TempDir()
	mustMkdotForge(t, root)

	checks := QuickChecks(root)
	if len(checks) != 1 || checks[0].Name != "preflight" || checks[0].Detail != "quick doctor check complete" {
		t.Fatalf("QuickChecks = %+v, want only the closing preflight entry", checks)
	}
}

// TestQuickChecks_CheckpointReadable covers the checkpoint sub-check firing
// "ok" once a valid checkpoint.json is present.
func TestQuickChecks_CheckpointReadable(t *testing.T) {
	root := t.TempDir()
	dotForge := mustMkdotForge(t, root)
	cp := persist.Checkpoint{Workflow: "build", Mode: "balanced", Iteration: 1}
	if err := persist.Save(filepath.Join(dotForge, "checkpoint.json"), cp, 0); err != nil {
		t.Fatalf("seed checkpoint: %v", err)
	}

	checks := QuickChecks(root)
	if !hasQuickCheck(checks, "checkpoint", "ok") {
		t.Errorf("QuickChecks = %+v, want an ok checkpoint entry", checks)
	}
}

// TestQuickChecks_TmpResidueWarns covers the WARN (not FAIL — quick checks
// are advisory) sub-check for leftover *.tmp files.
func TestQuickChecks_TmpResidueWarns(t *testing.T) {
	root := t.TempDir()
	dotForge := mustMkdotForge(t, root)
	if err := os.WriteFile(filepath.Join(dotForge, "checkpoint.json.tmp"), []byte("{"), 0o644); err != nil {
		t.Fatalf("seed .tmp residue: %v", err)
	}

	checks := QuickChecks(root)
	if !hasQuickCheck(checks, "tmp-residue", "WARN") {
		t.Errorf("QuickChecks = %+v, want a WARN tmp-residue entry", checks)
	}
}

// TestQuickChecks_TruncatedTraceFails covers the trace sub-check's FAIL path
// on a malformed last line — unlike a missing/empty trace, which is silent.
func TestQuickChecks_TruncatedTraceFails(t *testing.T) {
	root := t.TempDir()
	dotForge := mustMkdotForge(t, root)
	if err := os.WriteFile(filepath.Join(dotForge, "trace.jsonl"), []byte(`{"seq":1,`), 0o644); err != nil {
		t.Fatalf("seed truncated trace: %v", err)
	}

	checks := QuickChecks(root)
	if !hasQuickCheck(checks, "trace", "FAIL") {
		t.Errorf("QuickChecks = %+v, want a FAIL trace entry", checks)
	}
}

// hasQuickCheck reports whether checks contains an entry with the given name
// and status.
func hasQuickCheck(checks []QuickCheck, name, status string) bool {
	for _, c := range checks {
		if c.Name == name && c.Status == status {
			return true
		}
	}
	return false
}
