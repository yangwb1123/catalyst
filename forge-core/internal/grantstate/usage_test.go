//go:build unix

package grantstate

import (
	"os"
	"path/filepath"
	"testing"
)

func TestUsageNamespaceOpensAndReplaysWithoutRepository(t *testing.T) {
	layout := newTestLayout(t, 1024)
	config := Config{
		AuthorityRoot: layout.authority, StateDir: "state", MaxBytes: 1024,
	}
	session, err := OpenUsage(config)
	if err != nil {
		t.Fatal(err)
	}
	missing, err := session.Current()
	if err != nil || missing.Present {
		t.Fatalf("missing usage snapshot = %#v, %v", missing, err)
	}
	if err := session.Commit(missing, []byte(`{"usage":1}`)); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Lstat(filepath.Join(layout.state, LedgerFile)); !os.IsNotExist(err) {
		t.Fatalf("usage session touched issuance ledger: %v", err)
	}
	assertPrivateLeaf(t, filepath.Join(layout.state, usageLedgerFile))
	assertPrivateLeaf(t, filepath.Join(layout.state, usageLockFile))
	if err := session.Close(); err != nil {
		t.Fatal(err)
	}
	if err := os.RemoveAll(layout.repo); err != nil {
		t.Fatal(err)
	}
	replay, err := OpenUsage(config)
	if err != nil {
		t.Fatalf("usage replay touched missing repository: %v", err)
	}
	defer func() { _ = replay.Close() }()
	current, err := replay.Current()
	if err != nil || string(current.Data) != `{"usage":1}` {
		t.Fatalf("usage replay = %#v, %v", current, err)
	}
}

func TestUsageRepositoryBindingIsLazyStableAndSingleAssignment(t *testing.T) {
	layout := newTestLayout(t, 1024)
	session, err := OpenUsage(Config{
		AuthorityRoot: layout.authority, StateDir: "state", MaxBytes: 1024,
	})
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = session.Close() }()
	handle, err := session.BindRepository(layout.repo)
	if err != nil {
		t.Fatal(err)
	}
	if info, err := handle.Stat(); err != nil || !info.IsDir() {
		t.Fatalf("duplicated repository handle = %v, %v", info, err)
	}
	_ = handle.Close()
	if _, err := session.BindRepository(layout.repo); ErrorCode(err) != CodeConflict {
		t.Fatalf("repository rebound: %v", err)
	}
	if err := session.VerifyRepository(); err != nil {
		t.Fatal(err)
	}
}

func TestIssuanceAndUsageUseClosedIndependentFilePairs(t *testing.T) {
	layout := newTestLayout(t, 1024)
	issuance := openTestSession(t, layout)
	usage, err := OpenUsage(Config{
		AuthorityRoot: layout.authority, StateDir: "state", MaxBytes: 1024,
	})
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = usage.Close() }()
	if err := issuance.Commit(Snapshot{}, []byte("issuance")); err != nil {
		t.Fatal(err)
	}
	if err := usage.Commit(Snapshot{}, []byte("usage")); err != nil {
		t.Fatal(err)
	}
	if string(readDisk(t, filepath.Join(layout.state, LedgerFile))) != "issuance" ||
		string(readDisk(t, filepath.Join(layout.state, usageLedgerFile))) != "usage" {
		t.Fatal("closed state layouts crossed ledger images")
	}
}

func TestUsageCanPersistTerminalAfterRepositoryBindingFails(t *testing.T) {
	layout := newTestLayout(t, 1024)
	session, err := OpenUsage(Config{
		AuthorityRoot: layout.authority, StateDir: "state", MaxBytes: 1024,
	})
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = session.Close() }()
	handle, err := session.BindRepository(layout.repo)
	if err != nil {
		t.Fatal(err)
	}
	_ = handle.Close()
	if err := session.Commit(Snapshot{}, []byte("intent")); err != nil {
		t.Fatal(err)
	}
	if err := os.Rename(layout.repo, layout.repo+"-replaced"); err != nil {
		t.Fatal(err)
	}
	if err := session.VerifyRepository(); ErrorCode(err) != CodeUnsafe {
		t.Fatalf("repository replacement was not detected: %v", err)
	}
	expected, err := session.Current()
	if err != nil {
		t.Fatalf("usage state was coupled to failed repository: %v", err)
	}
	if err := session.Commit(expected, []byte("failed-consumed")); err != nil {
		t.Fatalf("terminal state could not be persisted: %v", err)
	}
}
