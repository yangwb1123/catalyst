package migrationtxn

import (
	"bytes"
	"os"
	"testing"
)

func TestManualIntentRejectsProjectAfterUnrelatedInjectionWithFreshDigest(t *testing.T) {
	capture := captureManualTransaction(t)
	capture.resetToCrashStage(t, manualCrashAfterIntent)
	projectBefore := readTestFile(t, projectPath(capture.root))
	roadmapBefore := readTestFile(t, roadmapPath(capture.root))
	forged := capture.intent
	forgedData := append([]byte(nil), forged.ProjectAfter.Data...)
	forgedData = append(forgedData, []byte("unrelated_forged_field: true\n")...)
	forged.ProjectAfter = newFileImage(
		forgedData, os.FileMode(forged.ProjectAfter.Mode), true,
	)

	writeForgedManualPending(t, capture, forged)
	assertManualForgeryRejected(t, capture, projectBefore, roadmapBefore)
}

func TestManualIntentRejectsRoadmapHistoryDeletionWithFreshDigest(t *testing.T) {
	capture := captureManualTransaction(t)
	capture.resetToCrashStage(t, manualCrashAfterIntent)
	projectBefore := readTestFile(t, projectPath(capture.root))
	roadmapBefore := readTestFile(t, roadmapPath(capture.root))
	forged := capture.intent
	blockStart := bytes.Index(forged.RoadmapAfter.Data, []byte(promotionBlockStart))
	if blockStart <= 0 {
		t.Fatalf("captured roadmap has no removable history prefix: index=%d", blockStart)
	}
	blockOnly := append([]byte(nil), forged.RoadmapAfter.Data[blockStart:]...)
	assertPromotionMarkers(t, blockOnly)
	forged.RoadmapAfter = newFileImage(
		blockOnly, os.FileMode(forged.RoadmapAfter.Mode), true,
	)

	writeForgedManualPending(t, capture, forged)
	assertManualForgeryRejected(t, capture, projectBefore, roadmapBefore)
}
