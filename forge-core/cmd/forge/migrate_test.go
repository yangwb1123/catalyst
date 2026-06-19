package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// fakeAgentRepo builds a minimal repo root with a <root>/.agent/{project.yml,
// ROADMAP.md} so the apply path has the two files it mutates, without the real
// ForgeOS tree. project.yml carries a `mode: explorer` line with an inline
// comment (to prove the comment survives the minimal-edit rewrite).
func fakeAgentRepo(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".agent"))
	writeFile(t, filepath.Join(root, ".agent", "project.yml"),
		"project: demo\nmode: explorer                 # 全闸门 (explorer | balanced | engineering | cto)\nlifecycle: mvp\n")
	writeFile(t, filepath.Join(root, ".agent", "ROADMAP.md"), "# Roadmap\n\n## v0\n- [x] existing item\n")
	return root
}

// readFileStr reads a file's text or fails the test.
func readFileStr(t *testing.T, path string) string {
	t.Helper()
	b, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	return string(b)
}

// DRY is the fail-safe default: `forge migrate --to engineering` (no --apply)
// must exit 0, and must NOT touch project.yml or ROADMAP.md — every byte
// unchanged. This is the load-bearing "default dry writes nothing" guarantee.
func TestMigrate_DryWritesNothing(t *testing.T) {
	root := fakeAgentRepo(t)
	projPath := filepath.Join(root, ".agent", "project.yml")
	roadPath := filepath.Join(root, ".agent", "ROADMAP.md")
	projBefore := readFileStr(t, projPath)
	roadBefore := readFileStr(t, roadPath)

	if code := cmdMigrate([]string{"--to", "engineering", "--root", root}); code != 0 {
		t.Fatalf("dry migrate exit = %d, want 0", code)
	}
	if got := readFileStr(t, projPath); got != projBefore {
		t.Errorf("DRY run mutated project.yml; before=%q after=%q", projBefore, got)
	}
	if got := readFileStr(t, roadPath); got != roadBefore {
		t.Errorf("DRY run mutated ROADMAP.md; before=%q after=%q", roadBefore, got)
	}
}

// --apply must flip project.yml's mode to engineering (preserving its inline
// comment and the other lines) AND append the five derived backfill tasks to
// ROADMAP.md as unchecked `- [ ] [migrate] ...` items — the gap->roadmap move.
func TestMigrate_ApplyFlipsModeAndInjectsTasks(t *testing.T) {
	root := fakeAgentRepo(t)
	projPath := filepath.Join(root, ".agent", "project.yml")
	roadPath := filepath.Join(root, ".agent", "ROADMAP.md")

	if code := cmdMigrate([]string{"--to", "engineering", "--apply", "--root", root}); code != 0 {
		t.Fatalf("apply migrate exit = %d, want 0", code)
	}

	// project.yml: mode flipped, comment + sibling lines preserved.
	proj := readFileStr(t, projPath)
	if !strings.Contains(proj, "mode: engineering") {
		t.Errorf("apply did not flip mode to engineering; got:\n%s", proj)
	}
	if strings.Contains(proj, "mode: explorer") {
		t.Errorf("old `mode: explorer` line still present after apply:\n%s", proj)
	}
	if !strings.Contains(proj, "(explorer | balanced | engineering | cto)") {
		t.Errorf("inline comment on the mode line was lost:\n%s", proj)
	}
	for _, sib := range []string{"project: demo", "lifecycle: mvp"} {
		if !strings.Contains(proj, sib) {
			t.Errorf("apply clobbered sibling line %q:\n%s", sib, proj)
		}
	}

	// ROADMAP.md: existing content preserved + all five tasks appended as
	// unchecked, [migrate]-tagged items carrying gate/priority.
	road := readFileStr(t, roadPath)
	if !strings.Contains(road, "- [x] existing item") {
		t.Errorf("apply clobbered the existing ROADMAP content:\n%s", road)
	}
	if n := strings.Count(road, "- [ ] [migrate]"); n != 5 {
		t.Errorf("expected 5 injected `- [ ] [migrate]` tasks, found %d:\n%s", n, road)
	}
	// Gate-scoped tasks must carry their gate; un-scoped ones must NOT fake one.
	for _, want := range []string{"(gate: test, high)", "(gate: complexity, medium)", "(gate: security, high)"} {
		if !strings.Contains(road, want) {
			t.Errorf("ROADMAP missing gate-scoped task metadata %q:\n%s", want, road)
		}
	}
	// add-ci / add-monitoring are un-scoped: a bare priority, no "gate:".
	if !strings.Contains(road, "add CI running the full harness (high)") {
		t.Errorf("un-scoped add-ci task should carry a bare priority (no gate):\n%s", road)
	}
}

// An unsupported / missing --to is an honest usage error (exit 2), never a
// silent no-op or a faked migration — and it must write nothing either.
func TestMigrate_UnsupportedToIsUsageError(t *testing.T) {
	root := fakeAgentRepo(t)
	projBefore := readFileStr(t, filepath.Join(root, ".agent", "project.yml"))

	if code := cmdMigrate([]string{"--to", "balanced", "--root", root}); code != 2 {
		t.Errorf("unsupported --to balanced should be a usage error (exit 2); got %d", code)
	}
	if code := cmdMigrate([]string{"--root", root}); code != 2 {
		t.Errorf("missing --to should be a usage error (exit 2); got %d", code)
	}
	if got := readFileStr(t, filepath.Join(root, ".agent", "project.yml")); got != projBefore {
		t.Errorf("a usage error must write nothing; project.yml changed:\n%s", got)
	}
}

// --apply against a repo whose project.yml has NO `mode:` line must fail loud
// (exit 1), not silently succeed — flipping the mode is the migration's point.
func TestMigrate_ApplyMissingModeLineFailsLoud(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".agent"))
	writeFile(t, filepath.Join(root, ".agent", "project.yml"), "project: demo\nlifecycle: mvp\n")
	if code := cmdMigrate([]string{"--to", "engineering", "--apply", "--root", root}); code != 1 {
		t.Errorf("apply with no `mode:` line should fail loud (exit 1); got %d", code)
	}
}
