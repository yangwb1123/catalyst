package migrationtxn

import (
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestPendingAndPreviewProbeOversizedIntentWithoutReadingOrChmod(t *testing.T) {
	root := newRepository(t, "explorer", "mvp", []byte("# Roadmap\n"))
	ensureTestForgeDir(t, root)
	if err := ensureMigrationStateDir(root, realFileOps{}); err != nil {
		t.Fatalf("ensure migration state: %v", err)
	}
	path := pendingPath(root)
	file, err := os.OpenFile(path, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o640)
	if err != nil {
		t.Fatalf("create oversized pending intent: %v", err)
	}
	if err := file.Truncate(stateMaxBytes + 1); err != nil {
		file.Close()
		t.Fatalf("truncate oversized pending intent: %v", err)
	}
	if err := file.Close(); err != nil {
		t.Fatalf("close oversized pending intent: %v", err)
	}
	if err := os.Chmod(path, 0o640); err != nil {
		t.Fatalf("chmod oversized pending intent: %v", err)
	}

	pending, err := Pending(root)
	if err != nil || !pending {
		t.Fatalf("Pending oversized intent = %v, %v; want true, nil", pending, err)
	}
	if _, err := PreviewManual(root); err == nil {
		t.Fatal("PreviewManual did not refuse pending intent")
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatalf("stat pending intent after probes: %v", err)
	}
	if info.Mode().Perm() != 0o640 || info.Size() != stateMaxBytes+1 {
		t.Fatalf("pending intent mutated by presence probes: mode=%#o size=%d",
			info.Mode().Perm(), info.Size())
	}
}

func TestManualPreviewRejectsProjectRewriteThatExceedsImageLimit(t *testing.T) {
	project := paddedTestData(
		[]byte("mode: explorer\nlifecycle: mvp\n# padding\n"),
		int(projectMaxBytes),
	)
	root := newRawRepository(t, project, []byte("# Roadmap\n"))
	assertPreviewAndApplyRejectOversizedPlan(t, root, projectPath(root), project)
}

func TestManualPreviewRejectsRoadmapAppendThatExceedsImageLimit(t *testing.T) {
	roadmap := paddedTestData([]byte("# Roadmap\n"), int(roadmapMaxBytes))
	root := newRepository(t, "explorer", "mvp", roadmap)
	assertPreviewAndApplyRejectOversizedPlan(t, root, roadmapPath(root), roadmap)
}

func paddedTestData(prefix []byte, size int) []byte {
	data := make([]byte, size)
	copy(data, prefix)
	for index := len(prefix); index < len(data); index++ {
		data[index] = 'x'
	}
	return data
}

func assertPreviewAndApplyRejectOversizedPlan(
	t *testing.T,
	root, trackedPath string,
	before []byte,
) {
	t.Helper()
	if _, err := PreviewManual(root); err == nil {
		t.Fatal("PreviewManual planned an oversized transaction image")
	}
	if _, err := ApplyManual(root); err == nil {
		t.Fatal("ApplyManual accepted an oversized transaction image")
	}
	assertBytesEqual(t, "tracked file after oversized plan rejection",
		readTestFile(t, trackedPath), before)
	if _, err := os.Lstat(filepath.Join(root, ".forge")); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("oversized dry/preflight plan created .forge: %v", err)
	}
}
