package migrationtxn

import (
	"bytes"
	"errors"
	"os"
	"strings"
	"testing"

	"forgeos/forge-core/internal/migrate"
)

type faultBoundaryCase struct {
	boundary       transactionBoundary
	timing         faultTiming
	pending        bool
	roadmapAfter   bool
	projectAfter   bool
	receiptPresent bool
	retryStatus    Status
}

func TestEveryTransactionBoundaryRollsForwardDeterministically(t *testing.T) {
	cases := []faultBoundaryCase{
		{boundaryIntent, faultBefore, false, false, false, false, StatusApplied},
		{boundaryIntent, faultAfter, true, false, false, false, StatusRecovered},
		{boundaryRoadmap, faultBefore, true, false, false, false, StatusRecovered},
		{boundaryRoadmap, faultAfter, true, true, false, false, StatusRecovered},
		{boundaryProject, faultBefore, true, true, false, false, StatusRecovered},
		{boundaryProject, faultAfter, true, true, true, false, StatusRecovered},
		{boundaryReceipt, faultBefore, true, true, true, false, StatusRecovered},
		{boundaryReceipt, faultAfter, true, true, true, true, StatusRecovered},
		{boundaryRemove, faultBefore, true, true, true, true, StatusRecovered},
		{boundaryRemove, faultAfter, false, true, true, true, StatusReplayed},
	}
	for _, testCase := range cases {
		name := string(testCase.boundary) + "/" + string(testCase.timing)
		t.Run(name, func(t *testing.T) {
			testFaultBoundaryRollForward(t, testCase)
		})
	}
}

func testFaultBoundaryRollForward(t *testing.T, testCase faultBoundaryCase) {
	t.Helper()
	initialRoadmap := []byte("# Fault recovery roadmap\n\n- [ ] original\n")
	root := newRepository(t, "explorer", "mvp", initialRoadmap)
	initialProject := readTestFile(t, projectPath(root))
	ensureTestForgeDir(t, root)
	ops := &faultFileOps{
		fileOps: realFileOps{}, root: root,
		boundary: testCase.boundary, timing: testCase.timing,
	}

	_, err := applyLocked(root, ops)
	if !errors.Is(err, errInjectedTransactionFault) {
		t.Fatalf("faulted apply error = %v, want injected fault", err)
	}
	if !ops.fired {
		t.Fatalf("fault boundary %s was never reached", testCase.boundary)
	}
	assertIntermediateFaultState(t, root, initialProject, initialRoadmap, testCase)

	retried, err := applyLocked(root, realFileOps{})
	if err != nil {
		t.Fatalf("roll-forward retry: %v", err)
	}
	assertResult(t, retried, testCase.retryStatus,
		"explorer", "engineering", "mvp", true, len(expectedPromotionTaskIDs))
	assertPromotedExplorer(t, root, migrate.LifecycleMVP)

	replay, err := applyLocked(root, realFileOps{})
	if err != nil {
		t.Fatalf("post-recovery replay: %v", err)
	}
	assertResult(t, replay, StatusReplayed,
		"explorer", "engineering", "mvp", true, len(expectedPromotionTaskIDs))
	assertPromotedExplorer(t, root, migrate.LifecycleMVP)
}

func assertIntermediateFaultState(
	t *testing.T,
	root string,
	initialProject, initialRoadmap []byte,
	testCase faultBoundaryCase,
) {
	t.Helper()
	assertPending(t, root, testCase.pending)
	if testCase.projectAfter {
		assertProjectSelectors(t, root, "engineering", "production")
	} else {
		assertBytesEqual(t, "intermediate project", readTestFile(t, projectPath(root)), initialProject)
	}
	if testCase.roadmapAfter {
		assertPromotionMarkers(t, readTestFile(t, roadmapPath(root)))
	} else {
		assertBytesEqual(t, "intermediate roadmap", readTestFile(t, roadmapPath(root)), initialRoadmap)
	}
	_, receiptErr := os.Lstat(receiptPath(root))
	if testCase.receiptPresent {
		if receiptErr != nil {
			t.Fatalf("terminal receipt missing after fault: %v", receiptErr)
		}
		return
	}
	if !errors.Is(receiptErr, os.ErrNotExist) {
		t.Fatalf("terminal receipt unexpectedly present: %v", receiptErr)
	}
}

func TestPendingRecoveryRejectsTrackedFileDrift(t *testing.T) {
	cases := []struct {
		name   string
		target func(string) string
		mutate func(*testing.T, string, []byte, os.FileMode)
	}{
		{
			name: "roadmap-content", target: roadmapPath,
			mutate: func(t *testing.T, path string, data []byte, mode os.FileMode) {
				writeTestFile(t, path, append(data, []byte("- [ ] external roadmap edit\n")...), mode)
			},
		},
		{
			name: "project-content", target: projectPath,
			mutate: func(t *testing.T, path string, data []byte, mode os.FileMode) {
				writeTestFile(t, path, append(data, []byte("external: drift\n")...), mode)
			},
		},
		{
			name: "project-permissions", target: projectPath,
			mutate: func(t *testing.T, path string, _ []byte, mode os.FileMode) {
				if err := os.Chmod(path, mode^0o040); err != nil {
					t.Fatalf("chmod drift: %v", err)
				}
			},
		},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			testTrackedDriftConflict(t, testCase.target, testCase.mutate)
		})
	}
}

