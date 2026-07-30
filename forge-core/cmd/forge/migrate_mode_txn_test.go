package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func runManualModeMigration(t *testing.T, root string, apply bool) (int, string) {
	t.Helper()
	args := []string{"--to", "engineering", "--root", root}
	if apply {
		args = append(args, "--apply")
	}
	return captureChainOutput(t, func() int { return cmdMigrate(args) })
}

func migrationStateSnapshot(t *testing.T, root string) string {
	t.Helper()
	dir := filepath.Join(root, ".forge", "migrations")
	entries, err := os.ReadDir(dir)
	if os.IsNotExist(err) {
		return ""
	}
	if err != nil {
		t.Fatalf("read migration state: %v", err)
	}
	var snapshot strings.Builder
	for _, entry := range entries {
		if entry.IsDir() {
			t.Fatalf("unexpected migration state directory %q", entry.Name())
		}
		data, err := os.ReadFile(filepath.Join(dir, entry.Name()))
		if err != nil {
			t.Fatalf("read migration state %q: %v", entry.Name(), err)
		}
		snapshot.WriteString(entry.Name())
		snapshot.WriteByte(0)
		snapshot.Write(data)
		snapshot.WriteByte(0)
	}
	return snapshot.String()
}

func requireNoMigrationState(t *testing.T, root string) {
	t.Helper()
	if got := migrationStateSnapshot(t, root); got != "" {
		t.Fatalf("operation created migration state: %q", got)
	}
}

func TestManualModeMigration_DryIsZeroWrite(t *testing.T) {
	root := fakeLifecycleRepo(t, "explorer", "mvp")
	before := snapshotMigrationFiles(t, root)
	code, output := runManualModeMigration(t, root, false)
	if code != 0 || !strings.Contains(output, "status=PLANNED") {
		t.Fatalf("manual dry exit=%d output:\n%s", code, output)
	}
	requireMigrationFiles(t, root, before)
	requireNoForgeDir(t, root)
}

func TestManualModeMigration_ApplyThenReplayIsByteStable(t *testing.T) {
	root := fakeLifecycleRepo(t, "explorer", "growth")
	code, output := runManualModeMigration(t, root, true)
	if code != 0 || !strings.Contains(output, "status=APPLIED") {
		t.Fatalf("manual apply exit=%d output:\n%s", code, output)
	}
	afterApply := snapshotMigrationFiles(t, root)
	if !strings.Contains(afterApply.project, "mode: engineering") ||
		!strings.Contains(afterApply.project, "lifecycle: growth") ||
		strings.Count(afterApply.roadmap, "<!-- forge:migration-task:") != 5 {
		t.Fatalf("manual apply produced wrong tracked state: %+v", afterApply)
	}
	stateAfterApply := migrationStateSnapshot(t, root)
	if stateAfterApply == "" || strings.Contains(stateAfterApply, "pending.v1.json") {
		t.Fatalf("manual apply terminal state = %q", stateAfterApply)
	}

	code, output = runManualModeMigration(t, root, true)
	if code != 0 || !strings.Contains(output, "status=REPLAYED") {
		t.Fatalf("manual replay exit=%d output:\n%s", code, output)
	}
	requireMigrationFiles(t, root, afterApply)
	if got := migrationStateSnapshot(t, root); got != stateAfterApply {
		t.Fatalf("manual replay changed terminal state\nbefore=%q\nafter=%q", stateAfterApply, got)
	}
}

func TestManualModeMigration_NonExplorerFailsClosed(t *testing.T) {
	for _, mode := range []string{"balanced", "cto"} {
		for _, apply := range []bool{false, true} {
			name := mode + "/dry"
			if apply {
				name = mode + "/apply"
			}
			t.Run(name, func(t *testing.T) {
				root := fakeLifecycleRepo(t, mode, "mvp")
				before := snapshotMigrationFiles(t, root)
				if code, _ := runManualModeMigration(t, root, apply); code != 1 {
					t.Fatalf("manual migration from %s apply=%v exit=%d, want 1", mode, apply, code)
				}
				requireMigrationFiles(t, root, before)
				requireNoMigrationState(t, root)
			})
		}
	}
}

func TestManualModeMigration_AlreadyEngineeringWithoutReceiptIsNoop(t *testing.T) {
	root := fakeLifecycleRepo(t, "engineering", "growth")
	before := snapshotMigrationFiles(t, root)
	code, output := runManualModeMigration(t, root, true)
	if code != 0 || !strings.Contains(output, "status=NOOP") {
		t.Fatalf("already-engineering apply exit=%d output:\n%s", code, output)
	}
	requireMigrationFiles(t, root, before)
	requireNoMigrationState(t, root)
}

