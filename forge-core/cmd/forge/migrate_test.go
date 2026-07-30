package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/persist"
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

func fakeLifecycleRepo(t *testing.T, mode, lifecycle string) string {
	t.Helper()
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".agent"))
	writeFile(t, filepath.Join(root, ".agent", "project.yml"), fmt.Sprintf(
		"project: demo\nmode: %s  # persistent selector\nlifecycle: %s  # maturity\nowner: keep\n",
		mode, lifecycle))
	writeFile(t, filepath.Join(root, ".agent", "ROADMAP.md"),
		"# Roadmap\n\n## v0\n- [x] existing item\n")
	return root
}

type migrationFiles struct {
	project string
	roadmap string
}

func snapshotMigrationFiles(t *testing.T, root string) migrationFiles {
	t.Helper()
	return migrationFiles{
		project: readFileStr(t, filepath.Join(root, ".agent", "project.yml")),
		roadmap: readFileStr(t, filepath.Join(root, ".agent", "ROADMAP.md")),
	}
}

func requireMigrationFiles(t *testing.T, root string, want migrationFiles) {
	t.Helper()
	got := snapshotMigrationFiles(t, root)
	if got != want {
		t.Fatalf("persistent files changed\nproject before=%q after=%q\nroadmap before=%q after=%q",
			want.project, got.project, want.roadmap, got.roadmap)
	}
}

func requireNoForgeDir(t *testing.T, root string) {
	t.Helper()
	if _, err := os.Stat(filepath.Join(root, ".forge")); !os.IsNotExist(err) {
		t.Fatalf("operation created .forge; stat error = %v", err)
	}
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

func TestMigrateLifecycle_UsageErrorsWriteNothing(t *testing.T) {
	cases := []struct {
		name string
		args []string
	}{
		{"unsupported lifecycle", []string{"--to-lifecycle", "growth"}},
		{"mutually exclusive targets", []string{
			"--to", "engineering", "--to-lifecycle", "production",
		}},
		{"unexpected positional", []string{"--to-lifecycle", "production", "extra"}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			root := fakeLifecycleRepo(t, "explorer", "mvp")
			before := snapshotMigrationFiles(t, root)
			args := append(append([]string(nil), tc.args...), "--root", root)
			if code, _ := captureChainOutput(t, func() int { return cmdMigrate(args) }); code != 2 {
				t.Errorf("cmdMigrate(%v) = %d, want usage error 2", tc.args, code)
			}
			requireMigrationFiles(t, root, before)
			requireNoForgeDir(t, root)
		})
	}
}

func TestMigrateLifecycle_DryWritesNothingAndCreatesNoForgeState(t *testing.T) {
	root := fakeLifecycleRepo(t, "explorer", "mvp")
	before := snapshotMigrationFiles(t, root)
	code, output := captureChainOutput(t, func() int {
		return cmdMigrate([]string{"--to-lifecycle", "production", "--root", root})
	})
	if code != 0 {
		t.Fatalf("dry lifecycle promotion exit = %d, want 0; output:\n%s", code, output)
	}
	for _, want := range []string{"status=PLANNED", "auto mode migration: explorer -> engineering", "DRY run"} {
		if !strings.Contains(output, want) {
			t.Errorf("dry output missing %q:\n%s", want, output)
		}
	}
	requireMigrationFiles(t, root, before)
	requireNoForgeDir(t, root)
}

func TestMigrateLifecycle_ApplyExplorerPersistsPromotion(t *testing.T) {
	root := fakeLifecycleRepo(t, "explorer", "mvp")
	code, output := captureChainOutput(t, func() int {
		return cmdMigrate([]string{"--to-lifecycle", "production", "--apply", "--root", root})
	})
	if code != 0 || !strings.Contains(output, "status=APPLIED") {
		t.Fatalf("apply lifecycle promotion exit=%d output:\n%s", code, output)
	}
	project := readFileStr(t, filepath.Join(root, ".agent", "project.yml"))
	for _, want := range []string{
		"mode: engineering  # persistent selector",
		"lifecycle: production  # maturity",
		"project: demo", "owner: keep",
	} {
		if !strings.Contains(project, want) {
			t.Errorf("promoted project.yml missing %q:\n%s", want, project)
		}
	}
	roadmap := readFileStr(t, filepath.Join(root, ".agent", "ROADMAP.md"))
	if !strings.Contains(roadmap, "- [x] existing item") ||
		strings.Count(roadmap, "<!-- forge:migration-task:") != 5 {
		t.Errorf("explorer promotion did not preserve roadmap and inject five tasks:\n%s", roadmap)
	}
	if _, err := os.Stat(filepath.Join(root, ".forge", "migrations",
		"lifecycle-production.v1.json")); err != nil {
		t.Fatalf("terminal promotion receipt missing: %v", err)
	}
	if _, err := os.Stat(filepath.Join(root, ".forge", "migrations",
		"pending.v1.json")); !os.IsNotExist(err) {
		t.Fatalf("completed promotion retained pending intent: %v", err)
	}
}

