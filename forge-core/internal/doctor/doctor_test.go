package doctor

import (
	"os"
	"path/filepath"
	"testing"

	"forgeos/forge-core/internal/persist"
)

// TestRun_NoForgeDir covers the short-circuit: a repo that has never run
// (.forge/ absent) reports NoForgeDir and no checks at all.
func TestRun_NoForgeDir(t *testing.T) {
	root := t.TempDir()
	rep := Run(root)
	if !rep.NoForgeDir {
		t.Fatal("NoForgeDir = false, want true for a repo with no .forge directory")
	}
	if len(rep.Checks) != 0 {
		t.Errorf("Checks = %+v, want none when .forge is absent", rep.Checks)
	}
}

// TestRun_FreshForgeDir covers a .forge/ that exists but is otherwise empty:
// every check should read as OK (first-run defaults), never a hard failure.
func TestRun_FreshForgeDir(t *testing.T) {
	root := t.TempDir()
	mustMkdotForge(t, root)

	rep := Run(root)
	if rep.NoForgeDir {
		t.Fatal("NoForgeDir = true, want false once .forge exists")
	}
	if len(rep.Checks) == 0 {
		t.Fatal("Checks is empty, want at least the always-present ones")
	}
	for _, c := range rep.Checks {
		if !c.OK {
			t.Errorf("check %q not OK on a fresh .forge dir: %s", c.Name, c.Detail)
		}
	}
}

// TestRun_TmpResidueFlagged covers a leftover *.tmp file (a crashed write) as
// the one condition in a fresh .forge/ that must fail.
func TestRun_TmpResidueFlagged(t *testing.T) {
	root := t.TempDir()
	dotForge := mustMkdotForge(t, root)
	if err := os.WriteFile(filepath.Join(dotForge, "checkpoint.json.tmp"), []byte("{"), 0o644); err != nil {
		t.Fatalf("seed .tmp residue: %v", err)
	}

	rep := Run(root)
	var found bool
	for _, c := range rep.Checks {
		if c.Name == "no .tmp residue" {
			found = true
			if c.OK {
				t.Error("no .tmp residue check OK=true, want false with a leftover .tmp file present")
			}
		}
	}
	if !found {
		t.Fatal("no .tmp residue check missing from report")
	}
}

// TestRun_TruncatedTraceFlagged covers trace.jsonl's completeness check: a
// last line that isn't a well-formed `{...}` object must fail, distinct from
// the missing/empty cases which are fine.
func TestRun_TruncatedTraceFlagged(t *testing.T) {
	root := t.TempDir()
	dotForge := mustMkdotForge(t, root)
	if err := os.WriteFile(filepath.Join(dotForge, "trace.jsonl"), []byte(`{"seq":1}`+"\n"+`{"seq":2,"kind":`), 0o644); err != nil {
		t.Fatalf("seed truncated trace: %v", err)
	}

	rep := Run(root)
	var found bool
	for _, c := range rep.Checks {
		if c.Name == "trace.jsonl" {
			found = true
			if c.OK {
				t.Error("trace.jsonl check OK=true, want false for a truncated last line")
			}
		}
	}
	if !found {
		t.Fatal("trace.jsonl check missing from report")
	}
}

// TestRun_CheckpointHistory covers the retain=N backup chain surfacing as a
// passing "checkpoint history" check once backups exist.
func TestRun_CheckpointHistory(t *testing.T) {
	root := t.TempDir()
	dotForge := mustMkdotForge(t, root)
	cpPath := filepath.Join(dotForge, "checkpoint.json")
	cp := persist.Checkpoint{Workflow: "build", Mode: "balanced", Iteration: 1}
	if err := persist.Save(cpPath, cp, 3); err != nil {
		t.Fatalf("seed checkpoint #1: %v", err)
	}
	cp.Iteration = 2
	if err := persist.Save(cpPath, cp, 3); err != nil {
		t.Fatalf("seed checkpoint #2: %v", err)
	}

	rep := Run(root)
	var found bool
	for _, c := range rep.Checks {
		if c.Name == "checkpoint history" {
			found = true
			if !c.OK {
				t.Errorf("checkpoint history check not OK: %s", c.Detail)
			}
		}
	}
	if !found {
		t.Fatal("checkpoint history check missing once a backup exists")
	}
}

// TestCheck_Line covers Check.Line's two render shapes.
func TestCheck_Line(t *testing.T) {
	ok := Check{Name: "thing", OK: true}
	if got, want := ok.Line(), "[PASS] thing"; got != want {
		t.Errorf("Line() = %q, want %q", got, want)
	}
	bad := Check{Name: "thing", OK: false, Detail: "broke"}
	if got, want := bad.Line(), "[FAIL] thing — broke"; got != want {
		t.Errorf("Line() = %q, want %q", got, want)
	}
}

// mustMkdotForge creates <root>/.forge and returns its path, the shared
// fixture setup for every Run/QuickChecks/Status test in this package.
func mustMkdotForge(t *testing.T, root string) string {
	t.Helper()
	dotForge := filepath.Join(root, ".forge")
	if err := os.MkdirAll(dotForge, 0o755); err != nil {
		t.Fatalf("mkdir .forge: %v", err)
	}
	return dotForge
}
