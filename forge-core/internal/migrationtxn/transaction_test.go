package migrationtxn

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"forgeos/forge-core/internal/migrate"
)

func TestExplorerPromotionDryApplyReplay(t *testing.T) {
	initialRoadmap := []byte("# Roadmap\n\n- [ ] existing item\n")
	root := newRepository(t, migrate.ModeExplorer, migrate.LifecycleMVP, initialRoadmap)
	initialProject := readTestFile(t, projectPath(root))

	preview, err := Preview(root, productionRequest())
	if err != nil {
		t.Fatalf("Preview: %v", err)
	}
	assertResult(t, preview, StatusPlanned, "explorer", "engineering", "mvp", true, 5)
	assertBytesEqual(t, "project after preview", readTestFile(t, projectPath(root)), initialProject)
	assertBytesEqual(t, "roadmap after preview", readTestFile(t, roadmapPath(root)), initialRoadmap)
	assertPathMissing(t, filepath.Join(root, ".forge"))

	applied, err := Apply(root, productionRequest())
	if err != nil {
		t.Fatalf("Apply: %v", err)
	}
	assertResult(t, applied, StatusApplied, "explorer", "engineering", "mvp", true, 5)
	assertPromotedExplorer(t, root, migrate.LifecycleMVP)

	projectAfter := readTestFile(t, projectPath(root))
	roadmapAfter := readTestFile(t, roadmapPath(root))
	receiptAfter := readTestFile(t, receiptPath(root))
	replayed, err := Apply(root, productionRequest())
	if err != nil {
		t.Fatalf("replay Apply: %v", err)
	}
	assertResult(t, replayed, StatusReplayed, "explorer", "engineering", "mvp", true, 5)
	assertBytesEqual(t, "project after replay", readTestFile(t, projectPath(root)), projectAfter)
	assertBytesEqual(t, "roadmap after replay", readTestFile(t, roadmapPath(root)), roadmapAfter)
	assertBytesEqual(t, "receipt after replay", readTestFile(t, receiptPath(root)), receiptAfter)
	assertPromotedExplorer(t, root, migrate.LifecycleMVP)
}

func TestNonExplorerPromotionIsLifecycleOnlyMatrix(t *testing.T) {
	modes := []string{migrate.ModeBalanced, migrate.ModeEngineering, migrate.ModeCTO}
	lifecycles := []string{
		migrate.LifecycleIdea,
		migrate.LifecycleMVP,
		migrate.LifecycleGrowth,
	}
	for _, mode := range modes {
		for _, lifecycle := range lifecycles {
			t.Run(mode+"/"+lifecycle, func(t *testing.T) {
				testLifecycleOnlyPromotion(t, mode, lifecycle)
			})
		}
	}
}

func testLifecycleOnlyPromotion(t *testing.T, mode, lifecycle string) {
	t.Helper()
	initialRoadmap := []byte("# Roadmap\n\n- [ ] must remain byte-identical\n")
	root := newRepository(t, mode, lifecycle, initialRoadmap)
	if err := os.Chmod(projectPath(root), 0o640); err != nil {
		t.Fatalf("chmod project: %v", err)
	}

	preview, err := Preview(root, productionRequest())
	if err != nil {
		t.Fatalf("Preview: %v", err)
	}
	assertResult(t, preview, StatusPlanned, mode, mode, lifecycle, false, 0)
	assertPathMissing(t, filepath.Join(root, ".forge"))

	applied, err := Apply(root, productionRequest())
	if err != nil {
		t.Fatalf("Apply: %v", err)
	}
	assertResult(t, applied, StatusApplied, mode, mode, lifecycle, false, 0)
	assertProjectSelectors(t, root, mode, migrate.LifecycleProduction)
	assertBytesEqual(t, "lifecycle-only roadmap", readTestFile(t, roadmapPath(root)), initialRoadmap)
	if got := testFileMode(t, projectPath(root)); got != 0o640 {
		t.Fatalf("project mode = %#o, want 0640", got)
	}
	promotion, err := migrate.PromoteToProduction(mode, lifecycle)
	if err != nil {
		t.Fatalf("derive receipt: %v", err)
	}
	assertTerminalReceipt(t, root, receiptFromPromotion(promotion))

	replayed, err := Apply(root, productionRequest())
	if err != nil {
		t.Fatalf("replay Apply: %v", err)
	}
	assertResult(t, replayed, StatusReplayed, mode, mode, lifecycle, false, 0)
	assertBytesEqual(t, "roadmap after replay", readTestFile(t, roadmapPath(root)), initialRoadmap)
	assertPending(t, root, false)
}