func TestMigrateLifecycle_NonExplorerLeavesRoadmapUntouched(t *testing.T) {
	for _, currentMode := range []string{"balanced", "engineering", "cto"} {
		t.Run(currentMode, func(t *testing.T) {
			root := fakeLifecycleRepo(t, currentMode, "growth")
			before := snapshotMigrationFiles(t, root)
			code, output := captureChainOutput(t, func() int {
				return cmdMigrate([]string{
					"--to-lifecycle", "production", "--apply", "--root", root,
				})
			})
			if code != 0 || !strings.Contains(output, "mode unchanged: "+currentMode) {
				t.Fatalf("non-explorer apply exit=%d output:\n%s", code, output)
			}
			after := snapshotMigrationFiles(t, root)
			if after.roadmap != before.roadmap {
				t.Fatalf("%s promotion changed ROADMAP.md\nbefore=%q\nafter=%q",
					currentMode, before.roadmap, after.roadmap)
			}
			if !strings.Contains(after.project, "mode: "+currentMode) ||
				!strings.Contains(after.project, "lifecycle: production") {
				t.Errorf("non-explorer project selectors are wrong:\n%s", after.project)
			}
		})
	}
}

func TestMigrateLifecycle_ReplayIsByteStable(t *testing.T) {
	root := fakeLifecycleRepo(t, "explorer", "idea")
	apply := func() (int, string) {
		return captureChainOutput(t, func() int {
			return cmdMigrate([]string{
				"--to-lifecycle", "production", "--apply", "--root", root,
			})
		})
	}
	if code, output := apply(); code != 0 {
		t.Fatalf("initial apply exit=%d output:\n%s", code, output)
	}
	before := snapshotMigrationFiles(t, root)
	receiptPath := filepath.Join(root, ".forge", "migrations", "lifecycle-production.v1.json")
	receiptBefore := readFileStr(t, receiptPath)
	code, output := apply()
	if code != 0 || !strings.Contains(output, "status=REPLAYED") {
		t.Fatalf("replay exit=%d output:\n%s", code, output)
	}
	requireMigrationFiles(t, root, before)
	if got := readFileStr(t, receiptPath); got != receiptBefore {
		t.Fatalf("replay changed terminal receipt\nbefore=%q\nafter=%q", receiptBefore, got)
	}
}

func TestMigrateLifecycle_AlreadyProductionIsZeroWriteNoop(t *testing.T) {
	root := fakeLifecycleRepo(t, "balanced", "production")
	before := snapshotMigrationFiles(t, root)
	code, output := captureChainOutput(t, func() int {
		return cmdMigrate([]string{
			"--to-lifecycle", "production", "--apply", "--root", root,
		})
	})
	if code != 0 || !strings.Contains(output, "status=NOOP") {
		t.Fatalf("already-production apply exit=%d output:\n%s", code, output)
	}
	requireMigrationFiles(t, root, before)
	requireNoForgeDir(t, root)
}

