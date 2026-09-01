package domain

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestSourceScanBudgetAcceptsExactLimits(t *testing.T) {
	root := t.TempDir()
	source := []byte("package fixture\n")
	if err := os.WriteFile(filepath.Join(root, "fixture.go"), source, 0o600); err != nil {
		t.Fatal(err)
	}
	budget := constrainedSourceScanBudget(1, 1, 1, int64(len(source)))
	consumers, err := scanModuleConsumerSourcesWithBudget(root, budget)
	if err != nil || len(consumers) != 0 {
		t.Fatalf("exact scan limits = %v, %v", consumers, err)
	}
}

func TestSourceScanBudgetRejectsOneOverEveryLimit(t *testing.T) {
	t.Run("entries", func(t *testing.T) {
		root := t.TempDir()
		mustMakeSourceDirectories(t, root, "a", "b")
		assertSourceScanBound(t, root, constrainedSourceScanBudget(1, 4, 2, 64), "entry count")
	})
	t.Run("files", func(t *testing.T) {
		root := t.TempDir()
		mustWriteSourceFiles(t, root, map[string]string{"a.txt": "a", "b.txt": "b"})
		assertSourceScanBound(t, root, constrainedSourceScanBudget(2, 1, 1, 64), "file count")
	})
	t.Run("depth", func(t *testing.T) {
		root := t.TempDir()
		if err := os.MkdirAll(filepath.Join(root, "a", "b"), 0o700); err != nil {
			t.Fatal(err)
		}
		assertSourceScanBound(t, root, constrainedSourceScanBudget(2, 2, 1, 64), "depth")
	})
	t.Run("aggregate bytes", func(t *testing.T) {
		root := t.TempDir()
		source := "package fixture\n"
		mustWriteSourceFiles(t, root, map[string]string{"fixture.go": source})
		budget := constrainedSourceScanBudget(1, 1, 1, int64(len(source)-1))
		assertSourceScanBound(t, root, budget, "aggregate bytes")
	})
}

func constrainedSourceScanBudget(
	entries, files, depth int, bytes int64,
) *sourceScanBudget {
	return &sourceScanBudget{
		maxEntries: entries, maxFiles: files, maxDepth: depth, maxBytes: bytes,
	}
}

func assertSourceScanBound(
	t *testing.T, root string, budget *sourceScanBudget, diagnostic string,
) {
	t.Helper()
	_, err := scanModuleConsumerSourcesWithBudget(root, budget)
	if err == nil || !strings.Contains(err.Error(), diagnostic) {
		t.Fatalf("scan bound %q = %v", diagnostic, err)
	}
}

func mustMakeSourceDirectories(t *testing.T, root string, names ...string) {
	t.Helper()
	for _, name := range names {
		if err := os.Mkdir(filepath.Join(root, name), 0o700); err != nil {
			t.Fatal(err)
		}
	}
}

func mustWriteSourceFiles(t *testing.T, root string, values map[string]string) {
	t.Helper()
	for name, value := range values {
		if err := os.WriteFile(filepath.Join(root, name), []byte(value), 0o600); err != nil {
			t.Fatal(err)
		}
	}
}
