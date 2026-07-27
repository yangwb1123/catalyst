package memory

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestAppendCreatesPrivateMemoryStore(t *testing.T) {
	path := filepath.Join(t.TempDir(), "memory.jsonl")
	if err := Append(path, Entry{Kind: KindGap, Topic: "security", Detail: "private"}); err != nil {
		t.Fatal(err)
	}
	assertPrivateStore(t, path)
}

func TestAppendTightensLegacyMemoryStorePermissions(t *testing.T) {
	path := filepath.Join(t.TempDir(), "memory.jsonl")
	if err := os.WriteFile(path, nil, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := Append(path, Entry{Kind: KindLesson, Topic: "legacy", Detail: "tighten"}); err != nil {
		t.Fatal(err)
	}
	assertPrivateStore(t, path)
}

func TestLoadRejectsOversizedStoreBeforeReading(t *testing.T) {
	path := filepath.Join(t.TempDir(), "memory.jsonl")
	file, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := file.Truncate(maxStoreBytes + 1); err != nil {
		file.Close()
		t.Fatal(err)
	}
	if err := file.Close(); err != nil {
		t.Fatal(err)
	}
	if _, err := Load(path); err == nil || !strings.Contains(err.Error(), "exceeds") {
		t.Fatalf("oversized memory store error = %v", err)
	}
}

func TestLoadRejectsStoreBehindParentSymlink(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	if err := os.WriteFile(
		filepath.Join(outside, "memory.jsonl"),
		[]byte("{\"kind\":\"gap\",\"topic\":\"injected\",\"detail\":\"outside\"}\n"),
		0o600,
	); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(root, ".forge")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	if _, err := Load(filepath.Join(root, ".forge", "memory.jsonl")); err == nil {
		t.Fatal("memory store behind parent symlink was accepted")
	}
}

func assertPrivateStore(t *testing.T, path string) {
	t.Helper()
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if got := info.Mode().Perm(); got != 0o600 {
		t.Fatalf("memory permissions = %04o, want 0600", got)
	}
}
