package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/persist"
)

type optionalMigrationFile struct {
	present bool
	data    string
}

func snapshotOptionalMigrationFile(t *testing.T, path string) optionalMigrationFile {
	t.Helper()
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		return optionalMigrationFile{}
	}
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	return optionalMigrationFile{present: true, data: string(data)}
}

type completedMigrationSnapshot struct {
	project    optionalMigrationFile
	roadmap    optionalMigrationFile
	runLock    optionalMigrationFile
	trace      optionalMigrationFile
	checkpoint optionalMigrationFile
	chain      optionalMigrationFile
	memory     optionalMigrationFile
	state      string
}

func snapshotCompletedMigration(t *testing.T, root string) completedMigrationSnapshot {
	t.Helper()
	return completedMigrationSnapshot{
		project: snapshotOptionalMigrationFile(
			t, filepath.Join(root, ".agent", "project.yml"),
		),
		roadmap: snapshotOptionalMigrationFile(
			t, filepath.Join(root, ".agent", "ROADMAP.md"),
		),
		runLock: snapshotOptionalMigrationFile(
			t, filepath.Join(root, ".forge", "run.lock"),
		),
		trace: snapshotOptionalMigrationFile(
			t, filepath.Join(root, ".forge", "trace.jsonl"),
		),
		checkpoint: snapshotOptionalMigrationFile(
			t, filepath.Join(root, ".forge", "checkpoint.json"),
		),
		chain: snapshotOptionalMigrationFile(
			t, filepath.Join(root, ".forge", "chain-state.json"),
		),
		memory: snapshotOptionalMigrationFile(
			t, filepath.Join(root, ".forge", "memory.jsonl"),
		),
		state: migrationStateSnapshot(t, root),
	}
}

func requireCompletedMigrationSnapshot(
	t *testing.T,
	root string,
	want completedMigrationSnapshot,
) {
	t.Helper()
	got := snapshotCompletedMigration(t, root)
	if got != want {
		t.Fatalf("execution guard mutated completed migration state\nbefore=%+v\nafter=%+v",
			want, got)
	}
}

func newCompletedLifecycleExecutionRepo(t *testing.T) (string, string, string) {
	t.Helper()
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	writeFile(t, filepath.Join(root, ".agent", "project.yml"),
		"project: guard-e2e\nmode: explorer\nlifecycle: mvp\n")
	writeFile(t, filepath.Join(root, ".agent", "ROADMAP.md"), "# Roadmap\n")
	code, output := captureChainOutput(t, func() int {
		return cmdMigrate([]string{
			"--to-lifecycle", "production", "--apply", "--root", root,
		})
	})
	if code != 0 || !strings.Contains(output, "status=APPLIED") {
		t.Fatalf("seed lifecycle promotion exit=%d output:\n%s", code, output)
	}
	sentinel := filepath.Join(root, "migration-guard-agent-ran")
	agent := filepath.Join(root, "migration-guard-agent")
	writeFile(t, agent, "#!/bin/sh\n: > \""+sentinel+"\"\n")
	if err := os.Chmod(agent, 0o700); err != nil {
		t.Fatal(err)
	}
	return root, agent, sentinel
}

func replaceCompletedMigrationText(t *testing.T, path, old, replacement string) {
	t.Helper()
	data := readFileStr(t, path)
	if !strings.Contains(data, old) {
		t.Fatalf("completed migration fixture %s lacks %q", path, old)
	}
	if err := os.WriteFile(path,
		[]byte(strings.Replace(data, old, replacement, 1)), 0o600); err != nil {
		t.Fatal(err)
	}
}

type completedMigrationDrift struct {
	name  string
	apply func(*testing.T, string)
}

func completedMigrationDrifts() []completedMigrationDrift {
	return []completedMigrationDrift{
		{"mode rollback", func(t *testing.T, root string) {
			replaceCompletedMigrationText(t, filepath.Join(root, ".agent", "project.yml"),
				"mode: engineering", "mode: explorer")
		}},
		{"lifecycle rollback", func(t *testing.T, root string) {
			replaceCompletedMigrationText(t, filepath.Join(root, ".agent", "project.yml"),
				"lifecycle: production", "lifecycle: mvp")
		}},
		{"project deleted", func(t *testing.T, root string) {
			if err := os.Remove(filepath.Join(root, ".agent", "project.yml")); err != nil {
				t.Fatal(err)
			}
		}},
		{"task marker deleted", func(t *testing.T, root string) {
			replaceCompletedMigrationText(t, filepath.Join(root, ".agent", "ROADMAP.md"),
				"<!-- forge:migration-task:add-ci -->", "")
		}},
		{"task marker tampered", func(t *testing.T, root string) {
			replaceCompletedMigrationText(t, filepath.Join(root, ".agent", "ROADMAP.md"),
				"forge:migration-task:add-monitoring", "forge:migration-task:operator-edited")
		}},
		{"receipt malformed", func(t *testing.T, root string) {
			path := filepath.Join(
				root, ".forge", "migrations", "lifecycle-production.v1.json",
			)
			if err := os.WriteFile(path, []byte("{"), 0o600); err != nil {
				t.Fatal(err)
			}
		}},
	}
}

