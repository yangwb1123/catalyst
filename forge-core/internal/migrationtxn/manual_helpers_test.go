package migrationtxn

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/migrate"
)

func assertManualResult(
	t *testing.T,
	result Result,
	status Status,
	fromMode, toMode, lifecycle string,
	roadmapMigration bool,
	taskCount int,
) {
	t.Helper()
	if result.Status != status ||
		result.FromMode != fromMode ||
		result.ToMode != toMode ||
		result.FromLifecycle != lifecycle ||
		result.ToLifecycle != lifecycle ||
		result.AutoMigration ||
		result.RoadmapMigration != roadmapMigration ||
		len(result.Tasks) != taskCount {
		t.Fatalf(
			"manual result = %+v, want status=%s %s/%s -> %s/%s auto=false roadmap=%v tasks=%d",
			result, status, fromMode, lifecycle, toMode, lifecycle,
			roadmapMigration, taskCount,
		)
	}
}

func manualReceiptPathForTest(t *testing.T, root string) string {
	t.Helper()
	entries, err := os.ReadDir(migrationStateDir(root))
	if err != nil {
		t.Fatalf("read migration state directory: %v", err)
	}
	var candidates []string
	for _, entry := range entries {
		path := filepath.Join(migrationStateDir(root), entry.Name())
		if entry.IsDir() || path == pendingPath(root) || path == receiptPath(root) {
			continue
		}
		if strings.HasSuffix(entry.Name(), ".json") {
			candidates = append(candidates, path)
		}
	}
	if len(candidates) != 1 {
		t.Fatalf("manual receipt candidates = %v, want exactly one", candidates)
	}
	return candidates[0]
}

func assertManualReceipt(t *testing.T, root, lifecycle string) (string, promotionReceipt) {
	t.Helper()
	path := manualReceiptPathForTest(t, root)
	data := readTestFile(t, path)
	receipt, err := decodeReceipt(data)
	if err != nil {
		t.Fatalf("decode manual receipt: %v", err)
	}
	if receipt.Operation != manualModeOperationID ||
		receipt.FromMode != migrate.ModeExplorer ||
		receipt.ToMode != migrate.ModeEngineering ||
		receipt.FromLifecycle != lifecycle ||
		receipt.ToLifecycle != lifecycle ||
		receipt.AutoMigration ||
		!receipt.RoadmapMigration ||
		!equalStrings(receipt.TaskIDs, expectedPromotionTaskIDs) {
		t.Fatalf("manual receipt = %+v, want operation=%q", receipt, manualModeOperationID)
	}
	if mode := testFileMode(t, path); mode != 0o600 {
		t.Fatalf("manual receipt mode = %#o, want 0600", mode)
	}
	return path, receipt
}

func assertManuallyMigrated(t *testing.T, root, lifecycle string) {
	t.Helper()
	assertProjectSelectors(t, root, migrate.ModeEngineering, lifecycle)
	assertPromotionMarkers(t, readTestFile(t, roadmapPath(root)))
	assertManualReceipt(t, root, lifecycle)
	assertPathMissing(t, pendingPath(root))
}

type capturedManualTransaction struct {
	root          string
	projectBefore fileImage
	projectAfter  fileImage
	roadmapBefore fileImage
	roadmapAfter  fileImage
	receipt       promotionReceipt
	receiptPath   string
	intent        promotionIntent
}

func captureManualTransaction(t *testing.T) capturedManualTransaction {
	t.Helper()
	root := newRepository(t, migrate.ModeExplorer, migrate.LifecycleMVP,
		[]byte("# Manual migration roadmap\n\n- [ ] preserve this\n"))
	ops := realFileOps{}
	projectBefore := readTrackedForTest(t, ops, projectPath(root), projectMaxBytes)
	roadmapBefore := readTrackedForTest(t, ops, roadmapPath(root), roadmapMaxBytes)
	result, err := ApplyManual(root)
	if err != nil {
		t.Fatalf("capture ApplyManual: %v", err)
	}
	assertManualResult(t, result, StatusApplied,
		"explorer", "engineering", "mvp", true, len(expectedPromotionTaskIDs))
	projectAfter := readTrackedForTest(t, ops, projectPath(root), projectMaxBytes)
	roadmapAfter := readTrackedForTest(t, ops, roadmapPath(root), roadmapMaxBytes)
	manualPath, receipt := assertManualReceipt(t, root, migrate.LifecycleMVP)
	intent := promotionIntent{
		Format: intentFormat, Operation: receipt.Operation, Receipt: receipt,
		ProjectBefore: projectBefore, ProjectAfter: projectAfter,
		RoadmapManaged: true, RoadmapBefore: roadmapBefore, RoadmapAfter: roadmapAfter,
	}
	if err := validateIntent(intent); err != nil {
		t.Fatalf("captured manual intent is invalid: %v", err)
	}
	return capturedManualTransaction{
		root: root, projectBefore: projectBefore, projectAfter: projectAfter,
		roadmapBefore: roadmapBefore, roadmapAfter: roadmapAfter,
		receipt: receipt, receiptPath: manualPath, intent: intent,
	}
}

