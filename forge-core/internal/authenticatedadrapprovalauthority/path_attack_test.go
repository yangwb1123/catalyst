//go:build unix

package authenticatedadrapprovalauthority

import (
	"errors"
	"os"
	"path/filepath"
	"syscall"
	"testing"
)

func TestProtectedAuthorityRejectsTrustRootAttacks(t *testing.T) {
	t.Run("trust root symlink", func(t *testing.T) {
		config := stateTestConfig(t)
		root := filepath.Join(config.AuthorityRoot, config.TrustRootPath)
		target := filepath.Join(config.AuthorityRoot, "root-target.json")
		mustRename(t, root, target)
		if err := os.Symlink(target, root); err != nil {
			t.Fatal(err)
		}
		if _, err := readProtectedTrustRoot(config); err == nil {
			t.Fatal("symlinked trust root passed")
		}
	})
	t.Run("trust root mode", func(t *testing.T) {
		config := stateTestConfig(t)
		if err := os.Chmod(filepath.Join(config.AuthorityRoot, config.TrustRootPath),
			0o644); err != nil {
			t.Fatal(err)
		}
		if _, err := readProtectedTrustRoot(config); err == nil {
			t.Fatal("public trust root mode passed")
		}
	})
}

func TestProtectedAuthorityRejectsSeedFIFO(t *testing.T) {
	config := stateTestConfig(t)
	seed := filepath.Join(config.AuthorityRoot, config.StateSignerSeedPath)
	if err := os.Remove(seed); err != nil {
		t.Fatal(err)
	}
	if err := syscall.Mkfifo(seed, uint32(privateMode)); err != nil {
		t.Fatal(err)
	}
	session, err := openProtectedState(config)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = session.close() }()
	if _, err = session.readLeaf(config.StateSignerSeedPath,
		stateSeedBytes, privateMode); err == nil {
		t.Fatal("FIFO signer seed passed")
	}
}

func TestProtectedAuthorityRejectsRootDirectorySymlinks(t *testing.T) {
	t.Run("state directory symlink", func(t *testing.T) {
		config := stateTestConfig(t)
		state := filepath.Join(config.AuthorityRoot, config.StateDir)
		target := filepath.Join(config.AuthorityRoot, "state-target")
		if err := os.Remove(state); err != nil {
			t.Fatal(err)
		}
		if err := os.Mkdir(target, privateDirMode); err != nil {
			t.Fatal(err)
		}
		if err := os.Symlink(target, state); err != nil {
			t.Fatal(err)
		}
		if session, err := openProtectedState(config); err == nil {
			_ = session.close()
			t.Fatal("symlinked state directory passed")
		}
	})
	t.Run("repository symlink", func(t *testing.T) {
		config := stateTestConfig(t)
		alias := filepath.Join(filepath.Dir(config.RepositoryRoot), "repository-alias")
		if err := os.Symlink(config.RepositoryRoot, alias); err != nil {
			t.Fatal(err)
		}
		config.RepositoryRoot = alias
		if session, err := openProtectedState(config); err == nil {
			_ = session.close()
			t.Fatal("symlinked repository root passed")
		}
	})
}

func TestProtectedStateRejectsUnsafeExistingLedgerAndLock(t *testing.T) {
	for _, leaf := range []string{stateLedgerFile, stateLockFile} {
		for _, attack := range []string{"symlink", "hardlink", "mode", "fifo"} {
			t.Run(leaf+"/"+attack, func(t *testing.T) {
				config := stateTestConfig(t)
				state := filepath.Join(config.AuthorityRoot, config.StateDir)
				path := filepath.Join(state, leaf)
				installUnsafeLeaf(t, path, attack)
				if session, err := openProtectedState(config); err == nil {
					_ = session.close()
					t.Fatal("unsafe state leaf passed")
				}
			})
		}
	}
}