func runMigrationGuardCommand(
	t *testing.T,
	command, root, agent string,
) (int, string) {
	t.Helper()
	return captureChainOutput(t, func() int {
		if command == "run" {
			return cmdRun([]string{
				"evolve", "--root", root,
				"--executor", "command", "--agent-cmd", agent,
			})
		}
		return cmdEvolve([]string{
			"evolve", "--root", root, "--max-iter", "1",
			"--executor", "command", "--agent-cmd", agent,
		})
	})
}

func requireNoMigrationGuardExecution(t *testing.T, sentinel string) {
	t.Helper()
	if _, err := os.Stat(sentinel); !os.IsNotExist(err) {
		t.Fatalf("completed migration guard executed agent: %v", err)
	}
}

func testCompletedMigrationExecutionGuard(
	t *testing.T,
	drift completedMigrationDrift,
	command string,
) {
	t.Helper()
	root, agent, sentinel := newCompletedLifecycleExecutionRepo(t)
	drift.apply(t, root)
	before := snapshotCompletedMigration(t, root)
	code, output := runMigrationGuardCommand(t, command, root, agent)
	if code != 1 || !strings.Contains(output, "completed migration state is invalid") {
		t.Fatalf("%s accepted completed migration %s exit=%d output:\n%s",
			command, drift.name, code, output)
	}
	requireCompletedMigrationSnapshot(t, root, before)
	requireNoMigrationGuardExecution(t, sentinel)
}

func TestRunAndEvolveRejectCompletedLifecycleMigrationDriftAtEntry(t *testing.T) {
	for _, drift := range completedMigrationDrifts() {
		for _, command := range []string{"run", "evolve"} {
			t.Run(drift.name+"/"+command, func(t *testing.T) {
				testCompletedMigrationExecutionGuard(t, drift, command)
			})
		}
	}
}

func TestCompletedMigrationGuardPrecedesWorkflowLoad(t *testing.T) {
	for _, command := range []string{"run", "evolve"} {
		t.Run(command, func(t *testing.T) {
			root, agent, sentinel := newCompletedLifecycleExecutionRepo(t)
			completedMigrationDrifts()[0].apply(t, root)
			if err := os.Remove(filepath.Join(
				root, ".agent", "workflows", "evolve.yml",
			)); err != nil {
				t.Fatal(err)
			}
			before := snapshotCompletedMigration(t, root)
			code, output := runMigrationGuardCommand(t, command, root, agent)
			if code != 1 ||
				!strings.Contains(output, "completed migration state is invalid") {
				t.Fatalf("%s loaded workflow before migration guard exit=%d output:\n%s",
					command, code, output)
			}
			requireCompletedMigrationSnapshot(t, root, before)
			requireNoMigrationGuardExecution(t, sentinel)
		})
	}
}

func TestRunAndEvolveAllowLegacyMissingProjectWithoutReceipt(t *testing.T) {
	for _, command := range []string{"run", "evolve"} {
		t.Run(command, func(t *testing.T) {
			root := fakeRepo(t, "evolve", externalAgentWorkflow)
			code, output := captureChainOutput(t, func() int {
				if command == "run" {
					return cmdRun([]string{"evolve", "--root", root})
				}
				return cmdEvolve([]string{
					"evolve", "--root", root, "--max-iter", "1",
				})
			})
			if code != 0 || !strings.Contains(output, "mode=balanced") {
				t.Fatalf("legacy %s defaults exit=%d output:\n%s", command, code, output)
			}
			if command == "run" && !strings.Contains(output, "lifecycle=mvp") {
				t.Fatalf("legacy run did not resolve lifecycle=mvp:\n%s", output)
			}
			if command == "evolve" {
				checkpoint, found, err := persist.Load(checkpointPath(root))
				if err != nil || !found ||
					checkpoint.Mode != "balanced" || checkpoint.Lifecycle != "mvp" {
					t.Fatalf("legacy evolve checkpoint=%+v found=%v err=%v",
						checkpoint, found, err)
				}
			}
			if _, err := os.Stat(filepath.Join(root, ".agent", "project.yml")); !os.IsNotExist(err) {
				t.Fatalf("legacy %s invented project.yml: %v", command, err)
			}
			if _, err := os.Stat(filepath.Join(root, ".forge", "migrations")); !os.IsNotExist(err) {
				t.Fatalf("legacy %s invented migration state: %v", command, err)
			}
		})
	}
}