func TestAlreadyProductionIsExactNoopForEveryMode(t *testing.T) {
	modes := []string{
		migrate.ModeExplorer,
		migrate.ModeBalanced,
		migrate.ModeEngineering,
		migrate.ModeCTO,
	}
	for _, mode := range modes {
		t.Run(mode, func(t *testing.T) {
			root := newRepository(t, mode, migrate.LifecycleProduction, []byte("# untouched\n"))
			projectBefore := readTestFile(t, projectPath(root))
			roadmapBefore := readTestFile(t, roadmapPath(root))

			preview, err := Preview(root, productionRequest())
			if err != nil {
				t.Fatalf("Preview: %v", err)
			}
			assertResult(t, preview, StatusNoop, mode, mode, "production", false, 0)
			applied, err := Apply(root, productionRequest())
			if err != nil {
				t.Fatalf("Apply: %v", err)
			}
			assertResult(t, applied, StatusNoop, mode, mode, "production", false, 0)
			assertBytesEqual(t, "project", readTestFile(t, projectPath(root)), projectBefore)
			assertBytesEqual(t, "roadmap", readTestFile(t, roadmapPath(root)), roadmapBefore)
			assertPathMissing(t, filepath.Join(root, ".forge"))
		})
	}
}

func TestExplorerPromotionPreservesCRLFQuotesCommentsAndPermissions(t *testing.T) {
	project := []byte(
		"# selectors retain their spelling\r\n" +
			"lifecycle:\t'mvp'  # release stage\r\n" +
			"nested:\r\n" +
			"  mode: balanced\r\n" +
			"mode:  \"explorer\"\t# governance posture\r\n" +
			"owner: '#not-a-comment'\r\n",
	)
	wantProject := []byte(
		"# selectors retain their spelling\r\n" +
			"lifecycle:\t'production'  # release stage\r\n" +
			"nested:\r\n" +
			"  mode: balanced\r\n" +
			"mode:  \"engineering\"\t# governance posture\r\n" +
			"owner: '#not-a-comment'\r\n",
	)
	roadmap := []byte("# Existing roadmap\r\n\r\n- [ ] preserve me\r\n")
	root := newRawRepository(t, project, roadmap)
	writeTestFile(t, projectPath(root), project, 0o640)
	writeTestFile(t, roadmapPath(root), roadmap, 0o604)

	result, err := Apply(root, productionRequest())
	if err != nil {
		t.Fatalf("Apply: %v", err)
	}
	assertResult(t, result, StatusApplied, "explorer", "engineering", "mvp", true, 5)
	gotProject := readTestFile(t, projectPath(root))
	gotRoadmap := readTestFile(t, roadmapPath(root))
	assertBytesEqual(t, "quoted CRLF project", gotProject, wantProject)
	if !bytes.HasPrefix(gotRoadmap, append(append([]byte(nil), roadmap...), []byte("\r\n")...)) {
		t.Fatalf("roadmap prefix/comments changed:\n%s", gotRoadmap)
	}
	assertNoBareLF(t, "project", gotProject)
	assertNoBareLF(t, "roadmap", gotRoadmap)
	assertPromotionMarkers(t, gotRoadmap)
	if got := testFileMode(t, projectPath(root)); got != 0o640 {
		t.Fatalf("project mode = %#o, want 0640", got)
	}
	if got := testFileMode(t, roadmapPath(root)); got != 0o604 {
		t.Fatalf("roadmap mode = %#o, want 0604", got)
	}
}

func TestExplorerPromotionCreatesMissingRoadmapWithStableMarkers(t *testing.T) {
	root := newRepository(t, migrate.ModeExplorer, migrate.LifecycleGrowth, nil)
	if _, err := Apply(root, productionRequest()); err != nil {
		t.Fatalf("Apply: %v", err)
	}
	data := readTestFile(t, roadmapPath(root))
	if !bytes.HasPrefix(data, []byte(promotionBlockStart+"\n")) {
		t.Fatalf("new roadmap does not start with canonical block:\n%s", data)
	}
	if got := testFileMode(t, roadmapPath(root)); got != 0o644 {
		t.Fatalf("new ROADMAP.md mode = %#o, want 0644", got)
	}
	assertPromotionMarkers(t, data)

	beforeReplay := append([]byte(nil), data...)
	result, err := Apply(root, productionRequest())
	if err != nil {
		t.Fatalf("replay Apply: %v", err)
	}
	assertResult(t, result, StatusReplayed, "explorer", "engineering", "growth", true, 5)
	assertBytesEqual(t, "new roadmap after replay", readTestFile(t, roadmapPath(root)), beforeReplay)
	assertPromotionMarkers(t, beforeReplay)
}

func TestUnsupportedLifecycleTargetFailsWithoutMutation(t *testing.T) {
	root := newRepository(t, migrate.ModeExplorer, migrate.LifecycleMVP, []byte("# Roadmap\n"))
	projectBefore := readTestFile(t, projectPath(root))
	for _, apply := range []bool{false, true} {
		name := fmt.Sprintf("apply=%v", apply)
		t.Run(name, func(t *testing.T) {
			request := Request{ToLifecycle: migrate.LifecycleGrowth}
			var err error
			if apply {
				_, err = Apply(root, request)
			} else {
				_, err = Preview(root, request)
			}
			if err == nil {
				t.Fatal("unsupported target succeeded")
			}
		})
	}
	assertBytesEqual(t, "project", readTestFile(t, projectPath(root)), projectBefore)
	assertPathMissing(t, filepath.Join(root, ".forge"))
}