func TestBoundAuthorityLeavesRejectReplacementAndInPlaceMutation(t *testing.T) {
	t.Run("root inode replacement", func(t *testing.T) {
		config := stateTestConfig(t)
		session, err := openProtectedState(config)
		if err != nil {
			t.Fatal(err)
		}
		defer func() { _ = session.close() }()
		rootPath := filepath.Join(config.AuthorityRoot, config.TrustRootPath)
		original, err := session.readLeaf(config.TrustRootPath,
			maxTrustRootBytes, privateMode)
		if err != nil {
			t.Fatal(err)
		}
		replacement := rootPath + ".replacement"
		testWritePrivate(t, replacement, original)
		if err = os.Rename(replacement, rootPath); err != nil {
			t.Fatal(err)
		}
		if _, err = session.current(); err == nil {
			t.Fatal("same-byte root inode replacement passed")
		}
	})
	t.Run("seed in-place mutation", func(t *testing.T) {
		config := stateTestConfig(t)
		session, err := openProtectedState(config)
		if err != nil {
			t.Fatal(err)
		}
		defer func() { _ = session.close() }()
		if _, err = session.readLeaf(config.StateSignerSeedPath,
			stateSeedBytes, privateMode); err != nil {
			t.Fatal(err)
		}
		seedPath := filepath.Join(config.AuthorityRoot, config.StateSignerSeedPath)
		if err = os.WriteFile(seedPath, make([]byte, stateSeedBytes), privateMode); err != nil {
			t.Fatal(err)
		}
		if _, err = session.current(); err == nil {
			t.Fatal("in-place seed mutation passed")
		}
	})
}

func TestBoundPathsRejectAncestorSwapToSelfAlias(t *testing.T) {
	for _, target := range []string{"authority", "state", "repository"} {
		t.Run(target, func(t *testing.T) {
			config := stateTestConfig(t)
			config, ancestor := nestBoundPathForAttack(t, config, target)
			session, err := openProtectedState(config)
			if err != nil {
				t.Fatal(err)
			}
			defer func() { _ = session.close() }()
			initial, err := session.current()
			if err != nil {
				t.Fatal(err)
			}
			replaceDirectoryWithSelfAlias(t, ancestor)
			if _, err = session.current(); err == nil {
				t.Fatal("ancestor alias passed current binding check")
			}
			if _, err = session.readLeaf(config.TrustRootPath,
				maxTrustRootBytes, privateMode); err == nil {
				t.Fatal("ancestor alias passed authority leaf binding check")
			}
			if err = session.commit(initial, []byte(`{"ledger":"alias"}`)); err == nil {
				t.Fatal("ancestor alias published a ledger")
			}
			ledger := filepath.Join(config.AuthorityRoot, config.StateDir, stateLedgerFile)
			if _, statErr := os.Lstat(ledger); !errors.Is(statErr, os.ErrNotExist) {
				t.Fatalf("ancestor alias created ledger: %v", statErr)
			}
		})
	}
}

func nestBoundPathForAttack(t *testing.T, config Config, target string) (Config, string) {
	t.Helper()
	base := filepath.Dir(config.AuthorityRoot)
	container := filepath.Join(base, target+"-container")
	if target == "state" {
		container = filepath.Join(config.AuthorityRoot, target+"-container")
	}
	if err := os.Mkdir(container, privateDirMode); err != nil {
		t.Fatal(err)
	}
	switch target {
	case "authority":
		nested := filepath.Join(container, "authority")
		mustRename(t, config.AuthorityRoot, nested)
		config.AuthorityRoot = nested
	case "repository":
		nested := filepath.Join(container, "repository")
		mustRename(t, config.RepositoryRoot, nested)
		config.RepositoryRoot = nested
	case "state":
		nested := filepath.Join(container, "state")
		mustRename(t, filepath.Join(config.AuthorityRoot, config.StateDir), nested)
		config.StateDir = target + "-container/state"
	default:
		t.Fatalf("unknown bound path target %q", target)
	}
	return config, container
}

func replaceDirectoryWithSelfAlias(t *testing.T, path string) {
	t.Helper()
	moved := path + ".moved"
	mustRename(t, path, moved)
	if err := os.Symlink(moved, path); err != nil {
		t.Fatal(err)
	}
}

func installUnsafeLeaf(t *testing.T, path, attack string) {
	t.Helper()
	switch attack {
	case "symlink":
		target := path + ".target"
		testWritePrivate(t, target, []byte("{}"))
		if err := os.Symlink(target, path); err != nil {
			t.Fatal(err)
		}
	case "hardlink":
		testWritePrivate(t, path, []byte("{}"))
		if err := os.Link(path, path+".alias"); err != nil {
			t.Fatal(err)
		}
	case "mode":
		testWritePrivate(t, path, []byte("{}"))
		if err := os.Chmod(path, 0o644); err != nil {
			t.Fatal(err)
		}
	case "fifo":
		if err := syscall.Mkfifo(path, uint32(privateMode)); err != nil {
			t.Fatal(err)
		}
	default:
		t.Fatalf("unknown attack %q", attack)
	}
}

func mustRename(t *testing.T, oldPath, newPath string) {
	t.Helper()
	if err := os.Rename(oldPath, newPath); err != nil {
		t.Fatal(err)
	}
}
