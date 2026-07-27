package statefs

import (
	"os"
	"path/filepath"
	"testing"
)

func TestEnsurePrivateDirRejectsSymlink(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	path := filepath.Join(root, ".forge")
	if err := os.Symlink(outside, path); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	if err := EnsurePrivateDir(path); err == nil {
		t.Fatal("state directory symlink was accepted")
	}
}

func TestEnsurePrivateDirTreePreservesIntentionalAnchorAlias(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	link := filepath.Join(root, "link")
	if err := os.Symlink(outside, link); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	path := filepath.Join(link, "deeper")
	if err := EnsurePrivateDirTree(path); err != nil {
		t.Fatalf("intentional existing anchor alias: %v", err)
	}
	if info, err := os.Stat(filepath.Join(outside, "deeper")); err != nil || !info.IsDir() {
		t.Fatalf("anchored descendant missing: info=%v err=%v", info, err)
	}
}

func TestOpenRegularRejectsSymlinkWithoutClobberingTarget(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(t.TempDir(), "sentinel")
	if err := os.WriteFile(target, []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "run.lock")
	if err := os.Symlink(target, path); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	if _, err := OpenRegular(path, os.O_RDWR|os.O_CREATE, 0o600); err == nil {
		t.Fatal("state leaf symlink was accepted")
	}
	data, err := os.ReadFile(target)
	if err != nil || string(data) != "keep" {
		t.Fatalf("outside sentinel changed: data=%q err=%v", data, err)
	}
}

func TestAtomicWriteRejectsTargetSymlink(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, ".forge")
	if err := EnsurePrivateDir(dir); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(dir, "chain-state.json")
	outside := filepath.Join(t.TempDir(), "sentinel")
	if err := os.WriteFile(outside, []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, target); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	if err := AtomicWrite(target, []byte("replace"), 0o600); err == nil {
		t.Fatal("atomic state target symlink was accepted")
	}
	data, err := os.ReadFile(outside)
	if err != nil || string(data) != "keep" {
		t.Fatalf("outside sentinel changed: data=%q err=%v", data, err)
	}
}

func TestStateLeavesRejectHardlinksWithoutChangingOutsideFile(t *testing.T) {
	dir := filepath.Join(t.TempDir(), ".forge")
	if err := EnsurePrivateDir(dir); err != nil {
		t.Fatal(err)
	}
	outside := filepath.Join(t.TempDir(), "sentinel")
	if err := os.WriteFile(outside, []byte("keep"), 0o640); err != nil {
		t.Fatal(err)
	}
	before, err := os.Stat(outside)
	if err != nil {
		t.Fatal(err)
	}
	paths := []string{
		filepath.Join(dir, "open"),
		filepath.Join(dir, "read"),
		filepath.Join(dir, "atomic"),
	}
	for _, path := range paths {
		if err := os.Link(outside, path); err != nil {
			t.Skipf("hard links unavailable: %v", err)
		}
	}
	if file, err := OpenRegular(paths[0], os.O_RDWR, 0o600); err == nil {
		file.Close()
		t.Fatal("OpenRegular accepted a hard link")
	}
	if _, _, err := ReadRegular(paths[1], 1024); err == nil {
		t.Fatal("ReadRegular accepted a hard link")
	}
	if err := AtomicWrite(paths[2], []byte("replace"), 0o600); err == nil {
		t.Fatal("AtomicWrite accepted a hard link")
	}
	data, err := os.ReadFile(outside)
	after, statErr := os.Stat(outside)
	if err != nil || statErr != nil || string(data) != "keep" ||
		after.Mode().Perm() != before.Mode().Perm() {
		t.Fatalf("outside hardlink target changed: data=%q before=%v after=%v err=%v/%v",
			data, before.Mode().Perm(), after.Mode().Perm(), err, statErr)
	}
}

func TestRemoveRegularRejectsParentSymlink(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	sentinel := filepath.Join(outside, "decision")
	if err := os.WriteFile(sentinel, []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(root, ".forge")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	if err := RemoveRegular(filepath.Join(root, ".forge", "decision")); err == nil {
		t.Fatal("RemoveRegular accepted a parent-directory symlink")
	}
	data, err := os.ReadFile(sentinel)
	if err != nil || string(data) != "keep" {
		t.Fatalf("outside sentinel changed: data=%q err=%v", data, err)
	}
}
