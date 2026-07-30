package migrationtxn

import (
	"bytes"
	"os"
	"strings"
	"testing"

	"forgeos/forge-core/internal/migrate"
)

func TestValidateExecutionStateAllowsLegacyRepositoriesWithoutReceipts(t *testing.T) {
	for _, root := range []string{
		t.TempDir(),
		newRawRepository(t, []byte("malformed: project\n"), nil),
	} {
		if err := ValidateExecutionState(root); err != nil {
			t.Fatalf("legacy repository rejected: %v", err)
		}
	}
}

func TestValidateExecutionStateAcceptsCompletedAndComposedMigrations(t *testing.T) {
	lifecycleRoot := newRepository(
		t, migrate.ModeExplorer, migrate.LifecycleMVP, []byte("# Roadmap\n"),
	)
	if _, err := Apply(lifecycleRoot, productionRequest()); err != nil {
		t.Fatal(err)
	}
	if err := ValidateExecutionState(lifecycleRoot); err != nil {
		t.Fatalf("completed lifecycle migration rejected: %v", err)
	}

	composedRoot := newRepository(
		t, migrate.ModeExplorer, migrate.LifecycleGrowth, []byte("# Roadmap\n"),
	)
	if _, err := ApplyManual(composedRoot); err != nil {
		t.Fatal(err)
	}
	if _, err := Apply(composedRoot, productionRequest()); err != nil {
		t.Fatal(err)
	}
	if err := ValidateExecutionState(composedRoot); err != nil {
		t.Fatalf("composed manual+lifecycle migration rejected: %v", err)
	}
}

func TestValidateExecutionStateRejectsTerminalStateDrift(t *testing.T) {
	cases := []struct {
		name  string
		drift func(*testing.T, string)
	}{
		{"mode", driftMigratedMode},
		{"lifecycle", driftMigratedLifecycle},
		{"missing-project", removeMigratedProject},
		{"roadmap-marker", driftMigratedRoadmap},
		{"receipt", corruptMigratedReceipt},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			root := newRepository(
				t, migrate.ModeExplorer, migrate.LifecycleMVP, []byte("# Roadmap\n"),
			)
			if _, err := Apply(root, productionRequest()); err != nil {
				t.Fatal(err)
			}
			testCase.drift(t, root)
			if err := ValidateExecutionState(root); err == nil {
				t.Fatal("terminal migration drift was accepted")
			}
		})
	}
}

func driftMigratedMode(t *testing.T, root string) {
	t.Helper()
	path := projectPath(root)
	data := bytes.Replace(
		readTestFile(t, path), []byte("mode: engineering"), []byte("mode: explorer"), 1,
	)
	writeTestFile(t, path, data, testFileMode(t, path))
}

func driftMigratedLifecycle(t *testing.T, root string) {
	t.Helper()
	path := projectPath(root)
	data := bytes.Replace(
		readTestFile(t, path), []byte("lifecycle: production"), []byte("lifecycle: mvp"), 1,
	)
	writeTestFile(t, path, data, testFileMode(t, path))
}

func removeMigratedProject(t *testing.T, root string) {
	t.Helper()
	if err := os.Remove(projectPath(root)); err != nil {
		t.Fatal(err)
	}
}

func driftMigratedRoadmap(t *testing.T, root string) {
	t.Helper()
	path := roadmapPath(root)
	data := strings.Replace(
		string(readTestFile(t, path)), "forge:migration-task:add-ci", "removed-task", 1,
	)
	writeTestFile(t, path, []byte(data), testFileMode(t, path))
}

func corruptMigratedReceipt(t *testing.T, root string) {
	t.Helper()
	writeTestFile(t, receiptPath(root), []byte("{"), 0o600)
}
