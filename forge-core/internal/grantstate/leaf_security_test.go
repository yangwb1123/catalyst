//go:build unix

package grantstate

import (
	"errors"
	"io/fs"
	"os"
	"path/filepath"
	"syscall"
	"testing"
)

var errFIFOFixtureUnsupported = errors.New("FIFO test fixture is unavailable")

type foreignOwnerInfo struct{ fs.FileInfo }

func (foreignOwnerInfo) Sys() any {
	return &syscall.Stat_t{Uid: uint32(os.Geteuid() + 1), Nlink: 1}
}

func TestReadLeafUsesClosedRelativePathsAndExactModes(t *testing.T) {
	layout := newTestLayout(t, 1024)
	writeMode(t, filepath.Join(layout.authority, "input.json"), []byte("public"), 0o640)
	writeMode(t, filepath.Join(layout.authority, "private.key"), []byte("private"), 0o600)
	session := openTestSession(t, layout)
	value, err := session.ReadLeaf("input.json", 16, 0o640)
	if err != nil || string(value) != "public" {
		t.Fatalf("input = %q, %v", value, err)
	}
	value, err = session.ReadLeaf("private.key", 16, 0o600)
	if err != nil || string(value) != "private" {
		t.Fatalf("private = %q, %v", value, err)
	}
	for _, name := range []string{"", ".", "../private.key", "/private.key", `dir\leaf`, "a/../private.key"} {
		if _, err := session.ReadLeaf(name, 16, 0o600); ErrorCode(err) != CodeInvalid {
			t.Errorf("path %q: %v", name, err)
		}
	}
}

func TestReadLeafRejectsModeAndBoundsWithoutChmod(t *testing.T) {
	layout := newTestLayout(t, 1024)
	leaf := filepath.Join(layout.authority, "private.key")
	writeMode(t, leaf, []byte("secret"), 0o644)
	session := openTestSession(t, layout)
	if _, err := session.ReadLeaf("private.key", 16, 0o600); ErrorCode(err) != CodeUnsafe {
		t.Fatalf("unsafe mode: %v", err)
	}
	info, _ := os.Stat(leaf)
	if info.Mode().Perm() != 0o644 {
		t.Fatalf("mode changed to %04o", info.Mode().Perm())
	}
	if _, err := session.ReadLeaf("private.key", 3, 0o644); ErrorCode(err) != CodeUnsafe {
		t.Fatalf("oversize leaf: %v", err)
	}
	if _, err := session.ReadLeaf("private.key", 0, 0o644); ErrorCode(err) != CodeInvalid {
		t.Fatalf("invalid bound: %v", err)
	}
}

func TestReadLeafRejectsSpecialModeBitsWithoutChmod(t *testing.T) {
	for _, name := range []string{"trust-root.json", "issuer.seed"} {
		for _, bit := range []os.FileMode{os.ModeSetuid, os.ModeSetgid, os.ModeSticky} {
			t.Run(name+"-"+bit.String(), func(t *testing.T) {
				layout := newTestLayout(t, 1024)
				leaf := filepath.Join(layout.authority, name)
				writeMode(t, leaf, []byte("secret"), 0o600|bit)
				session := openTestSession(t, layout)
				if _, err := session.ReadLeaf(name, 16, 0o600); ErrorCode(err) != CodeUnsafe {
					t.Fatalf("special-mode leaf read: %v", err)
				}
				info, err := os.Lstat(leaf)
				if err != nil || info.Mode()&bit == 0 {
					t.Fatalf("special mode was changed: %v, %v", info, err)
				}
			})
		}
	}
}

func TestReadLeafRejectsSymlinkHardlinkAndFIFO(t *testing.T) {
	layout := newTestLayout(t, 1024)
	target := filepath.Join(layout.authority, "target")
	writeMode(t, target, []byte("value"), 0o600)
	if err := os.Symlink(target, filepath.Join(layout.authority, "alias")); err != nil {
		t.Fatal(err)
	}
	if err := os.Link(target, filepath.Join(layout.authority, "hard")); err != nil {
		t.Fatal(err)
	}
	if err := makeFIFO(filepath.Join(layout.authority, "pipe"), 0o600); err != nil {
		if errors.Is(err, errFIFOFixtureUnsupported) {
			t.Skip(err)
		}
		t.Fatal(err)
	}
	session := openTestSession(t, layout)
	for _, name := range []string{"alias", "target", "hard", "pipe"} {
		if _, err := session.ReadLeaf(name, 16, 0o600); ErrorCode(err) != CodeUnsafe {
			t.Errorf("leaf %q: %v", name, err)
		}
	}
}

