package gate

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
)

// mustTool resolves a program on PATH or skips the test if it is unavailable,
// so the suite degrades gracefully on minimal environments.
func mustTool(t *testing.T, name string) string {
	t.Helper()
	p, err := exec.LookPath(name)
	if err != nil {
		t.Skipf("%s not available: %v", name, err)
	}
	return p
}

// findRepoRoot walks up from the test's cwd to the directory that contains
// harness/gate.mjs. Returns "" if no such ancestor exists (not in the repo).
func findRepoRoot() string {
	dir, err := os.Getwd()
	if err != nil {
		return ""
	}
	for {
		if _, err := os.Stat(filepath.Join(dir, "harness", "gate.mjs")); err == nil {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			return ""
		}
		dir = parent
	}
}

func TestRun_OK(t *testing.T) {
	tru := mustTool(t, "true")
	r := runWith(context.Background(), "ok", "", Options{}, tru)
	if !r.OK {
		t.Errorf("run(true).OK = false, output=%q", r.Output)
	}
	if r.Name != "ok" {
		t.Errorf("Name = %q, want ok", r.Name)
	}
}

func TestRun_NotOK(t *testing.T) {
	fal := mustTool(t, "false")
	r := runWith(context.Background(), "fail", "", Options{}, fal)
	if r.OK {
		t.Error("run(false).OK = true, want false")
	}
}

func TestRun_EmptyArgv(t *testing.T) {
	r := runWith(context.Background(), "empty", "", Options{})
	if r.OK {
		t.Error("empty argv should be not-OK")
	}
}

func TestRun_MissingBinary(t *testing.T) {
	r := runWith(context.Background(), "missing", "", Options{}, "forge-no-such-binary-xyz")
	if r.OK {
		t.Error("missing binary should be not-OK, not a panic")
	}
}

// runWith honors an explicit working directory only when one is given; an
// empty root leaves the process cwd untouched (RepoRoot("") resolves to
// ".", which is the same directory).
func TestRun_SetsDirWhenGiven(t *testing.T) {
	pwd := mustTool(t, "pwd")
	r := runWith(context.Background(), "dir", "/", Options{}, pwd)
	if !r.OK {
		t.Fatalf("run(pwd) in / failed: %q", r.Output)
	}
	if r.Output != "/" {
		t.Errorf("dir not applied: cwd = %q, want /", r.Output)
	}
}

func TestRepoRoot(t *testing.T) {
	if got := RepoRoot("/explicit"); got != "/explicit" {
		t.Errorf("explicit root = %q, want /explicit", got)
	}

	t.Setenv(EnvRoot, "/from/env")
	if got := RepoRoot(""); got != "/from/env" {
		t.Errorf("env root = %q, want /from/env", got)
	}

	t.Setenv(EnvRoot, "")
	if got := RepoRoot(""); got != "." {
		t.Errorf("default root = %q, want .", got)
	}
}

// When run inside the real ForgeOS repo with node available, the structural
// gate must pass for the current tree (clean source is a precondition of this
// whole change). Skips when either prerequisite is absent.
func TestGate_RealRepo(t *testing.T) {
	mustTool(t, "node")
	root := findRepoRoot()
	if root == "" {
		t.Skip("not running inside the ForgeOS repo (no harness/gate.mjs ancestor)")
	}
	r := Gate(root)
	if !r.OK {
		t.Errorf("Gate(%q).OK = false; structural gate failed:\n%s", root, r.Output)
	}
}
