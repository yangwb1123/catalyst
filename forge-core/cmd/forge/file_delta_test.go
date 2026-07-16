package main

import (
	"os"
	"os/exec"
	"path/filepath"
	"testing"
)

// file_delta_test.go — computeFileDelta against a REAL temp git repository (not a
// mock), since the function's whole job is to shell out to `git diff --name-only
// HEAD` and cross-reference it against ROADMAP.md's DONE items. Kept as its own
// file (gates_test.go is already sizeable) since the git-fixture helpers below are
// only needed here.

// initGitRepo creates a fresh git repository in a temp dir with local user.name/
// user.email configured (so commits succeed even with no global git identity), and
// returns its root path.
func initGitRepo(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	runGit(t, root, "init")
	runGit(t, root, "config", "user.email", "test@example.com")
	runGit(t, root, "config", "user.name", "Test")
	return root
}

// runGit runs a git subcommand against root, failing the test on any error.
func runGit(t *testing.T, root string, args ...string) {
	t.Helper()
	cmd := exec.Command("git", append([]string{"-C", root}, args...)...)
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("git %v: %v\n%s", args, err, out)
	}
}

// commitAll stages and commits every file currently in root (the baseline HEAD
// computeFileDelta's `git diff --name-only HEAD` measures against).
func commitAll(t *testing.T, root, msg string) {
	t.Helper()
	runGit(t, root, "add", "-A")
	runGit(t, root, "commit", "-m", msg)
}

// writeAndDir writes content to a file under root, creating parent dirs as needed.
func writeAndDir(t *testing.T, root, rel, content string) {
	t.Helper()
	full := filepath.Join(root, rel)
	if err := os.MkdirAll(filepath.Dir(full), 0o755); err != nil {
		t.Fatalf("mkdir %s: %v", filepath.Dir(full), err)
	}
	if err := os.WriteFile(full, []byte(content), 0o644); err != nil {
		t.Fatalf("write %s: %v", full, err)
	}
}

// stageChange writes a NEW file under root and STAGES it (git add, no commit) so it
// appears in `git diff --name-only HEAD` — computeFileDelta's exact git invocation
// (mirroring computeCodeTestRatio's `git diff --stat HEAD`). HONESTY GOTCHA this
// helper exists to route around: git diff HEAD, like plain `git diff`, never shows
// a brand-new UNTRACKED file — only staged or already-tracked-and-modified paths —
// so a fixture must `git add` a new file before the diff will report it. This is
// the same real git semantics computeFileDelta inherits (a truly untracked new file
// an agent forgot to `git add` would not count as evidence either); the test fixture
// mirrors realistic usage rather than hiding the gotcha.
func stageChange(t *testing.T, root, rel, content string) {
	t.Helper()
	writeAndDir(t, root, rel, content)
	runGit(t, root, "add", rel)
}

// computeFileDelta with NO done ("- [x]") items in ROADMAP.md must return 0
// (nothing was claimed done, so there is nothing to cross-check) — even though a
// real, uncommitted change exists in the diff.
func TestComputeFileDelta_NoDoneItems(t *testing.T) {
	root := initGitRepo(t)
	writeAndDir(t, root, ".agent/ROADMAP.md", "# Roadmap\n- [ ] pending feature\n- [~] partial feature\n")
	commitAll(t, root, "baseline")
	stageChange(t, root, "src/pending/feature.go", "package pending\n")

	if got := computeFileDelta(root); got != 0 {
		t.Errorf("computeFileDelta() with no done items = %v, want 0", got)
	}
}

// computeFileDelta where EVERY done item's keyword touches a changed path must
// return exactly 1.0 (full cross-validation — the honest "high confidence" case).
func TestComputeFileDelta_AllItemsMatch(t *testing.T) {
	root := initGitRepo(t)
	writeAndDir(t, root, ".agent/ROADMAP.md",
		"# Roadmap\n- [x] implement payment gateway\n- [x] add user authentication\n")
	commitAll(t, root, "baseline")
	stageChange(t, root, "src/payment/gateway.go", "package payment\n")
	stageChange(t, root, "src/auth/user_login.go", "package auth\n")

	got := computeFileDelta(root)
	if got != 1.0 {
		t.Errorf("computeFileDelta() with every done item matched = %v, want 1.0", got)
	}
}

// computeFileDelta where SOME done items match and others don't must return the
// exact fraction — the case that actually drives loop.go's honesty warning
// (roadmap high, file evidence partial/low).
func TestComputeFileDelta_PartialMatch(t *testing.T) {
	root := initGitRepo(t)
	writeAndDir(t, root, ".agent/ROADMAP.md",
		"# Roadmap\n- [x] implement payment gateway\n- [x] add user authentication\n- [x] refactor logging module\n")
	commitAll(t, root, "baseline")
	// Only the payment item has a corresponding changed path; auth and logging do not.
	stageChange(t, root, "src/payment/gateway.go", "package payment\n")

	got := computeFileDelta(root)
	want := 1.0 / 3.0
	if got != want {
		t.Errorf("computeFileDelta() with 1-of-3 done items matched = %v, want %v", got, want)
	}
}

// computeFileDelta where NO done item's keywords touch any changed path must
// return 0 — the exact false-positive-warning scenario the task description
// starts from (roadmap claims completion, git diff shows unrelated changes).
func TestComputeFileDelta_NoMatch(t *testing.T) {
	root := initGitRepo(t)
	writeAndDir(t, root, ".agent/ROADMAP.md",
		"# Roadmap\n- [x] implement payment gateway\n- [x] add user authentication\n")
	commitAll(t, root, "baseline")
	stageChange(t, root, "docs/unrelated-notes.md", "nothing to do with either item\n")

	if got := computeFileDelta(root); got != 0 {
		t.Errorf("computeFileDelta() with no matching changes = %v, want 0", got)
	}
}

// computeFileDelta must degrade to 0 (never error/panic) when ROADMAP.md is
// missing entirely — the same error posture as computeCodeTestRatio.
func TestComputeFileDelta_MissingRoadmapDegradesToZero(t *testing.T) {
	root := initGitRepo(t)
	writeAndDir(t, root, "README.md", "no .agent dir at all\n")
	commitAll(t, root, "baseline")

	if got := computeFileDelta(root); got != 0 {
		t.Errorf("computeFileDelta() with no ROADMAP.md = %v, want 0", got)
	}
}