func TestPendingLifecyclePromotionBlocksRunAndEvolveBeforeRunState(t *testing.T) {
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	writeFile(t, filepath.Join(root, ".agent", "project.yml"),
		"mode: explorer\nlifecycle: mvp\n")
	writeFile(t, filepath.Join(root, ".agent", "ROADMAP.md"), "# Roadmap\n")
	stateDir := filepath.Join(root, ".forge", "migrations")
	mkdir(t, stateDir)
	writeFile(t, filepath.Join(stateDir, "pending.v1.json"), "{}\n")
	before := snapshotMigrationFiles(t, root)
	calls := []struct {
		name string
		run  func() int
	}{
		{"run", func() int { return cmdRun([]string{"evolve", "--root", root}) }},
		{"evolve", func() int { return cmdEvolve([]string{"evolve", "--root", root}) }},
	}
	for _, tc := range calls {
		t.Run(tc.name, func(t *testing.T) {
			code, output := captureChainOutput(t, tc.run)
			if code != 1 ||
				!strings.Contains(output, "pending migration blocks execution") ||
				!strings.Contains(output, "forge migrate --to-lifecycle production --apply") {
				t.Fatalf("%s pending block exit=%d output:\n%s", tc.name, code, output)
			}
			requireMigrationFiles(t, root, before)
			requireOnlyPendingMigrationState(t, root)
		})
	}
}

func requireOnlyPendingMigrationState(t *testing.T, root string) {
	t.Helper()
	for dir, want := range map[string][]string{
		filepath.Join(root, ".forge"):               {"migrations"},
		filepath.Join(root, ".forge", "migrations"): {"pending.v1.json"},
	} {
		entries, err := os.ReadDir(dir)
		if err != nil {
			t.Fatalf("read state directory %s: %v", dir, err)
		}
		if len(entries) != len(want) {
			t.Fatalf("pending block created run state in %s: entries=%v, want=%v", dir, entries, want)
		}
		for index := range entries {
			if entries[index].Name() != want[index] {
				t.Fatalf("pending block created run state in %s: entries=%v, want=%v",
					dir, entries, want)
			}
		}
	}
}

func TestEvolveConsumesPersistedSelectors(t *testing.T) {
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	writeFile(t, filepath.Join(root, ".agent", "project.yml"),
		"mode: engineering\nlifecycle: growth\n")
	writeFile(t, filepath.Join(root, ".agent", "ROADMAP.md"), "# Roadmap\n")
	before := snapshotMigrationFiles(t, root)
	code, output := captureChainOutput(t, func() int {
		return cmdEvolve([]string{"evolve", "--root", root, "--max-iter", "1"})
	})
	if code != 0 {
		t.Fatalf("evolve with persisted selectors exit=%d output:\n%s", code, output)
	}
	checkpoint, found, err := persist.Load(checkpointPath(root))
	if err != nil || !found {
		t.Fatalf("persisted-selector checkpoint: found=%v err=%v", found, err)
	}
	if checkpoint.Mode != "engineering" || checkpoint.Lifecycle != "growth" {
		t.Fatalf("checkpoint selectors=%s/%s, want engineering/growth",
			checkpoint.Mode, checkpoint.Lifecycle)
	}
	requireMigrationFiles(t, root, before)
}

func TestTransientRunFlagsNeverPromotePersistentProject(t *testing.T) {
	calls := []struct {
		name string
		run  func(string) int
	}{
		{"run", func(root string) int {
			return cmdRun([]string{
				"evolve", "--root", root, "--mode", "explorer", "--lifecycle", "production",
			})
		}},
		{"evolve", func(root string) int {
			return cmdEvolve([]string{
				"evolve", "--root", root, "--mode", "explorer", "--lifecycle", "production",
				"--max-iter", "1",
			})
		}},
	}
	for _, tc := range calls {
		t.Run(tc.name, func(t *testing.T) {
			root := fakeRepo(t, "evolve", externalAgentWorkflow)
			writeFile(t, filepath.Join(root, ".agent", "project.yml"),
				"mode: balanced\nlifecycle: mvp\n")
			writeFile(t, filepath.Join(root, ".agent", "ROADMAP.md"), "# Roadmap\n")
			before := snapshotMigrationFiles(t, root)
			if code, output := captureChainOutput(t, func() int { return tc.run(root) }); code != 0 {
				t.Fatalf("%s with transient selectors exit=%d output:\n%s", tc.name, code, output)
			}
			requireMigrationFiles(t, root, before)
			if _, err := os.Stat(filepath.Join(root, ".forge", "migrations")); !os.IsNotExist(err) {
				t.Fatalf("%s transient selectors created migration state: %v", tc.name, err)
			}
		})
	}
}
