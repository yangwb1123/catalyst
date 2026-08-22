//go:build unix

package grantstate

import (
	"os"
	"path/filepath"
	"testing"
)

func TestOpenRejectsUnsafeAuthorityAndStateModesWithoutChmod(t *testing.T) {
	for _, target := range []string{"authority", "state"} {
		t.Run(target, func(t *testing.T) {
			layout := newTestLayout(t, 1024)
			path := layout.authority
			if target == "state" {
				path = layout.state
			}
			if err := os.Chmod(path, 0o755); err != nil {
				t.Fatal(err)
			}
			if session, err := Open(layout.config); session != nil || ErrorCode(err) != CodeUnsafe {
				t.Fatalf("open = %v, %v", session, err)
			}
			info, _ := os.Stat(path)
			if info.Mode().Perm() != 0o755 {
				t.Fatalf("unsafe existing mode was changed to %04o", info.Mode().Perm())
			}
		})
	}
}

func TestOpenRejectsSpecialBitsOnAuthorityAndStateDirectories(t *testing.T) {
	for _, target := range []string{"authority", "state"} {
		for _, bit := range []os.FileMode{os.ModeSetuid, os.ModeSetgid, os.ModeSticky} {
			t.Run(target+"-"+bit.String(), func(t *testing.T) {
				layout := newTestLayout(t, 1024)
				path := layout.authority
				if target == "state" {
					path = layout.state
				}
				if err := os.Chmod(path, 0o700|bit); err != nil {
					t.Fatal(err)
				}
				if session, err := Open(layout.config); session != nil || ErrorCode(err) != CodeUnsafe {
					t.Fatalf("special-mode directory opened: %v, %v", session, err)
				}
				info, err := os.Lstat(path)
				if err != nil || info.Mode()&bit == 0 {
					t.Fatalf("special mode was changed: %v, %v", info, err)
				}
			})
		}
	}
}

func TestOpenRejectsMissingOrAliasedStateDirectory(t *testing.T) {
	layout := newTestLayout(t, 1024)
	if err := os.Remove(layout.state); err != nil {
		t.Fatal(err)
	}
	if session, err := Open(layout.config); session != nil || ErrorCode(err) != CodeUnsafe {
		t.Fatalf("missing state = %v, %v", session, err)
	}
	if err := os.Symlink(layout.repo, layout.state); err != nil {
		t.Fatal(err)
	}
	if session, err := Open(layout.config); session != nil || ErrorCode(err) != CodeUnsafe {
		t.Fatalf("symlink state = %v, %v", session, err)
	}
}

func TestOpenRejectsAuthorityRootAndAncestorSymlinks(t *testing.T) {
	layout := newTestLayout(t, 1024)
	alias := filepath.Join(filepath.Dir(layout.authority), "alias")
	if err := os.Symlink(layout.authority, alias); err != nil {
		t.Fatal(err)
	}
	config := layout.config
	config.AuthorityRoot = alias
	if session, err := Open(config); session != nil || ErrorCode(err) != CodeUnsafe {
		t.Fatalf("root symlink = %v, %v", session, err)
	}
	parentAlias := filepath.Join(filepath.Dir(layout.authority), "parent-alias")
	if err := os.Symlink(filepath.Dir(layout.authority), parentAlias); err != nil {
		t.Fatal(err)
	}
	config.AuthorityRoot = filepath.Join(parentAlias, filepath.Base(layout.authority))
	if session, err := Open(config); session != nil || ErrorCode(err) != CodeUnsafe {
		t.Fatalf("ancestor symlink = %v, %v", session, err)
	}
}

func TestOpenRejectsRootOverlapInBothDirections(t *testing.T) {
	layout := newTestLayout(t, 1024)
	config := layout.config
	config.RepositoryRoot = filepath.Dir(layout.authority)
	if session, err := Open(config); session != nil || ErrorCode(err) != CodeUnsafe {
		t.Fatalf("repository contains authority = %v, %v", session, err)
	}
	config.RepositoryRoot = layout.state
	if session, err := Open(config); session != nil || ErrorCode(err) != CodeUnsafe {
		t.Fatalf("authority contains repository = %v, %v", session, err)
	}
}

func TestSessionRejectsRepositoryIdentityReplacement(t *testing.T) {
	layout := newTestLayout(t, 1024)
	session := openTestSession(t, layout)
	moved := layout.repo + "-moved"
	if err := os.Rename(layout.repo, moved); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(layout.repo, 0o755); err != nil {
		t.Fatal(err)
	}
	if _, err := session.Current(); ErrorCode(err) != CodeUnsafe {
		t.Fatalf("repository replacement = %v", err)
	}
}

func TestSessionRejectsRepositorySourceSymlinkRetarget(t *testing.T) {
	layout := newTestLayout(t, 1024)
	alias := filepath.Join(filepath.Dir(layout.repo), "repository-alias")
	if err := os.Symlink(layout.repo, alias); err != nil {
		t.Fatal(err)
	}
	layout.config.RepositoryRoot = alias
	session := openTestSession(t, layout)
	if err := os.Remove(alias); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(layout.authority, alias); err != nil {
		t.Fatal(err)
	}
	if _, err := session.Current(); ErrorCode(err) != CodeUnsafe {
		t.Fatalf("retargeted repository source current = %v", err)
	}
	if _, err := session.ReadLeaf("issuer-key.json", 1024, 0o600); ErrorCode(err) != CodeUnsafe {
		t.Fatalf("retargeted repository source read = %v", err)
	}
}

func TestOpenRejectsNonCanonicalConfiguration(t *testing.T) {
	layout := newTestLayout(t, 1024)
	for _, mutate := range []func(*Config){
		func(c *Config) { c.AuthorityRoot = "relative/authority" },
		func(c *Config) { c.AuthorityRoot += string(os.PathSeparator) },
		func(c *Config) { c.StateDir = "." },
		func(c *Config) { c.StateDir = "state/../state" },
		func(c *Config) { c.StateDir = "/state" },
		func(c *Config) { c.MaxBytes = 0 },
		func(c *Config) { c.MaxBytes = AbsoluteMaxBytes + 1 },
	} {
		config := layout.config
		mutate(&config)
		if session, err := Open(config); session != nil || ErrorCode(err) != CodeInvalid {
			t.Fatalf("invalid config = %#v: %v, %v", config, session, err)
		}
	}
}

func TestNestedStateDirectoryIsBoundWithoutCreatingParents(t *testing.T) {
	layout := newTestLayout(t, 1024)
	if err := os.Rename(layout.state, filepath.Join(layout.authority, "old-state")); err != nil {
		t.Fatal(err)
	}
	control := filepath.Join(layout.authority, "control")
	state := filepath.Join(control, "state")
	if err := os.Mkdir(control, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(state, 0o700); err != nil {
		t.Fatal(err)
	}
	layout.config.StateDir = "control/state"
	session := openTestSession(t, layout)
	if err := session.Commit(Snapshot{}, []byte("ledger")); err != nil {
		t.Fatal(err)
	}
	if string(readDisk(t, filepath.Join(state, LedgerFile))) != "ledger" {
		t.Fatal("nested ledger not persisted")
	}
}
