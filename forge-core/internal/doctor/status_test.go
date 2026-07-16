package doctor

import (
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/persist"
)

// TestStatus_DotForgeMissing covers the short-circuit for a repo that has
// never run.
func TestStatus_DotForgeMissing(t *testing.T) {
	snap := Status(t.TempDir())
	if !snap.DotForgeMissing {
		t.Fatal("DotForgeMissing = false, want true when .forge is absent")
	}
}

// TestStatus_WithCheckpoint covers the normal path: file stats populated and
// checkpoint.json's content summarized into CheckpointInfo.
func TestStatus_WithCheckpoint(t *testing.T) {
	root := t.TempDir()
	dotForge := mustMkdotForge(t, root)
	cp := persist.Checkpoint{
		Workflow: "build", Mode: "engineering", Iteration: 7,
		RoadmapCompletion: 0.42, GatesGreen: true,
	}
	if err := persist.Save(filepath.Join(dotForge, "checkpoint.json"), cp, 0); err != nil {
		t.Fatalf("seed checkpoint: %v", err)
	}

	snap := Status(root)
	if snap.DotForgeMissing {
		t.Fatal("DotForgeMissing = true, want false once .forge exists")
	}
	if !snap.Checkpoint.Exists {
		t.Fatal("Checkpoint.Exists = false, want true")
	}
	if !snap.CheckpointInfo.Found || !snap.CheckpointInfo.ParseOK {
		t.Fatalf("CheckpointInfo = %+v, want Found+ParseOK", snap.CheckpointInfo)
	}
	if snap.CheckpointInfo.Iteration != 7 || snap.CheckpointInfo.Mode != "engineering" {
		t.Errorf("CheckpointInfo = %+v, want iteration=7 mode=engineering", snap.CheckpointInfo)
	}
	if !snap.CheckpointInfo.GatesGreen {
		t.Error("CheckpointInfo.GatesGreen = false, want true")
	}
}

// TestStatus_MissingFilesAreNotExists covers that trace/memory/backup
// FileStats report Exists=false (not an error) when simply absent.
func TestStatus_MissingFilesAreNotExists(t *testing.T) {
	root := t.TempDir()
	mustMkdotForge(t, root)

	snap := Status(root)
	if snap.Trace.Exists || snap.TraceBackup.Exists || snap.Memory.Exists {
		t.Errorf("expected all-absent file stats, got trace=%+v backup=%+v memory=%+v",
			snap.Trace, snap.TraceBackup, snap.Memory)
	}
}

// TestCheckpointHistoryCount covers the retain=N backup counter directly:
// zero with no backups, N after N rotations.
func TestCheckpointHistoryCount(t *testing.T) {
	root := t.TempDir()
	dotForge := mustMkdotForge(t, root)
	cpPath := filepath.Join(dotForge, "checkpoint.json")
	if n := CheckpointHistoryCount(dotForge); n != 0 {
		t.Fatalf("CheckpointHistoryCount = %d, want 0 with no checkpoint at all", n)
	}
	for i := 1; i <= 3; i++ {
		cp := persist.Checkpoint{Iteration: i}
		if err := persist.Save(cpPath, cp, 5); err != nil {
			t.Fatalf("seed checkpoint #%d: %v", i, err)
		}
	}
	if n := CheckpointHistoryCount(dotForge); n != 2 {
		t.Errorf("CheckpointHistoryCount = %d, want 2 after 3 saves (1 current + 2 backups)", n)
	}
}

// TestHistoryLines covers forge status --history's rendered rows: one line
// per chain entry, "current" label on the newest.
func TestHistoryLines(t *testing.T) {
	root := t.TempDir()
	dotForge := mustMkdotForge(t, root)
	cp := persist.Checkpoint{Mode: "balanced", Iteration: 1, RoadmapCompletion: 0.5}
	if err := persist.Save(filepath.Join(dotForge, "checkpoint.json"), cp, 0); err != nil {
		t.Fatalf("seed checkpoint: %v", err)
	}

	lines := HistoryLines(root)
	if len(lines) != 1 {
		t.Fatalf("HistoryLines = %v, want exactly 1 line for one checkpoint", lines)
	}
	if !strings.Contains(lines[0], "current") || !strings.Contains(lines[0], "balanced") {
		t.Errorf("line = %q, want it to contain %q and %q", lines[0], "current", "balanced")
	}
}

// TestHistoryLines_Empty covers the no-history case: an empty slice, not nil
// panics or a synthetic entry.
func TestHistoryLines_Empty(t *testing.T) {
	root := t.TempDir()
	mustMkdotForge(t, root)
	if lines := HistoryLines(root); len(lines) != 0 {
		t.Errorf("HistoryLines = %v, want empty with no checkpoint present", lines)
	}
}
