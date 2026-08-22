//go:build unix && !aix && !solaris

package authenticatedadrlifecycleauthority

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"syscall"
	"testing"
)

func TestProtectedAuthorityRejectsUnsafeLifecycleLeaves(t *testing.T) {
	for _, relative := range []string{"profile.json", "approval-root.json", "lifecycle-root.json"} {
		for _, attack := range []string{"symlink", "hardlink", "mode", "fifo"} {
			t.Run(relative+"/"+attack, func(t *testing.T) {
				config := stateTestConfig(t)
				path := filepath.Join(config.AuthorityRoot, relative)
				payload, err := os.ReadFile(path)
				if err != nil {
					t.Fatal(err)
				}
				installUnsafeLifecycleLeaf(t, path, attack, payload)
				if _, err = readProtectedMaterials(config); err == nil {
					t.Fatal("unsafe authority leaf passed")
				}
			})
		}
	}
}

func TestProtectedAuthorityRejectsUnsafeSignerSeed(t *testing.T) {
	for _, attack := range []string{"symlink", "hardlink", "mode", "fifo"} {
		t.Run(attack, func(t *testing.T) {
			config := stateTestConfig(t)
			path := filepath.Join(config.AuthorityRoot, config.StateSignerSeedPath)
			installUnsafeLifecycleLeaf(t, path, attack, make([]byte, seedBytes))
			session, err := openProtectedState(config)
			if err != nil {
				t.Fatal(err)
			}
			defer func() { _ = session.close() }()
			if _, err = session.readLeaf(config.StateSignerSeedPath,
				seedBytes, privateMode); err == nil {
				t.Fatal("unsafe signer seed passed")
			}
		})
	}
}

func TestProtectedStateRejectsUnsafeStateAndLockLeaves(t *testing.T) {
	for _, leaf := range []string{stateFile, lockFile} {
		for _, attack := range []string{"symlink", "hardlink", "mode", "fifo"} {
			t.Run(leaf+"/"+attack, func(t *testing.T) {
				config := stateTestConfig(t)
				path := filepath.Join(config.AuthorityRoot, config.StateDir, leaf)
				installUnsafeLifecycleLeaf(t, path, attack, []byte(`{"unsafe":true}`))
				if session, err := openProtectedState(config); err == nil {
					_ = session.close()
					t.Fatal("unsafe lifecycle state leaf passed")
				}
			})
		}
	}
}

