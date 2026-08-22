//go:build unix

package grantstate

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestMissingCommitReadAndExactReplay(t *testing.T) {
	layout := newTestLayout(t, 1024)
	session := openTestSession(t, layout)
	missing, err := session.Current()
	if err != nil || missing.Present || len(missing.Data) != 0 {
		t.Fatalf("missing snapshot = %#v, %v", missing, err)
	}
	next := []byte(`{"sequence":1}`)
	if err := session.Commit(missing, next); err != nil {
		t.Fatal(err)
	}
	next[0] = 'x'
	current, err := session.Current()
	if err != nil || !current.Present || string(current.Data) != `{"sequence":1}` {
		t.Fatalf("current = %#v, %v", current, err)
	}
	if err := session.Commit(missing, current.Data); err != nil {
		t.Fatalf("exact replay: %v", err)
	}
	assertPrivateLeaf(t, ledgerPath(layout))
	assertPrivateLeaf(t, filepath.Join(layout.state, LockFile))
}

func TestLedgerSurvivesCloseAndLockIsNeverUnlinked(t *testing.T) {
	layout := newTestLayout(t, 1024)
	session := openTestSession(t, layout)
	if err := session.Commit(Snapshot{}, []byte("first")); err != nil {
		t.Fatal(err)
	}
	if err := session.Close(); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(layout.config)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = reopened.Close() }()
	current, err := reopened.Current()
	if err != nil || string(current.Data) != "first" {
		t.Fatalf("reopened = %#v, %v", current, err)
	}
	if _, err := os.Lstat(filepath.Join(layout.state, LockFile)); err != nil {
		t.Fatalf("stable lock was removed: %v", err)
	}
}

func TestSecondSessionFailsBusyWithoutBlocking(t *testing.T) {
	layout := newTestLayout(t, 1024)
	first := openTestSession(t, layout)
	second, err := Open(layout.config)
	if second != nil || !errors.Is(err, ErrBusy) {
		t.Fatalf("second open = %v, %v", second, err)
	}
	assertCode(t, err, CodeBusy)
	if err := first.Close(); err != nil {
		t.Fatal(err)
	}
	third, err := Open(layout.config)
	if err != nil {
		t.Fatal(err)
	}
	_ = third.Close()
}

func TestCommitRejectsConflictAndInvalidImages(t *testing.T) {
	layout := newTestLayout(t, 8)
	session := openTestSession(t, layout)
	if err := session.Commit(Snapshot{}, nil); ErrorCode(err) != CodeInvalid {
		t.Fatalf("empty next: %v", err)
	}
	if err := session.Commit(Snapshot{}, []byte("123456789")); ErrorCode(err) != CodeInvalid {
		t.Fatalf("oversize next: %v", err)
	}
	if err := session.Commit(Snapshot{Data: []byte("ghost")}, []byte("next")); ErrorCode(err) != CodeInvalid {
		t.Fatalf("invalid missing expected: %v", err)
	}
	if err := session.Commit(Snapshot{}, []byte("old")); err != nil {
		t.Fatal(err)
	}
	err := session.Commit(Snapshot{Present: true, Data: []byte("wrong")}, []byte("next"))
	assertCode(t, err, CodeConflict)
	if !bytes.Equal(readDisk(t, ledgerPath(layout)), []byte("old")) {
		t.Fatal("conflict changed ledger")
	}
}

func TestClosedSessionFailsClosed(t *testing.T) {
	layout := newTestLayout(t, 16)
	session, err := Open(layout.config)
	if err != nil {
		t.Fatal(err)
	}
	if err := session.Close(); err != nil {
		t.Fatal(err)
	}
	_, err = session.Current()
	assertCode(t, err, CodeClosed)
	if err := session.Commit(Snapshot{}, []byte("x")); ErrorCode(err) != CodeClosed {
		t.Fatalf("commit after close: %v", err)
	}
	if err := session.Close(); err != nil {
		t.Fatalf("second close: %v", err)
	}
}

func assertPrivateLeaf(t *testing.T, name string) {
	t.Helper()
	info, err := os.Lstat(name)
	if err != nil {
		t.Fatal(err)
	}
	if !info.Mode().IsRegular() || info.Mode().Perm() != 0o600 {
		t.Fatalf("%s mode = %v", name, info.Mode())
	}
}
