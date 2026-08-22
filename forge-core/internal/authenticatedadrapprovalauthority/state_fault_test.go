//go:build unix

package authenticatedadrapprovalauthority

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"testing"
)

type faultCommitPort struct {
	osCommitPort
	fail string
}

func (p faultCommitPort) injected(stage string) error {
	if p.fail == stage {
		return fmt.Errorf("injected %s failure", stage)
	}
	return nil
}

func (p faultCommitPort) fillRandom(value []byte) error {
	if err := p.injected("random"); err != nil {
		return err
	}
	return p.osCommitPort.fillRandom(value)
}

func (p faultCommitPort) createExclusive(root *os.Root, name string,
	mode fs.FileMode) (*os.File, error) {
	if err := p.injected("create"); err != nil {
		return nil, err
	}
	return p.osCommitPort.createExclusive(root, name, mode)
}

func (p faultCommitPort) writeAll(file *os.File, data []byte) error {
	if err := p.injected("write"); err != nil {
		return err
	}
	return p.osCommitPort.writeAll(file, data)
}

func (p faultCommitPort) syncFile(file *os.File) error {
	if err := p.injected("file-sync"); err != nil {
		return err
	}
	return p.osCommitPort.syncFile(file)
}

func (p faultCommitPort) closeFile(file *os.File) error {
	if err := p.injected("close"); err != nil {
		return err
	}
	return p.osCommitPort.closeFile(file)
}

func (p faultCommitPort) beforeRename(*os.Root, string) error {
	return p.injected("before-rename")
}

func (p faultCommitPort) rename(root *os.Root, oldName, newName string) error {
	if err := p.injected("rename"); err != nil {
		return err
	}
	return p.osCommitPort.rename(root, oldName, newName)
}

func (p faultCommitPort) syncDirectory(file *os.File) error {
	if err := p.injected("directory-sync"); err != nil {
		return err
	}
	return p.osCommitPort.syncDirectory(file)
}

func TestProtectedStateInjectedCommitFailures(t *testing.T) {
	stages := []string{"random", "create", "write", "file-sync", "close",
		"before-rename", "rename", "directory-sync"}
	for _, stage := range stages {
		t.Run(stage, func(t *testing.T) {
			config := stateTestConfig(t)
			session, err := openProtectedStateWith(config,
				faultCommitPort{fail: stage})
			if err != nil {
				t.Fatal(err)
			}
			initial, err := session.current()
			if err != nil {
				t.Fatal(err)
			}
			err = session.commit(initial, []byte(`{"ledger":"fault"}`))
			if err == nil {
				t.Fatal("injected commit failure returned success")
			}
			ambiguous := stage == "rename" || stage == "directory-sync"
			if errors.Is(err, errStateUncertain) != ambiguous {
				t.Fatalf("uncertainty = %v at %s: %v",
					errors.Is(err, errStateUncertain), stage, err)
			}
			if closeErr := session.close(); closeErr != nil {
				t.Fatal(closeErr)
			}
		})
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
	default:
		return fmt.Errorf("unknown temp attack %q", p.attack)
	}
}

func TestProtectedStateRejectsTempTamperingBeforeRename(t *testing.T) {
	for _, attack := range []string{"swap", "mode", "nlink", "content"} {
		t.Run(attack, func(t *testing.T) {
			config := stateTestConfig(t)
			session, err := openProtectedStateWith(config,
				tamperCommitPort{attack: attack})
			if err != nil {
				t.Fatal(err)
			}
			defer func() { _ = session.close() }()
			initial, err := session.current()
			if err != nil {
				t.Fatal(err)
			}
			err = session.commit(initial, []byte(`{"ledger":"tamper"}`))
			if err == nil || errors.Is(err, errStateUncertain) {
				t.Fatalf("temp tamper error = %v", err)
			}
			current, currentErr := session.current()
			if currentErr != nil || current.Present {
				t.Fatalf("tamper published ledger: present=%v err=%v",
					current.Present, currentErr)
			}
		})
	}
}