func TestProtectedStateRejectsSymlinkedBoundDirectories(t *testing.T) {
	t.Run("state", func(t *testing.T) {
		config := stateTestConfig(t)
		state := filepath.Join(config.AuthorityRoot, config.StateDir)
		target := filepath.Join(config.AuthorityRoot, "state-target")
		mustRenameLifecycle(t, state, target)
		if err := os.Symlink(target, state); err != nil {
			t.Fatal(err)
		}
		if session, err := openProtectedState(config); err == nil {
			_ = session.close()
			t.Fatal("symlinked state directory passed")
		}
	})
	t.Run("repository", func(t *testing.T) {
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

func TestBoundLifecyclePathsRejectAncestorReplacement(t *testing.T) {
	for _, target := range []string{"authority", "repository", "state"} {
		t.Run(target, func(t *testing.T) {
			config, ancestor := nestedLifecyclePath(t, stateTestConfig(t), target)
			session, err := openProtectedState(config)
			if err != nil {
				t.Fatal(err)
			}
			defer func() { _ = session.close() }()
			initial, err := session.current()
			if err != nil {
				t.Fatal(err)
			}
			replaceLifecycleAncestor(t, ancestor)
			if _, err = session.current(); err == nil {
				t.Fatal("ancestor alias passed state binding")
			}
			if _, err = session.readLeaf(config.LifecycleTrustRootPath, maxRoot, privateMode); err == nil {
				t.Fatal("ancestor alias passed authority binding")
			}
			if err = session.commit(initial, []byte(`{"state":"alias"}`)); err == nil {
				t.Fatal("ancestor alias published state")
			}
			state := filepath.Join(config.AuthorityRoot, config.StateDir, stateFile)
			if _, statErr := os.Lstat(state); !errors.Is(statErr, os.ErrNotExist) {
				t.Fatalf("ancestor alias created state: %v", statErr)
			}
		})
	}
}

func nestedLifecyclePath(t *testing.T, config Config, target string) (Config, string) {
	t.Helper()
	base := filepath.Dir(config.AuthorityRoot)
	container := filepath.Join(base, target+"-container")
	if target == "state" {
		container = filepath.Join(config.AuthorityRoot, target+"-container")
	}
	if err := os.Mkdir(container, privateDir); err != nil {
		t.Fatal(err)
	}
	switch target {
	case "authority":
		config.AuthorityRoot = moveIntoContainer(t, config.AuthorityRoot, container, "authority")
	case "repository":
		config.RepositoryRoot = moveIntoContainer(t, config.RepositoryRoot, container, "repository")
	case "state":
		old := filepath.Join(config.AuthorityRoot, config.StateDir)
		_ = moveIntoContainer(t, old, container, "state")
		config.StateDir = "state-container/state"
	default:
		t.Fatalf("unknown bound path %q", target)
	}
	return config, container
}

func moveIntoContainer(t *testing.T, old, container, name string) string {
	t.Helper()
	next := filepath.Join(container, name)
	mustRenameLifecycle(t, old, next)
	return next
}

func replaceLifecycleAncestor(t *testing.T, path string) {
	t.Helper()
	moved := path + ".moved"
	mustRenameLifecycle(t, path, moved)
	if err := os.Symlink(moved, path); err != nil {
		t.Fatal(err)
	}
}

func installUnsafeLifecycleLeaf(t *testing.T, path, attack string, payload []byte) {
	t.Helper()
	if err := os.Remove(path); err != nil && !errors.Is(err, os.ErrNotExist) {
		t.Fatal(err)
	}
	switch attack {
	case "symlink":
		target := path + ".target"
		writePrivateTest(t, target, payload)
		if err := os.Symlink(target, path); err != nil {
			t.Fatal(err)
		}
	case "hardlink":
		writePrivateTest(t, path, payload)
		if err := os.Link(path, path+".alias"); err != nil {
			t.Fatal(err)
		}
	case "mode":
		writePrivateTest(t, path, payload)
		if err := os.Chmod(path, 0o644); err != nil {
			t.Fatal(err)
		}
	case "fifo":
		if err := syscall.Mkfifo(path, uint32(privateMode)); err != nil {
			t.Fatal(err)
		}
	default:
		t.Fatalf("unknown leaf attack %q", attack)
	}
}

func mustRenameLifecycle(t *testing.T, oldPath, newPath string) {
	t.Helper()
	if err := os.Rename(oldPath, newPath); err != nil {
		t.Fatal(err)
	}
}

type tamperCommitPort struct {
	osCommitPort
	attack string
}

func (p tamperCommitPort) beforeRename(root *os.Root, name string) error {
	switch p.attack {
	case "swap":
		data, err := root.ReadFile(name)
		if err != nil {
			return err
		}
		replacement := name + ".replacement"
		if err = root.WriteFile(replacement, data, privateMode); err != nil {
			return err
		}
		if err = root.Chmod(replacement, privateMode); err != nil {
			return err
		}
		return root.Rename(replacement, name)
	case "mode":
		return root.Chmod(name, 0o644)
	case "nlink":
		return root.Link(name, name+".alias")
	case "content":
		return tamperTemporaryContent(root, name)
	default:
		return fmt.Errorf("unknown temp attack %q", p.attack)
	}
}

func tamperTemporaryContent(root *os.Root, name string) error {
	file, err := root.OpenFile(name, os.O_WRONLY, 0)
	if err != nil {
		return err
	}
	_, writeErr := file.WriteAt([]byte("!"), 0)
	if writeErr == nil {
		writeErr = file.Sync()
	}
	closeErr := file.Close()
	if writeErr != nil {
		return writeErr
	}
	return closeErr
}

func TestProtectedStateRejectsTemporaryTampering(t *testing.T) {
	for _, attack := range []string{"swap", "mode", "nlink", "content"} {
		t.Run(attack, func(t *testing.T) {
			config := stateTestConfig(t)
			session, err := openProtectedStateWith(config, tamperCommitPort{attack: attack})
			if err != nil {
				t.Fatal(err)
			}
			defer func() { _ = session.close() }()
			initial, err := session.current()
			if err != nil {
				t.Fatal(err)
			}
			err = session.commit(initial, []byte(`{"state":"tamper"}`))
			if err == nil || errors.Is(err, errStateUncertain) {
				t.Fatalf("temp tamper error=%v", err)
			}
			current, currentErr := session.current()
			if currentErr != nil || current.Present {
				t.Fatalf("tamper published state: present=%v err=%v", current.Present, currentErr)
			}
		})
	}
}
