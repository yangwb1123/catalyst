package migrationtxn

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/migrate"
)

var expectedPromotionTaskIDs = []string{
	"backfill-tests",
	"add-ci",
	"add-monitoring",
	"refactor-oversized",
	"security-pass",
}

func productionRequest() Request {
	return Request{ToLifecycle: migrate.LifecycleProduction}
}

func newRepository(t *testing.T, mode, lifecycle string, roadmap []byte) string {
	t.Helper()
	project := []byte(fmt.Sprintf(
		"name: migration-fixture\nmode: %s\nlifecycle: %s\nowner: test\n",
		mode, lifecycle,
	))
	return newRawRepository(t, project, roadmap)
}

func newRawRepository(t *testing.T, project, roadmap []byte) string {
	t.Helper()
	root := t.TempDir()
	agentDir := filepath.Join(root, ".agent")
	if err := os.Mkdir(agentDir, 0o755); err != nil {
		t.Fatalf("mkdir .agent: %v", err)
	}
	writeTestFile(t, projectPath(root), project, 0o644)
	if roadmap != nil {
		writeTestFile(t, roadmapPath(root), roadmap, 0o644)
	}
	return root
}

func ensureTestForgeDir(t *testing.T, root string) {
	t.Helper()
	if err := os.Mkdir(filepath.Join(root, ".forge"), 0o700); err != nil {
		t.Fatalf("mkdir .forge: %v", err)
	}
}

func writeTestFile(t *testing.T, path string, data []byte, mode os.FileMode) {
	t.Helper()
	if err := os.WriteFile(path, data, mode); err != nil {
		t.Fatalf("write %s: %v", path, err)
	}
	if err := os.Chmod(path, mode); err != nil {
		t.Fatalf("chmod %s: %v", path, err)
	}
}

func readTestFile(t *testing.T, path string) []byte {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	return data
}

func testFileMode(t *testing.T, path string) os.FileMode {
	t.Helper()
	info, err := os.Stat(path)
	if err != nil {
		t.Fatalf("stat %s: %v", path, err)
	}
	return info.Mode().Perm()
}

func assertPathMissing(t *testing.T, path string) {
	t.Helper()
	if _, err := os.Lstat(path); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("%s exists or has unexpected stat error: %v", path, err)
	}
}

func assertBytesEqual(t *testing.T, label string, got, want []byte) {
	t.Helper()
	if !bytes.Equal(got, want) {
		t.Fatalf("%s mismatch\n--- got ---\n%s\n--- want ---\n%s", label, got, want)
	}
}

func assertResult(
	t *testing.T,
	result Result,
	status Status,
	fromMode, toMode, fromLifecycle string,
	autoMigration bool,
	taskCount int,
) {
	t.Helper()
	if result.Status != status ||
		result.FromMode != fromMode ||
		result.ToMode != toMode ||
		result.FromLifecycle != fromLifecycle ||
		result.ToLifecycle != migrate.LifecycleProduction ||
		result.AutoMigration != autoMigration ||
		len(result.Tasks) != taskCount {
		t.Fatalf("result = %+v, want status=%s %s/%s -> %s/production auto=%v tasks=%d",
			result, status, fromMode, fromLifecycle, toMode, autoMigration, taskCount)
	}
}

func assertProjectSelectors(t *testing.T, root, wantMode, wantLifecycle string) {
	t.Helper()
	selectors, err := strictProjectSelectors(readTestFile(t, projectPath(root)))
	if err != nil {
		t.Fatalf("parse promoted project selectors: %v", err)
	}
	if selectors.mode != wantMode || selectors.lifecycle != wantLifecycle {
		t.Fatalf("project selectors = %s/%s, want %s/%s",
			selectors.mode, selectors.lifecycle, wantMode, wantLifecycle)
	}
}

func assertPromotionMarkers(t *testing.T, data []byte) {
	t.Helper()
	tasks := migrate.ExplorerToEngineering().Tasks
	present, err := validatePromotionRoadmap(data, tasks)
	if err != nil || !present {
		t.Fatalf("promotion markers: present=%v err=%v", present, err)
	}
	for index, id := range expectedPromotionTaskIDs {
		if index >= len(tasks) || tasks[index].ID != id {
			t.Fatalf("task IDs changed at index %d: tasks=%+v", index, tasks)
		}
		marker := []byte("forge:migration-task:" + id)
		if count := bytes.Count(data, marker); count != 1 {
			t.Fatalf("marker %q count = %d, want 1", id, count)
		}
	}
	if len(tasks) != len(expectedPromotionTaskIDs) {
		t.Fatalf("promotion task count = %d, want %d", len(tasks), len(expectedPromotionTaskIDs))
	}
}

