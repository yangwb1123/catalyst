//go:build linux && !android

package appserver

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"syscall"
	"testing"
)

func TestInstanceLockIsPrivateExclusiveAndReusable(t *testing.T) {
	stateDir := filepath.Join(privateTestParent(t), "state")
	first, err := acquireInstanceLock(stateDir)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = first.Close() }()
	assertPrivateState(t, stateDir)
	if _, err := acquireInstanceLock(stateDir); !errors.Is(err, ErrAlreadyRunning) {
		t.Fatalf("second lock error = %v", err)
	}
	if err := first.Close(); err != nil {
		t.Fatal(err)
	}
	third, err := acquireInstanceLock(stateDir)
	if err != nil {
		t.Fatalf("reacquire: %v", err)
	}
	_ = third.Close()
}

func assertPrivateState(t *testing.T, stateDir string) {
	t.Helper()
	dirInfo, err := os.Stat(stateDir)
	if err != nil || dirInfo.Mode().Perm() != 0o700 {
		t.Fatalf("state directory = %v, %v", dirInfo, err)
	}
	lockInfo, err := os.Stat(filepath.Join(stateDir, "server.lock"))
	if err != nil || lockInfo.Mode().Perm() != 0o600 {
		t.Fatalf("lock file = %v, %v", lockInfo, err)
	}
	identity, err := os.ReadFile(filepath.Join(stateDir, stateIdentityName))
	if err != nil || !bytes.Equal(identity, stateIdentity) {
		t.Fatalf("state identity = %q, %v", identity, err)
	}
}

