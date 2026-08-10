package statefs

import (
	"bytes"
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
		if closeErr := file.Close(); closeErr != nil {
			t.Fatalf("close unexpectedly accepted hard link: %v", closeErr)
		}
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

func TestTrackedReadAndAtomicWritePreserveRepositoryDirectoryMode(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, ".agent")
	if err := os.Mkdir(dir, 0o775); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(dir, "project.yml")
	if err := os.WriteFile(path, []byte("mode: explorer\n"), 0o640); err != nil {
		t.Fatal(err)
	}
	beforeDir, err := os.Stat(dir)
	if err != nil {
		t.Fatal(err)
	}
	data, mode, present, err := ReadTracked(path, 1024)
	if err != nil || !present || string(data) != "mode: explorer\n" || mode != 0o640 {
		t.Fatalf("ReadTracked = data=%q mode=%#o present=%v err=%v",
			data, mode, present, err)
	}
	if err := AtomicWriteTrackedIfUnchanged(
		path, data, mode, true, []byte("mode: engineering\n"), mode,
	); err != nil {
		t.Fatal(err)
	}
	afterDir, err := os.Stat(dir)
	if err != nil {
		t.Fatal(err)
	}
	afterData, afterMode, present, err := ReadTracked(path, 1024)
	if err != nil || !present || string(afterData) != "mode: engineering\n" ||
		afterMode != 0o640 {
		t.Fatalf("published tracked file = data=%q mode=%#o present=%v err=%v",
			afterData, afterMode, present, err)
	}
	if afterDir.Mode().Perm() != beforeDir.Mode().Perm() {
		t.Fatalf("tracked write changed parent mode %#o -> %#o",
			beforeDir.Mode().Perm(), afterDir.Mode().Perm())
	}
}

func TestTrackedOperationsRejectAliasesAndSpecialFiles(t *testing.T) {
	for _, kind := range []string{"symlink", "hardlink", "directory"} {
		t.Run(kind, func(t *testing.T) {
			root := t.TempDir()
			dir := filepath.Join(root, ".agent")
			if err := os.Mkdir(dir, 0o755); err != nil {
				t.Fatal(err)
			}
			path := filepath.Join(dir, "ROADMAP.md")
			outside := filepath.Join(t.TempDir(), "outside")
			if err := os.WriteFile(outside, []byte("keep"), 0o640); err != nil {
				t.Fatal(err)
			}
			switch kind {
			case "symlink":
				if err := os.Symlink(outside, path); err != nil {
					t.Skip(err)
				}
			case "hardlink":
				if err := os.Link(outside, path); err != nil {
					t.Skip(err)
				}
			case "directory":
				if err := os.Mkdir(path, 0o755); err != nil {
					t.Fatal(err)
				}
			}
			if _, _, _, err := ReadTracked(path, 1024); err == nil {
				t.Fatal("ReadTracked accepted an alias or special file")
			}
			if err := AtomicWriteTrackedIfUnchanged(
				path, nil, 0, false, []byte("replace"), 0o644,
			); err == nil {
				t.Fatal("AtomicWriteTracked accepted an alias or special file")
			}
			got, err := os.ReadFile(outside)
			if err != nil || string(got) != "keep" {
				t.Fatalf("outside target changed: data=%q err=%v", got, err)
			}
		})
	}
}

func TestAtomicWriteTrackedRejectsInPlaceImageDrift(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, ".agent")
	if err := os.Mkdir(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(dir, "project.yml")
	expected := []byte("mode: explorer\n")
	drifted := []byte("mode: balanced\n")
	if err := os.WriteFile(path, expected, 0o640); err != nil {
		t.Fatal(err)
	}
	before, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, drifted, 0o640); err != nil {
		t.Fatal(err)
	}
	after, err := os.Stat(path)
	if err != nil || !os.SameFile(before, after) {
		t.Fatalf("fixture did not retain inode: before=%v after=%v err=%v", before, after, err)
	}
	err = AtomicWriteTrackedIfUnchanged(
		path, expected, 0o640, true, []byte("mode: engineering\n"), 0o640,
	)
	if err == nil {
		t.Fatal("tracked CAS accepted in-place content drift")
	}
	got, readErr := os.ReadFile(path)
	if readErr != nil || !bytes.Equal(got, drifted) {
		t.Fatalf("tracked CAS overwrote drift: data=%q err=%v", got, readErr)
	}
}
