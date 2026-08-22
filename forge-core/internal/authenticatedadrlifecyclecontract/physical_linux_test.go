package authenticatedadrlifecyclecontract

import (
	"os"
	"path/filepath"
	"strings"
	"syscall"
	"testing"
)

func TestOwnedGoFilesAreRegularSingleLink0644AndCacheFree(t *testing.T) {
	entries, err := os.ReadDir(".")
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range entries {
		if entry.IsDir() || strings.HasSuffix(entry.Name(), ".pyc") ||
			entry.Name() == "__pycache__" || entry.Name() == ".cache" {
			t.Fatalf("cache or unexpected directory in owned package: %s", entry.Name())
		}
		if filepath.Ext(entry.Name()) != ".go" {
			continue
		}
		assertPhysicalGoFile(t, entry.Name())
	}
}

func assertPhysicalGoFile(t *testing.T, path string) {
	t.Helper()
	info, err := os.Lstat(path)
	if err != nil {
		t.Fatal(err)
	}
	if !info.Mode().IsRegular() || info.Mode().Perm() != 0o644 {
		t.Fatalf("%s must be a regular 0644 file; mode=%v", path, info.Mode())
	}
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !ok || stat.Nlink != 1 {
		t.Fatalf("%s must have exactly one physical link", path)
	}
}
