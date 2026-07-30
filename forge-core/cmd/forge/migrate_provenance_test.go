package main

import (
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

const malformedTrackedMigrationState = "attacker-controlled, not canonical JSON\n"

func requireTrackedMigrationGit(t *testing.T) {
	t.Helper()
	if runtime.GOOS != "linux" {
		t.Skip("tracked control-state provenance uses the verified Linux host-Git boundary")
	}
	if _, err := exec.LookPath("git"); err != nil {
		t.Skip("git unavailable")
	}
}

func commitTrackedMigrationControl(t *testing.T, root, controlRel string) {
	t.Helper()
	controlPath := filepath.Join(root, filepath.FromSlash(controlRel))
	mkdir(t, filepath.Dir(controlPath))
	writeFile(t, controlPath, malformedTrackedMigrationState)
	mustGit(t, root, "init", "-q")
	mustGit(t, root, "config", "user.name", "Forge Test")
	mustGit(t, root, "config", "user.email", "forge-test@example.invalid")
	mustGit(t, root, "add", "-A", "--", ".agent")
	mustGit(t, root, "add", "-f", "--", controlRel)
	mustGit(t, root, "commit", "-q", "-m", "tracked control fixture")
	mustGit(t, root, "ls-files", "--error-unmatch", "--", controlRel)
}

type trackedMigrationSnapshot struct {
	files   migrationFiles
	control string
	index   string
	status  string
}

func snapshotTrackedMigrationRepo(t *testing.T, root string) trackedMigrationSnapshot {
	t.Helper()
	return trackedMigrationSnapshot{
		files:   snapshotMigrationFiles(t, root),
		control: migrationStateSnapshot(t, root),
		index: string(mustGitOutput(t, root,
			"ls-files", "--stage", "-z")),
		status: string(mustGitOutput(t, root,
			"status", "--porcelain=v1", "--untracked-files=all")),
	}
}

func requireTrackedMigrationRepo(t *testing.T, root string, want trackedMigrationSnapshot) {
	t.Helper()
	requireMigrationFiles(t, root, want.files)
	if got := migrationStateSnapshot(t, root); got != want.control {
		t.Fatalf("tracked control state changed\nbefore=%q\nafter=%q", want.control, got)
	}
	if got := string(mustGitOutput(t, root, "ls-files", "--stage", "-z")); got != want.index {
		t.Fatalf("migration changed Git index\nbefore=%q\nafter=%q", want.index, got)
	}
	if got := string(mustGitOutput(t, root,
		"status", "--porcelain=v1", "--untracked-files=all")); got != want.status {
		t.Fatalf("migration changed tracked repository status\nbefore=%q\nafter=%q",
			want.status, got)
	}
}

type trackedMigrationOperation struct {
	name        string
	targetArgs  []string
	receiptPath string
}

func testTrackedControlMigrate(
	t *testing.T,
	operation trackedMigrationOperation,
	apply bool,
	controlPath string,
) {
	t.Helper()
	root := fakeLifecycleRepo(t, "explorer", "mvp")
	commitTrackedMigrationControl(t, root, controlPath)
	before := snapshotTrackedMigrationRepo(t, root)
	args := append([]string(nil), operation.targetArgs...)
	if apply {
		args = append(args, "--apply")
	}
	args = append(args, "--root", root)
	code, output := captureChainOutput(t, func() int { return cmdMigrate(args) })
	if code != 1 || !strings.Contains(output, "tracked Forge control state") {
		t.Fatalf("tracked control migrate exit=%d output:\n%s", code, output)
	}
	if strings.Contains(output, "decode terminal") ||
		strings.Contains(output, "pending migration requires") {
		t.Fatalf("tracked control was decoded before provenance rejection:\n%s", output)
	}
	requireTrackedMigrationRepo(t, root, before)
}

func TestMigrateRejectsTrackedControlBeforeStateDecodeOrWrite(t *testing.T) {
	requireTrackedMigrationGit(t)
	operations := []trackedMigrationOperation{
		{
			name:        "manual",
			targetArgs:  []string{"--to", "engineering"},
			receiptPath: ".forge/migrations/mode-engineering.v1.json",
		},
		{
			name:        "lifecycle",
			targetArgs:  []string{"--to-lifecycle", "production"},
			receiptPath: ".forge/migrations/lifecycle-production.v1.json",
		},
	}
	for _, operation := range operations {
		for _, apply := range []bool{false, true} {
			runMode := "dry"
			if apply {
				runMode = "apply"
			}
			controls := []struct{ name, path string }{
				{"pending decode", ".forge/migrations/pending.v1.json"},
				{"receipt decode", operation.receiptPath},
				{"state write", ".forge/migrations/operator-owned.state"},
			}
			for _, control := range controls {
				t.Run(operation.name+"/"+runMode+"/"+control.name, func(t *testing.T) {
					testTrackedControlMigrate(t, operation, apply, control.path)
				})
			}
		}
	}
}

func TestMigrateRejectsInvalidTargetBeforeProvenanceInspection(t *testing.T) {
	requireTrackedMigrationGit(t)
	cases := []struct {
		name string
		args []string
	}{
		{"manual", []string{"--to", "balanced"}},
		{"lifecycle", []string{"--to-lifecycle", "growth"}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			root := fakeLifecycleRepo(t, "explorer", "mvp")
			commitTrackedMigrationControl(
				t, root, ".forge/migrations/pending.v1.json",
			)
			before := snapshotTrackedMigrationRepo(t, root)
			args := append(append([]string(nil), tc.args...), "--root", root)
			code, output := captureChainOutput(t, func() int {
				return cmdMigrate(args)
			})
			if code != 2 || !strings.Contains(output, "unsupported") {
				t.Fatalf("invalid target exit=%d output:\n%s", code, output)
			}
			if strings.Contains(output, "unsafe control-state provenance") {
				t.Fatalf("invalid target inspected repository provenance:\n%s", output)
			}
			requireTrackedMigrationRepo(t, root, before)
		})
	}
}

func TestRunAndEvolveRejectTrackedPendingBeforeRecovery(t *testing.T) {
	requireTrackedMigrationGit(t)
	calls := []struct {
		name string
		run  func(string) int
	}{
		{"run", func(root string) int {
			return cmdRun([]string{"evolve", "--root", root})
		}},
		{"evolve", func(root string) int {
			return cmdEvolve([]string{"evolve", "--root", root, "--max-iter", "1"})
		}},
	}
	for _, call := range calls {
		t.Run(call.name, func(t *testing.T) {
			root := fakeRepo(t, "evolve", externalAgentWorkflow)
			writeFile(t, filepath.Join(root, ".agent", "project.yml"),
				"mode: explorer\nlifecycle: mvp\n")
			writeFile(t, filepath.Join(root, ".agent", "ROADMAP.md"), "# Roadmap\n")
			commitTrackedMigrationControl(
				t, root, ".forge/migrations/pending.v1.json",
			)
			before := snapshotTrackedMigrationRepo(t, root)
			code, output := captureChainOutput(t, func() int { return call.run(root) })
			if code != 1 ||
				!strings.Contains(output, "tracked Forge control state") {
				t.Fatalf("%s tracked pending exit=%d output:\n%s", call.name, code, output)
			}
			if strings.Contains(output, "pending migration blocks execution") ||
				strings.Contains(output, "recover with") {
				t.Fatalf("%s offered recovery before provenance rejection:\n%s",
					call.name, output)
			}
			requireTrackedMigrationRepo(t, root, before)
		})
	}
}
