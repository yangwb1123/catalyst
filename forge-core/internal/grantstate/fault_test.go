//go:build unix

package grantstate

import (
	"errors"
	"io/fs"
	"os"
	"testing"
)

type faultPort struct {
	base     osCommitPort
	fail     string
	mutation func() error
}

func (p *faultPort) fillRandom(value []byte) error {
	if p.fail == "random" {
		return errors.New("injected random failure")
	}
	return p.base.fillRandom(value)
}

func (p *faultPort) createExclusive(dir *os.Root, name string, mode fs.FileMode) (*os.File, error) {
	if p.fail == "create" {
		return nil, errors.New("injected create failure")
	}
	return p.base.createExclusive(dir, name, mode)
}

func (p *faultPort) writeAll(file *os.File, data []byte) error {
	if p.fail == "write" {
		return errors.New("injected write failure")
	}
	return p.base.writeAll(file, data)
}

func (p *faultPort) syncFile(file *os.File) error {
	if p.fail == "file-sync" {
		return errors.New("injected file sync failure")
	}
	if err := p.base.syncFile(file); err != nil {
		return err
	}
	if p.fail == "mutate-before-recheck" {
		return p.mutation()
	}
	return nil
}

func (p *faultPort) closeFile(file *os.File) error {
	if p.fail == "close" {
		_ = p.base.closeFile(file)
		return errors.New("injected close failure")
	}
	return p.base.closeFile(file)
}

func (p *faultPort) rename(dir *os.Root, oldName, newName string) error {
	if p.fail == "rename" {
		return errors.New("injected rename failure")
	}
	if err := p.base.rename(dir, oldName, newName); err != nil {
		return err
	}
	if p.fail == "rename-after" {
		return errors.New("injected post-publication rename failure")
	}
	return nil
}

func (p *faultPort) syncDirectory(dir *os.File) error {
	if p.fail == "dir-sync" {
		return errors.New("injected directory sync failure")
	}
	if err := p.base.syncDirectory(dir); err != nil {
		return err
	}
	if p.fail == "mutate-readback" {
		return p.mutation()
	}
	return nil
}

func (p *faultPort) remove(dir *os.Root, name string) error {
	return p.base.remove(dir, name)
}

func TestPreRenameFaultsPreserveOldLedger(t *testing.T) {
	for _, stage := range []string{"random", "create", "write", "file-sync", "close"} {
		t.Run(stage, func(t *testing.T) {
			layout := newTestLayout(t, 1024)
			writeMode(t, ledgerPath(layout), []byte("old"), 0o600)
			session, err := openWithPort(layout.config, &faultPort{fail: stage})
			if err != nil {
				t.Fatal(err)
			}
			defer func() { _ = session.Close() }()
			err = session.Commit(Snapshot{Present: true, Data: []byte("old")}, []byte("next"))
			assertCode(t, err, CodePersistence)
			if string(readDisk(t, ledgerPath(layout))) != "old" {
				t.Fatal("pre-rename failure changed old ledger")
			}
		})
	}
}

func TestExpectedOldIsRecheckedAfterTempSync(t *testing.T) {
	layout := newTestLayout(t, 1024)
	writeMode(t, ledgerPath(layout), []byte("old"), 0o600)
	port := &faultPort{fail: "mutate-before-recheck"}
	port.mutation = func() error { return os.WriteFile(ledgerPath(layout), []byte("raced"), 0o600) }
	session, err := openWithPort(layout.config, port)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = session.Close() }()
	err = session.Commit(Snapshot{Present: true, Data: []byte("old")}, []byte("next"))
	assertCode(t, err, CodeConflict)
	if string(readDisk(t, ledgerPath(layout))) != "raced" {
		t.Fatal("conflict overwrote concurrent image")
	}
}

func TestPostRenameFailuresArePersistenceUncertain(t *testing.T) {
	for _, stage := range []string{"rename", "rename-after", "dir-sync", "mutate-readback"} {
		t.Run(stage, func(t *testing.T) {
			layout := newTestLayout(t, 1024)
			port := &faultPort{fail: stage}
			port.mutation = func() error { return os.WriteFile(ledgerPath(layout), []byte("changed"), 0o600) }
			session, err := openWithPort(layout.config, port)
			if err != nil {
				t.Fatal(err)
			}
			defer func() { _ = session.Close() }()
			err = session.Commit(Snapshot{}, []byte("next"))
			if !errors.Is(err, ErrPersistenceUncertain) {
				t.Fatalf("post-rename error = %v", err)
			}
			assertCode(t, err, CodePersistenceUncertain)
			if stage == "rename-after" && string(readDisk(t, ledgerPath(layout))) != "next" {
				t.Fatal("rename-after fault did not reproduce a published image")
			}
		})
	}
}