func TestManualModeMigration_RejectsTrackedFileAliases(t *testing.T) {
	for _, target := range []string{"project.yml", "ROADMAP.md"} {
		for _, kind := range []string{"symlink", "hardlink"} {
			t.Run(target+"/"+kind, func(t *testing.T) {
				root := fakeLifecycleRepo(t, "explorer", "mvp")
				before := snapshotMigrationFiles(t, root)
				path, outside := installTrackedAlias(t, root, target, kind)
				outsideBefore := readFileStr(t, outside)
				code, output := runManualModeMigration(t, root, true)
				if code != 1 {
					t.Fatalf("%s %s apply exit=%d, want 1; output:\n%s",
						target, kind, code, output)
				}
				if got := readFileStr(t, outside); got != outsideBefore {
					t.Fatalf("%s alias mutated outside file\nbefore=%q\nafter=%q",
						kind, outsideBefore, got)
				}
				requireTrackedAlias(t, path, outside, kind)
				requireMigrationFiles(t, root, before)
				requireNoMigrationState(t, root)
			})
		}
	}
}

func installTrackedAlias(t *testing.T, root, name, kind string) (string, string) {
	t.Helper()
	path := filepath.Join(root, ".agent", name)
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	outside := filepath.Join(t.TempDir(), name)
	if err := os.WriteFile(outside, data, 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
	if kind == "symlink" {
		if err := os.Symlink(outside, path); err != nil {
			t.Skipf("symlink unavailable: %v", err)
		}
	} else if err := os.Link(outside, path); err != nil {
		t.Skipf("hardlink unavailable: %v", err)
	}
	return path, outside
}

func requireTrackedAlias(t *testing.T, path, outside, kind string) {
	t.Helper()
	if kind == "symlink" {
		info, err := os.Lstat(path)
		if err != nil || info.Mode()&os.ModeSymlink == 0 {
			t.Fatalf("symlink target changed: info=%v err=%v", info, err)
		}
		return
	}
	targetInfo, targetErr := os.Stat(path)
	outsideInfo, outsideErr := os.Stat(outside)
	if targetErr != nil || outsideErr != nil || !os.SameFile(targetInfo, outsideInfo) {
		t.Fatalf("hardlink target changed: target=%v/%v outside=%v/%v",
			targetInfo, targetErr, outsideInfo, outsideErr)
	}
}

func TestManualModeMigration_RefusesLifecyclePendingAndHintRecovers(t *testing.T) {
	root := fakeLifecycleRepo(t, "explorer", "mvp")
	before := snapshotMigrationFiles(t, root)
	seedRecoverableLifecyclePending(t, root)
	pendingPath := filepath.Join(root, ".forge", "migrations", "pending.v1.json")
	pendingBefore := readFileStr(t, pendingPath)

	for _, apply := range []bool{false, true} {
		code, output := runManualModeMigration(t, root, apply)
		if code != 1 ||
			!strings.Contains(output, "forge migrate --to-lifecycle production --apply") {
			t.Fatalf("manual apply=%v against lifecycle pending exit=%d output:\n%s",
				apply, code, output)
		}
		requireMigrationFiles(t, root, before)
		if got := readFileStr(t, pendingPath); got != pendingBefore {
			t.Fatalf("manual apply=%v changed lifecycle pending intent", apply)
		}
	}

	code, output := captureChainOutput(t, func() int {
		return cmdMigrate([]string{
			"--to-lifecycle", "production", "--apply", "--root", root,
		})
	})
	if code != 0 || !strings.Contains(output, "status=RECOVERED") {
		t.Fatalf("hinted lifecycle recovery exit=%d output:\n%s", code, output)
	}
	if _, err := os.Stat(pendingPath); !os.IsNotExist(err) {
		t.Fatalf("hinted recovery retained lifecycle pending intent: %v", err)
	}
	recovered := snapshotMigrationFiles(t, root)
	if !strings.Contains(recovered.project, "mode: engineering") ||
		!strings.Contains(recovered.project, "lifecycle: production") {
		t.Fatalf("hinted lifecycle recovery produced wrong selectors: %q", recovered.project)
	}
}

func seedRecoverableLifecyclePending(t *testing.T, root string) {
	t.Helper()
	agentDir := filepath.Join(root, ".agent")
	info, err := os.Stat(agentDir)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = os.Chmod(agentDir, info.Mode().Perm()) })
	if err := os.Chmod(agentDir, 0o555); err != nil {
		t.Skipf("cannot make tracked directory read-only: %v", err)
	}
	code, output := captureChainOutput(t, func() int {
		return cmdMigrate([]string{
			"--to-lifecycle", "production", "--apply", "--root", root,
		})
	})
	if err := os.Chmod(agentDir, info.Mode().Perm()); err != nil {
		t.Fatal(err)
	}
	pending := filepath.Join(root, ".forge", "migrations", "pending.v1.json")
	if code == 0 {
		t.Skip("filesystem permissions did not induce a recoverable pending transaction")
	}
	if _, err := os.Stat(pending); err != nil {
		t.Fatalf("failed lifecycle apply did not leave pending intent: %v; output:\n%s", err, output)
	}
}