func TestInstanceLockRejectsInsecureExistingDirectoryWithoutChmod(t *testing.T) {
	stateDir := filepath.Join(privateTestParent(t), "state")
	if err := os.Mkdir(stateDir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(stateDir, 0o755); err != nil {
		t.Fatal(err)
	}
	if _, err := acquireInstanceLock(stateDir); err == nil {
		t.Fatal("insecure state directory succeeded")
	}
	info, err := os.Stat(stateDir)
	if err != nil || info.Mode().Perm() != 0o755 {
		t.Fatalf("state directory permissions changed: %v, %v", info, err)
	}
}

func TestInstanceLockRejectsSymlinkStateDirectory(t *testing.T) {
	parent := privateTestParent(t)
	target := filepath.Join(parent, "target")
	if err := os.Mkdir(target, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(target, filepath.Join(parent, "state")); err != nil {
		t.Fatal(err)
	}
	if _, err := acquireInstanceLock(filepath.Join(parent, "state")); err == nil {
		t.Fatal("symlink state directory succeeded")
	}
}

func TestInstanceLockRejectsUndedicatedPrivateDirectory(t *testing.T) {
	stateDir := filepath.Join(privateTestParent(t), "state")
	if err := os.Mkdir(stateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	if _, err := acquireInstanceLock(stateDir); err == nil {
		t.Fatal("unmarked private directory succeeded")
	}
	entries, err := os.ReadDir(stateDir)
	if err != nil || len(entries) != 0 {
		t.Fatalf("unmarked directory was mutated: %v, %v", entries, err)
	}
}

func TestInstanceLockRejectsStateDirectorySpecialModeBits(t *testing.T) {
	for _, bit := range []os.FileMode{os.ModeSticky, os.ModeSetgid, os.ModeSetuid} {
		t.Run(bit.String(), func(t *testing.T) {
			stateDir := initializedStateDir(t)
			if err := os.Chmod(stateDir, 0o700|bit); err != nil {
				t.Fatal(err)
			}
			if _, err := acquireInstanceLock(stateDir); err == nil {
				t.Fatalf("state mode %v succeeded", bit)
			}
			info, err := os.Stat(stateDir)
			if err != nil || info.Mode()&bit == 0 {
				t.Fatalf("state mode was changed: %v, %v", info, err)
			}
		})
	}
}

func TestInstanceLockRejectsUnknownStateEntry(t *testing.T) {
	stateDir := initializedStateDir(t)
	if err := os.WriteFile(filepath.Join(stateDir, "workspace.txt"), []byte("not app state"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := acquireInstanceLock(stateDir); err == nil {
		t.Fatal("state directory with unknown content succeeded")
	}
}

func TestInstanceLockRejectsHardLinkedLockFile(t *testing.T) {
	parent := privateTestParent(t)
	stateDir := filepath.Join(parent, "state")
	lock, err := acquireInstanceLock(stateDir)
	if err != nil {
		t.Fatal(err)
	}
	if err := lock.Close(); err != nil {
		t.Fatal(err)
	}
	lockPath := filepath.Join(stateDir, stateLockName)
	if err := os.Remove(lockPath); err != nil {
		t.Fatal(err)
	}
	external := filepath.Join(parent, "external-lock")
	if err := os.WriteFile(external, stateLock, 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Link(external, lockPath); err != nil {
		t.Fatal(err)
	}
	if _, err := acquireInstanceLock(stateDir); err == nil {
		t.Fatal("hard-linked lock file succeeded")
	}
}

func TestInstanceLockRejectsSymlinkAncestor(t *testing.T) {
	parent := privateTestParent(t)
	realParent := filepath.Join(parent, "real")
	if err := os.Mkdir(realParent, 0o700); err != nil {
		t.Fatal(err)
	}
	alias := filepath.Join(parent, "alias")
	if err := os.Symlink(realParent, alias); err != nil {
		t.Fatal(err)
	}
	if _, err := acquireInstanceLock(filepath.Join(alias, "state")); err == nil {
		t.Fatal("state path with symlink ancestor succeeded")
	}
}

func TestInstanceLockRejectsReplaceableAncestor(t *testing.T) {
	parent := privateTestParent(t)
	replaceable := filepath.Join(parent, "replaceable")
	if err := os.Mkdir(replaceable, 0o777); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(replaceable, 0o777); err != nil {
		t.Fatal(err)
	}
	stateDir := filepath.Join(replaceable, "state")
	if _, err := acquireInstanceLock(stateDir); err == nil {
		t.Fatal("state path beneath a replaceable ancestor succeeded")
	}
	if _, err := os.Lstat(stateDir); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("replaceable ancestor path was mutated: %v", err)
	}
}

func TestInstanceLockRejectsWritableUpperAncestor(t *testing.T) {
	parent := privateTestParent(t)
	outer := filepath.Join(parent, "outer")
	if err := os.Mkdir(outer, 0o777); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(outer, 0o777); err != nil {
		t.Fatal(err)
	}
	anchor := filepath.Join(outer, "anchor")
	if err := os.Mkdir(anchor, 0o700); err != nil {
		t.Fatal(err)
	}
	stateDir := filepath.Join(anchor, "state")
	if _, err := acquireInstanceLock(stateDir); err == nil {
		t.Fatal("private state parent beneath a writable ancestor succeeded")
	}
	if _, err := os.Lstat(stateDir); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("writable ancestor path was mutated: %v", err)
	}
}

func TestStateIdentityPredicatesRejectForeignOwner(t *testing.T) {
	directory, err := os.Stat(privateTestParent(t))
	if err != nil {
		t.Fatal(err)
	}
	foreignDirectory := ownerOverride{
		FileInfo: directory,
		system:   &syscall.Stat_t{Uid: uint32(os.Geteuid() + 1)},
	}
	if requireStateAnchor(foreignDirectory) == nil || requireStateDirectory(foreignDirectory) == nil {
		t.Fatal("foreign-owned directory passed an identity predicate")
	}
	path := filepath.Join(privateTestParent(t), "leaf")
	if err := os.WriteFile(path, stateLock, 0o600); err != nil {
		t.Fatal(err)
	}
	file, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	foreignFile := ownerOverride{
		FileInfo: file,
		system:   &syscall.Stat_t{Uid: uint32(os.Geteuid() + 1), Nlink: 1},
	}
	if requireStateFile(foreignFile) == nil {
		t.Fatal("foreign-owned file passed an identity predicate")
	}
}

func TestBoundStateAnchorRejectsInodeReuseABA(t *testing.T) {
	directory, err := os.Stat(privateTestParent(t))
	if err != nil {
		t.Fatal(err)
	}
	stat := *directory.Sys().(*syscall.Stat_t)
	stat.Uid = uint32(os.Geteuid() + 1)
	foreign := ownerOverride{FileInfo: directory, system: &stat}
	sameInode := func(os.FileInfo, os.FileInfo) bool { return true }
	if validateBoundStateAnchorWith(directory, foreign, directory, sameInode) == nil {
		t.Fatal("same-inode foreign-owner replacement passed binding")
	}
	unsafe := modeOverride{FileInfo: directory, mode: directory.Mode() | 0o022}
	if validateBoundStateAnchorWith(directory, unsafe, directory, sameInode) == nil {
		t.Fatal("same-inode writable replacement passed binding")
	}
}

func initializedStateDir(t *testing.T) string {
	t.Helper()
	stateDir := filepath.Join(privateTestParent(t), "state")
	lock, err := acquireInstanceLock(stateDir)
	if err != nil {
		t.Fatal(err)
	}
	if err := lock.Close(); err != nil {
		t.Fatal(err)
	}
	return stateDir
}

type ownerOverride struct {
	os.FileInfo
	system any
}

func (info ownerOverride) Sys() any { return info.system }

type modeOverride struct {
	os.FileInfo
	mode os.FileMode
}

func (info modeOverride) Mode() os.FileMode { return info.mode }

func privateTestParent(t *testing.T) string {
	t.Helper()
	parent := t.TempDir()
	if err := os.Chmod(parent, 0o700); err != nil {
		t.Fatal(err)
	}
	return parent
}
