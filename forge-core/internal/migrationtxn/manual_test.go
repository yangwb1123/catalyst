package migrationtxn

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	"forgeos/forge-core/internal/migrate"
)

func TestManualExplorerMigrationDryApplyReplay(t *testing.T) {
	initialRoadmap := []byte("# Roadmap\n\n- [ ] existing item\n")
	root := newRepository(t, migrate.ModeExplorer, migrate.LifecycleMVP, initialRoadmap)
	initialProject := readTestFile(t, projectPath(root))

	preview, err := PreviewManual(root)
	if err != nil {
		t.Fatalf("PreviewManual: %v", err)
	}
	assertManualResult(t, preview, StatusPlanned,
		"explorer", "engineering", "mvp", true, len(expectedPromotionTaskIDs))
	assertBytesEqual(t, "project after manual preview",
		readTestFile(t, projectPath(root)), initialProject)
	assertBytesEqual(t, "roadmap after manual preview",
		readTestFile(t, roadmapPath(root)), initialRoadmap)
	assertPathMissing(t, filepath.Join(root, ".forge"))

	applied, err := ApplyManual(root)
	if err != nil {
		t.Fatalf("ApplyManual: %v", err)
	}
	assertManualResult(t, applied, StatusApplied,
		"explorer", "engineering", "mvp", true, len(expectedPromotionTaskIDs))
	assertManuallyMigrated(t, root, migrate.LifecycleMVP)
	projectAfter := readTestFile(t, projectPath(root))
	roadmapAfter := readTestFile(t, roadmapPath(root))
	manualPath, _ := assertManualReceipt(t, root, migrate.LifecycleMVP)
	receiptAfter := readTestFile(t, manualPath)

	replayed, err := ApplyManual(root)
	if err != nil {
		t.Fatalf("replay ApplyManual: %v", err)
	}
	assertManualResult(t, replayed, StatusReplayed,
		"explorer", "engineering", "mvp", true, len(expectedPromotionTaskIDs))
	assertBytesEqual(t, "project after manual replay",
		readTestFile(t, projectPath(root)), projectAfter)
	assertBytesEqual(t, "roadmap after manual replay",
		readTestFile(t, roadmapPath(root)), roadmapAfter)
	assertBytesEqual(t, "manual receipt after replay", readTestFile(t, manualPath), receiptAfter)
}

func TestManualMigrationRejectsUnsupportedSourceModes(t *testing.T) {
	for _, mode := range []string{migrate.ModeBalanced, migrate.ModeCTO, "unknown"} {
		t.Run(mode, func(t *testing.T) {
			root := newRepository(t, mode, migrate.LifecycleMVP, []byte("# Roadmap\n"))
			projectBefore := readTestFile(t, projectPath(root))
			roadmapBefore := readTestFile(t, roadmapPath(root))
			if _, err := PreviewManual(root); err == nil {
				t.Fatal("PreviewManual accepted unsupported source mode")
			}
			if _, err := ApplyManual(root); err == nil {
				t.Fatal("ApplyManual accepted unsupported source mode")
			}
			assertBytesEqual(t, "project", readTestFile(t, projectPath(root)), projectBefore)
			assertBytesEqual(t, "roadmap", readTestFile(t, roadmapPath(root)), roadmapBefore)
			assertPathMissing(t, filepath.Join(root, ".forge"))
		})
	}
}

func TestManualMigrationAlreadyEngineeringWithoutReceiptIsNoop(t *testing.T) {
	root := newRepository(t, migrate.ModeEngineering, migrate.LifecycleGrowth,
		[]byte("# Must remain untouched\n"))
	projectBefore := readTestFile(t, projectPath(root))
	roadmapBefore := readTestFile(t, roadmapPath(root))

	preview, err := PreviewManual(root)
	if err != nil {
		t.Fatalf("PreviewManual: %v", err)
	}
	assertManualResult(t, preview, StatusNoop,
		"engineering", "engineering", "growth", false, 0)
	applied, err := ApplyManual(root)
	if err != nil {
		t.Fatalf("ApplyManual: %v", err)
	}
	assertManualResult(t, applied, StatusNoop,
		"engineering", "engineering", "growth", false, 0)
	assertBytesEqual(t, "project", readTestFile(t, projectPath(root)), projectBefore)
	assertBytesEqual(t, "roadmap", readTestFile(t, roadmapPath(root)), roadmapBefore)
	assertPathMissing(t, filepath.Join(root, ".forge"))
}

func TestManualAndLifecyclePromotionsUseIndependentReceipts(t *testing.T) {
	root := newRepository(t, migrate.ModeExplorer, migrate.LifecycleMVP, []byte("# Roadmap\n"))
	manualResult, err := ApplyManual(root)
	if err != nil {
		t.Fatalf("ApplyManual: %v", err)
	}
	assertManualResult(t, manualResult, StatusApplied,
		"explorer", "engineering", "mvp", true, len(expectedPromotionTaskIDs))
	manualPath, _ := assertManualReceipt(t, root, migrate.LifecycleMVP)
	manualReceiptBefore := readTestFile(t, manualPath)
	assertPathMissing(t, receiptPath(root))

	lifecycleResult, err := Apply(root, productionRequest())
	if err != nil {
		t.Fatalf("lifecycle Apply after manual migration: %v", err)
	}
	assertResult(t, lifecycleResult, StatusApplied,
		"engineering", "engineering", "mvp", false, 0)
	if lifecycleResult.RoadmapMigration {
		t.Fatalf("lifecycle-only result unexpectedly reports RoadmapMigration: %+v", lifecycleResult)
	}
	assertBytesEqual(t, "manual receipt after lifecycle promotion",
		readTestFile(t, manualPath), manualReceiptBefore)
	if _, err := os.Stat(receiptPath(root)); err != nil {
		t.Fatalf("lifecycle receipt missing: %v", err)
	}

	manualReplay, err := ApplyManual(root)
	if err != nil {
		t.Fatalf("manual replay after lifecycle promotion: %v", err)
	}
	assertManualResult(t, manualReplay, StatusReplayed,
		"explorer", "engineering", "mvp", true, len(expectedPromotionTaskIDs))
	lifecycleReplay, err := Apply(root, productionRequest())
	if err != nil {
		t.Fatalf("lifecycle replay: %v", err)
	}
	assertResult(t, lifecycleReplay, StatusReplayed,
		"engineering", "engineering", "mvp", false, 0)
}

func TestManualReceiptIsNotLifecycleReceiptAlias(t *testing.T) {
	root := newRepository(t, migrate.ModeExplorer, migrate.LifecycleIdea, []byte("# Roadmap\n"))
	if _, err := ApplyManual(root); err != nil {
		t.Fatalf("ApplyManual: %v", err)
	}
	manualPath, manualReceipt := assertManualReceipt(t, root, migrate.LifecycleIdea)
	if manualPath == receiptPath(root) {
		t.Fatal("manual and lifecycle receipt paths alias")
	}
	if manualReceipt.Operation != manualModeOperationID {
		t.Fatalf("manual receipt operation = %q, want %q",
			manualReceipt.Operation, manualModeOperationID)
	}
	if _, err := os.Stat(receiptPath(root)); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("lifecycle receipt exists after manual-only migration: %v", err)
	}
}