func TestOpenRejectsUnsafeExistingStateLeaves(t *testing.T) {
	for _, name := range []string{LockFile, LedgerFile} {
		t.Run(name, func(t *testing.T) {
			layout := newTestLayout(t, 1024)
			leaf := filepath.Join(layout.state, name)
			writeMode(t, leaf, []byte("unsafe"), 0o644)
			session, err := Open(layout.config)
			if session != nil || ErrorCode(err) != CodeUnsafe {
				t.Fatalf("unsafe %s: %v, %v", name, session, err)
			}
			info, _ := os.Stat(leaf)
			if info.Mode().Perm() != 0o644 {
				t.Fatalf("mode changed to %04o", info.Mode().Perm())
			}
		})
	}
}

func TestOpenRejectsSpecialModeBitsOnLockAndLedger(t *testing.T) {
	for _, name := range []string{LockFile, LedgerFile} {
		for _, bit := range []os.FileMode{os.ModeSetuid, os.ModeSetgid, os.ModeSticky} {
			t.Run(name+"-"+bit.String(), func(t *testing.T) {
				layout := newTestLayout(t, 1024)
				leaf := filepath.Join(layout.state, name)
				writeMode(t, leaf, []byte("state"), 0o600|bit)
				session, err := Open(layout.config)
				if session != nil || ErrorCode(err) != CodeUnsafe {
					t.Fatalf("special-mode %s opened: %v, %v", name, session, err)
				}
				info, statErr := os.Lstat(leaf)
				if statErr != nil || info.Mode()&bit == 0 {
					t.Fatalf("special mode was changed: %v, %v", info, statErr)
				}
			})
		}
	}
}

func TestLedgerRejectsHardlinkAndFIFOWithoutBlocking(t *testing.T) {
	for _, kind := range []string{"hardlink", "fifo"} {
		t.Run(kind, func(t *testing.T) {
			layout := newTestLayout(t, 1024)
			ledger := ledgerPath(layout)
			if kind == "hardlink" {
				writeMode(t, ledger, []byte("value"), 0o600)
				if err := os.Link(ledger, filepath.Join(layout.state, "other")); err != nil {
					t.Fatal(err)
				}
			} else if err := makeFIFO(ledger, 0o600); err != nil {
				if errors.Is(err, errFIFOFixtureUnsupported) {
					t.Skip(err)
				}
				t.Fatal(err)
			}
			if session, err := Open(layout.config); session != nil || ErrorCode(err) != CodeUnsafe {
				t.Fatalf("unsafe ledger = %v, %v", session, err)
			}
		})
	}
}

func TestOpenOrReadRejectsAliasedStateLeaves(t *testing.T) {
	for _, name := range []string{LockFile, LedgerFile} {
		t.Run(name, func(t *testing.T) {
			layout := newTestLayout(t, 1024)
			target := filepath.Join(layout.state, "target")
			writeMode(t, target, []byte("value"), 0o600)
			if err := os.Symlink(target, filepath.Join(layout.state, name)); err != nil {
				t.Fatal(err)
			}
			session, err := Open(layout.config)
			if session != nil {
				t.Fatalf("aliased state leaf opened: %v", session)
			}
			assertCode(t, err, CodeUnsafe)
		})
	}
}

func TestOpenRejectsHardlinkedLock(t *testing.T) {
	layout := newTestLayout(t, 1024)
	lock := filepath.Join(layout.state, LockFile)
	writeMode(t, lock, nil, 0o600)
	if err := os.Link(lock, filepath.Join(layout.state, "lock-link")); err != nil {
		t.Fatal(err)
	}
	if session, err := Open(layout.config); session != nil || ErrorCode(err) != CodeUnsafe {
		t.Fatalf("hardlinked lock = %v, %v", session, err)
	}
}

func TestOpenRejectsOversizedExistingLedger(t *testing.T) {
	layout := newTestLayout(t, 4)
	writeMode(t, ledgerPath(layout), []byte("12345"), 0o600)
	if session, err := Open(layout.config); session != nil || ErrorCode(err) != CodeUnsafe {
		t.Fatalf("oversized ledger = %v, %v", session, err)
	}
}

func TestOwnerCheckRejectsNonEUIDFileInfo(t *testing.T) {
	layout := newTestLayout(t, 16)
	info, err := os.Stat(layout.authority)
	if err != nil {
		t.Fatal(err)
	}
	if err := requireOwner(foreignOwnerInfo{FileInfo: info}, "foreign leaf"); err == nil {
		t.Fatal("foreign owner was accepted")
	}
}