func readTrackedForTest(
	t *testing.T,
	ops realFileOps,
	path string,
	maxBytes int64,
) fileImage {
	t.Helper()
	image, err := ops.readTracked(path, maxBytes)
	if err != nil {
		t.Fatalf("read tracked image %s: %v", path, err)
	}
	return image
}

type manualCrashStage string

const (
	manualCrashAfterIntent  manualCrashStage = "intent"
	manualCrashAfterRoadmap manualCrashStage = "roadmap"
	manualCrashAfterProject manualCrashStage = "project"
	manualCrashAfterReceipt manualCrashStage = "receipt"
)

func (capture capturedManualTransaction) resetToCrashStage(
	t *testing.T,
	stage manualCrashStage,
) {
	t.Helper()
	ops := realFileOps{}
	project := capture.projectBefore
	roadmap := capture.roadmapBefore
	keepReceipt := false
	switch stage {
	case manualCrashAfterRoadmap:
		roadmap = capture.roadmapAfter
	case manualCrashAfterProject:
		project, roadmap = capture.projectAfter, capture.roadmapAfter
	case manualCrashAfterReceipt:
		project, roadmap = capture.projectAfter, capture.roadmapAfter
		keepReceipt = true
	}
	writeTrackedForTest(t, ops, projectPath(capture.root), project)
	writeTrackedForTest(t, ops, roadmapPath(capture.root), roadmap)
	if !keepReceipt {
		if err := ops.removeState(capture.receiptPath); err != nil {
			t.Fatalf("remove manual receipt: %v", err)
		}
	}
	data, err := encodeIntent(capture.intent)
	if err != nil {
		t.Fatalf("encode captured manual intent: %v", err)
	}
	if err := ops.writeState(pendingPath(capture.root), data); err != nil {
		t.Fatalf("write manual pending intent: %v", err)
	}
}

func writeTrackedForTest(t *testing.T, ops realFileOps, path string, image fileImage) {
	t.Helper()
	current, err := ops.readTracked(path, maxTestTrackedBytes(image))
	if err != nil {
		t.Fatalf("read current tracked image %s: %v", path, err)
	}
	if err := ops.writeTracked(path, current, image); err != nil {
		t.Fatalf("write tracked image %s: %v", path, err)
	}
}

func maxTestTrackedBytes(image fileImage) int64 {
	if len(image.Data) > roadmapMaxBytes {
		return int64(len(image.Data)) + 1
	}
	return roadmapMaxBytes
}

func writeForgedManualPending(
	t *testing.T,
	capture capturedManualTransaction,
	forged promotionIntent,
) {
	t.Helper()
	data, err := marshalCanonical(forged)
	if err != nil {
		t.Fatalf("marshal forged canonical intent: %v", err)
	}
	var roundTrip promotionIntent
	if err := unmarshalCanonical(data, &roundTrip); err != nil {
		t.Fatalf("forged intent is not valid canonical JSON: %v", err)
	}
	reencoded, err := marshalCanonical(roundTrip)
	if err != nil || !bytes.Equal(reencoded, data) {
		t.Fatalf("forged intent did not round-trip canonically: %v", err)
	}
	if _, err := encodeIntent(forged); err == nil {
		t.Fatal("encodeIntent accepted semantically forged intent")
	}
	if err := (realFileOps{}).writeState(pendingPath(capture.root), data); err != nil {
		t.Fatalf("publish forged pending intent: %v", err)
	}
}

func assertManualForgeryRejected(
	t *testing.T,
	capture capturedManualTransaction,
	projectBefore, roadmapBefore []byte,
) {
	t.Helper()
	if _, err := ApplyManual(capture.root); err == nil {
		t.Fatal("ApplyManual accepted forged pending intent")
	}
	assertBytesEqual(t, "project after rejected forgery",
		readTestFile(t, projectPath(capture.root)), projectBefore)
	assertBytesEqual(t, "roadmap after rejected forgery",
		readTestFile(t, roadmapPath(capture.root)), roadmapBefore)
	assertPending(t, capture.root, true)
	if _, err := os.Lstat(capture.receiptPath); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("manual receipt exists after rejected forgery: %v", err)
	}
}