func assertTerminalReceipt(t *testing.T, root string, want promotionReceipt) {
	t.Helper()
	data := readTestFile(t, receiptPath(root))
	got, err := decodeReceipt(data)
	if err != nil {
		t.Fatalf("decode terminal receipt: %v", err)
	}
	if !sameReceipt(got, want) {
		t.Fatalf("terminal receipt = %+v, want %+v", got, want)
	}
	if mode := testFileMode(t, receiptPath(root)); mode != 0o600 {
		t.Fatalf("terminal receipt mode = %#o, want 0600", mode)
	}
}

func assertPromotedExplorer(t *testing.T, root, fromLifecycle string) {
	t.Helper()
	assertProjectSelectors(t, root, migrate.ModeEngineering, migrate.LifecycleProduction)
	assertPromotionMarkers(t, readTestFile(t, roadmapPath(root)))
	promotion, err := migrate.PromoteToProduction(migrate.ModeExplorer, fromLifecycle)
	if err != nil {
		t.Fatalf("derive expected receipt: %v", err)
	}
	assertTerminalReceipt(t, root, receiptFromPromotion(promotion))
	assertPathMissing(t, pendingPath(root))
}

func assertPending(t *testing.T, root string, want bool) {
	t.Helper()
	got, err := Pending(root)
	if err != nil {
		t.Fatalf("Pending: %v", err)
	}
	if got != want {
		t.Fatalf("Pending = %v, want %v", got, want)
	}
}

type transactionBoundary string

const (
	boundaryIntent  transactionBoundary = "intent"
	boundaryRoadmap transactionBoundary = "roadmap"
	boundaryProject transactionBoundary = "project"
	boundaryReceipt transactionBoundary = "receipt"
	boundaryRemove  transactionBoundary = "remove"
)

type faultTiming string

const (
	faultBefore faultTiming = "before"
	faultAfter  faultTiming = "after"
)

var errInjectedTransactionFault = errors.New("injected migration transaction fault")

type faultFileOps struct {
	fileOps
	root     string
	boundary transactionBoundary
	timing   faultTiming
	fired    bool
}

func (ops *faultFileOps) writeTracked(path string, expected, image fileImage) error {
	boundary := boundaryProject
	if path == roadmapPath(ops.root) {
		boundary = boundaryRoadmap
	}
	return ops.run(boundary, func() error {
		return ops.fileOps.writeTracked(path, expected, image)
	})
}

func (ops *faultFileOps) writeState(path string, data []byte) error {
	boundary := boundaryReceipt
	if path == pendingPath(ops.root) {
		boundary = boundaryIntent
	}
	return ops.run(boundary, func() error {
		return ops.fileOps.writeState(path, data)
	})
}

func (ops *faultFileOps) removeState(path string) error {
	return ops.run(boundaryRemove, func() error {
		return ops.fileOps.removeState(path)
	})
}

func (ops *faultFileOps) run(boundary transactionBoundary, action func() error) error {
	if ops.fired || boundary != ops.boundary {
		return action()
	}
	ops.fired = true
	if ops.timing == faultBefore {
		return fmt.Errorf("%w at %s/%s", errInjectedTransactionFault, boundary, ops.timing)
	}
	if err := action(); err != nil {
		return err
	}
	return fmt.Errorf("%w at %s/%s", errInjectedTransactionFault, boundary, ops.timing)
}

func seedPendingBeforeRoadmap(t *testing.T, root string) {
	t.Helper()
	ops := &faultFileOps{
		fileOps: realFileOps{}, root: root,
		boundary: boundaryRoadmap, timing: faultBefore,
	}
	if _, err := applyLocked(root, ops); !errors.Is(err, errInjectedTransactionFault) {
		t.Fatalf("seed pending transaction: %v", err)
	}
	if !ops.fired {
		t.Fatal("roadmap fault was not reached")
	}
	assertPending(t, root, true)
}

func assertNoBareLF(t *testing.T, label string, data []byte) {
	t.Helper()
	if strings.Contains(string(bytes.ReplaceAll(data, []byte("\r\n"), nil)), "\n") {
		t.Fatalf("%s contains a bare LF", label)
	}
}
