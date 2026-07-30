package migrationtxn

import (
	"testing"

	"forgeos/forge-core/internal/migrate"
)

func TestManualCrashRecoveryAcrossDurableStages(t *testing.T) {
	stages := []manualCrashStage{
		manualCrashAfterIntent,
		manualCrashAfterRoadmap,
		manualCrashAfterProject,
		manualCrashAfterReceipt,
	}
	for _, stage := range stages {
		t.Run(string(stage), func(t *testing.T) {
			capture := captureManualTransaction(t)
			capture.resetToCrashStage(t, stage)
			assertPending(t, capture.root, true)

			recovered, err := ApplyManual(capture.root)
			if err != nil {
				t.Fatalf("ApplyManual recovery after %s: %v", stage, err)
			}
			assertManualResult(t, recovered, StatusRecovered,
				"explorer", "engineering", "mvp", true, len(expectedPromotionTaskIDs))
			assertManuallyMigrated(t, capture.root, migrate.LifecycleMVP)
		})
	}
}

func TestManualAPIRejectsLifecyclePendingOperation(t *testing.T) {
	root := newRepository(t, migrate.ModeExplorer, migrate.LifecycleMVP, []byte("# Roadmap\n"))
	ensureTestForgeDir(t, root)
	projectBefore := readTestFile(t, projectPath(root))
	roadmapBefore := readTestFile(t, roadmapPath(root))
	seedPendingBeforeRoadmap(t, root)

	if _, err := PreviewManual(root); err == nil {
		t.Fatal("PreviewManual accepted lifecycle pending operation")
	}
	if _, err := ApplyManual(root); err == nil {
		t.Fatal("ApplyManual recovered mismatched lifecycle pending operation")
	}
	assertBytesEqual(t, "project", readTestFile(t, projectPath(root)), projectBefore)
	assertBytesEqual(t, "roadmap", readTestFile(t, roadmapPath(root)), roadmapBefore)
	assertPending(t, root, true)

	recovered, err := Apply(root, productionRequest())
	if err != nil {
		t.Fatalf("matching lifecycle Apply recovery: %v", err)
	}
	assertResult(t, recovered, StatusRecovered,
		"explorer", "engineering", "mvp", true, len(expectedPromotionTaskIDs))
}

func TestLifecycleAPIRejectsManualPendingOperation(t *testing.T) {
	capture := captureManualTransaction(t)
	capture.resetToCrashStage(t, manualCrashAfterIntent)
	projectBefore := readTestFile(t, projectPath(capture.root))
	roadmapBefore := readTestFile(t, roadmapPath(capture.root))

	if _, err := Preview(capture.root, productionRequest()); err == nil {
		t.Fatal("lifecycle Preview accepted manual pending operation")
	}
	if _, err := Apply(capture.root, productionRequest()); err == nil {
		t.Fatal("lifecycle Apply recovered mismatched manual pending operation")
	}
	assertBytesEqual(t, "project", readTestFile(t, projectPath(capture.root)), projectBefore)
	assertBytesEqual(t, "roadmap", readTestFile(t, roadmapPath(capture.root)), roadmapBefore)
	assertPending(t, capture.root, true)

	recovered, err := ApplyManual(capture.root)
	if err != nil {
		t.Fatalf("matching ApplyManual recovery: %v", err)
	}
	assertManualResult(t, recovered, StatusRecovered,
		"explorer", "engineering", "mvp", true, len(expectedPromotionTaskIDs))
	assertManuallyMigrated(t, capture.root, migrate.LifecycleMVP)
}