func testTrackedDriftConflict(
	t *testing.T,
	target func(string) string,
	mutate func(*testing.T, string, []byte, os.FileMode),
) {
	t.Helper()
	root := newRepository(t, "explorer", "mvp", []byte("# Roadmap\n"))
	ensureTestForgeDir(t, root)
	path := target(root)
	before := readTestFile(t, path)
	mode := testFileMode(t, path)
	seedPendingBeforeRoadmap(t, root)
	mutate(t, path, before, mode)

	_, err := recoverPending(root, realFileOps{})
	if err == nil || !strings.Contains(err.Error(), "matches neither transaction before nor after image") {
		t.Fatalf("drift recovery error = %v, want before/after conflict", err)
	}
	assertPending(t, root, true)
	writeTestFile(t, path, before, mode)

	recovered, err := recoverPending(root, realFileOps{})
	if err != nil {
		t.Fatalf("recovery after restoring expected image: %v", err)
	}
	assertResult(t, recovered, StatusRecovered,
		"explorer", "engineering", "mvp", true, len(expectedPromotionTaskIDs))
	assertPromotedExplorer(t, root, migrate.LifecycleMVP)
}

func TestPendingRecoveryRejectsConflictingTerminalReceipt(t *testing.T) {
	root := newRepository(t, "explorer", "mvp", []byte("# Roadmap\n"))
	ensureTestForgeDir(t, root)
	seedPendingBeforeRoadmap(t, root)
	otherPromotion, err := migrate.PromoteToProduction("balanced", "mvp")
	if err != nil {
		t.Fatalf("derive conflicting receipt: %v", err)
	}
	otherData, err := encodeReceipt(receiptFromPromotion(otherPromotion))
	if err != nil {
		t.Fatalf("encode conflicting receipt: %v", err)
	}
	ops := realFileOps{}
	if err := ops.writeState(receiptPath(root), otherData); err != nil {
		t.Fatalf("write conflicting receipt: %v", err)
	}

	_, err = recoverPending(root, ops)
	if err == nil || !strings.Contains(err.Error(), "terminal receipt conflicts with pending intent") {
		t.Fatalf("conflicting receipt recovery error = %v", err)
	}
	assertPending(t, root, true)
	assertProjectSelectors(t, root, "engineering", "production")
	assertPromotionMarkers(t, readTestFile(t, roadmapPath(root)))

	if err := ops.removeState(receiptPath(root)); err != nil {
		t.Fatalf("remove conflicting receipt: %v", err)
	}
	recovered, err := recoverPending(root, ops)
	if err != nil {
		t.Fatalf("recover after conflict resolution: %v", err)
	}
	assertResult(t, recovered, StatusRecovered,
		"explorer", "engineering", "mvp", true, len(expectedPromotionTaskIDs))
	assertPromotedExplorer(t, root, migrate.LifecycleMVP)
}

func TestRecoveryDoesNotDuplicateTaskMarkersAfterPartialRoadmapCommit(t *testing.T) {
	root := newRepository(t, "explorer", "growth", []byte("# Roadmap\n"))
	ensureTestForgeDir(t, root)
	ops := &faultFileOps{
		fileOps: realFileOps{}, root: root,
		boundary: boundaryRoadmap, timing: faultAfter,
	}
	if _, err := applyLocked(root, ops); !errors.Is(err, errInjectedTransactionFault) {
		t.Fatalf("fault after roadmap commit: %v", err)
	}
	first := readTestFile(t, roadmapPath(root))
	assertPromotionMarkers(t, first)

	if _, err := recoverPending(root, realFileOps{}); err != nil {
		t.Fatalf("recover pending: %v", err)
	}
	second := readTestFile(t, roadmapPath(root))
	assertBytesEqual(t, "roadmap across recovery", second, first)
	for _, id := range expectedPromotionTaskIDs {
		if count := bytes.Count(second, []byte("forge:migration-task:"+id)); count != 1 {
			t.Fatalf("marker %s count after recovery = %d, want 1", id, count)
		}
	}
}

func TestPublicPreviewRefusesPendingAndApplyRecoversIt(t *testing.T) {
	root := newRepository(t, "explorer", "mvp", []byte("# Roadmap\n"))
	ensureTestForgeDir(t, root)
	projectBefore := readTestFile(t, projectPath(root))
	roadmapBefore := readTestFile(t, roadmapPath(root))
	seedPendingBeforeRoadmap(t, root)

	_, err := Preview(root, productionRequest())
	if err == nil || !strings.Contains(err.Error(), "pending migration requires") {
		t.Fatalf("Preview pending error = %v", err)
	}
	assertBytesEqual(t, "project after refused preview",
		readTestFile(t, projectPath(root)), projectBefore)
	assertBytesEqual(t, "roadmap after refused preview",
		readTestFile(t, roadmapPath(root)), roadmapBefore)
	assertPending(t, root, true)

	recovered, err := Apply(root, productionRequest())
	if err != nil {
		t.Fatalf("public Apply recovery: %v", err)
	}
	assertResult(t, recovered, StatusRecovered,
		"explorer", "engineering", "mvp", true, len(expectedPromotionTaskIDs))
	assertPromotedExplorer(t, root, migrate.LifecycleMVP)
}
