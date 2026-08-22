//go:build unix && !aix && !solaris

package authenticatedadrlifecycleauthority

import (
	"errors"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

type faultCommitPort struct {
	base  osCommitPort
	point string
}

func (p faultCommitPort) fillRandom(value []byte) error {
	if p.point == "random" {
		return errors.New("random")
	}
	return p.base.fillRandom(value)
}
func (p faultCommitPort) createExclusive(root *os.Root, name string, mode fs.FileMode) (*os.File, error) {
	if p.point == "create" {
		return nil, errors.New("create")
	}
	return p.base.createExclusive(root, name, mode)
}
func (p faultCommitPort) writeAll(file *os.File, data []byte) error {
	if p.point == "write" {
		return errors.New("write")
	}
	return p.base.writeAll(file, data)
}
func (p faultCommitPort) syncFile(file *os.File) error {
	if p.point == "file-sync" {
		return errors.New("file sync")
	}
	return p.base.syncFile(file)
}
func (p faultCommitPort) closeFile(file *os.File) error {
	if p.point == "close" {
		return errors.New("close")
	}
	return p.base.closeFile(file)
}
func (p faultCommitPort) beforeRename(root *os.Root, name string) error {
	if p.point == "before-rename" || p.point == "cleanup-remove" {
		return errors.New("before rename")
	}
	return p.base.beforeRename(root, name)
}
func (p faultCommitPort) rename(root *os.Root, oldName, newName string) error {
	if p.point == "rename" {
		return errors.New("rename")
	}
	return p.base.rename(root, oldName, newName)
}
func (p faultCommitPort) syncDirectory(file *os.File) error {
	if p.point == "directory-sync" {
		return errors.New("directory sync")
	}
	return p.base.syncDirectory(file)
}
func (p faultCommitPort) remove(root *os.Root, name string) error {
	if p.point == "cleanup-remove" {
		return errors.New("cleanup remove")
	}
	return p.base.remove(root, name)
}

func TestTransitionWithholdsOutputAcrossPublicationFaults(t *testing.T) {
	cases := []struct {
		point string
		code  Code
	}{
		{"random", codePersistenceFailed}, {"create", codePersistenceFailed},
		{"write", codePersistenceFailed}, {"file-sync", codePersistenceFailed},
		{"close", codePersistenceFailed}, {"before-rename", codePersistenceFailed},
		{"cleanup-remove", codePersistenceFailed},
		{"rename", codePersistenceUncertain}, {"directory-sync", codePersistenceUncertain},
	}
	for _, test := range cases {
		t.Run(test.point, func(t *testing.T) {
			fixture := newAuthorityFixture(t)
			authorization := fixture.approvalStored(t)
			input := fixture.lifecycleInput(t, authorization)
			deps := productionDependencies
			deps.openState = func(config Config) (stateSession, error) {
				return openProtectedStateWith(config, faultCommitPort{point: test.point})
			}
			stored, err := transitionAndStoreWith(fixture.lifecycleConfig, input, authorization,
				fixture.lifecycleTrust(), deps)
			if stored != nil {
				t.Fatal("fault returned StoredTransition")
			}
			assertLifecycleCode(t, err, test.code)
			if test.point == "cleanup-remove" {
				assertOnlyUnpublishedTemporary(t, fixture.lifecycleConfig)
			}
		})
	}
}

func assertOnlyUnpublishedTemporary(t *testing.T, config Config) {
	t.Helper()
	stateDir := filepath.Join(config.AuthorityRoot, config.StateDir)
	entries, err := os.ReadDir(stateDir)
	if err != nil {
		t.Fatal(err)
	}
	temporary := 0
	for _, entry := range entries {
		if entry.Name() == stateFile {
			t.Fatal("cleanup failure published lifecycle state")
		}
		if strings.HasPrefix(entry.Name(), "."+stateFile+".tmp-") {
			temporary++
		}
	}
	if temporary != 1 {
		t.Fatalf("unpublished temporary count=%d", temporary)
	}
}

type wrappedFaultSession struct {
	stateSession
	corruptAfterCommit bool
	closeFailure       bool
	committed          bool
}

func (s *wrappedFaultSession) commit(expected stateSnapshot, next []byte) error {
	err := s.stateSession.commit(expected, next)
	if err == nil {
		s.committed = true
	}
	return err
}

func (s *wrappedFaultSession) current() (stateSnapshot, error) {
	value, err := s.stateSession.current()
	if err == nil && s.committed && s.corruptAfterCommit && len(value.Data) > 0 {
		value.Data[0] ^= 1
	}
	return value, err
}

func (s *wrappedFaultSession) close() error {
	err := s.stateSession.close()
	if err != nil {
		return err
	}
	if s.closeFailure {
		return errors.New("injected close failure")
	}
	return nil
}

func TestTransitionWithholdsOutputOnReopenAndCloseUncertainty(t *testing.T) {
	for _, point := range []string{"reopen", "close"} {
		t.Run(point, func(t *testing.T) {
			fixture := newAuthorityFixture(t)
			authorization := fixture.approvalStored(t)
			deps := productionDependencies
			deps.openState = func(config Config) (stateSession, error) {
				base, err := openProtectedState(config)
				if err != nil {
					return nil, err
				}
				return &wrappedFaultSession{stateSession: base, corruptAfterCommit: point == "reopen", closeFailure: point == "close"}, nil
			}
			stored, err := transitionAndStoreWith(fixture.lifecycleConfig,
				fixture.lifecycleInput(t, authorization), authorization, fixture.lifecycleTrust(), deps)
			if stored != nil {
				t.Fatal("uncertainty returned StoredTransition")
			}
			assertLifecycleCode(t, err, codePersistenceUncertain)
		})
	}
}

func TestSignerMismatchFailsBeforePublication(t *testing.T) {
	fixture := newAuthorityFixture(t)
	authorization := fixture.approvalStored(t)
	if err := os.WriteFile(filepath.Join(fixture.lifecycleConfig.AuthorityRoot,
		fixture.lifecycleConfig.StateSignerSeedPath), make([]byte, 32), privateMode); err != nil {
		t.Fatal(err)
	}
	stored, err := TransitionAndStore(fixture.lifecycleConfig,
		fixture.lifecycleInput(t, authorization), authorization, fixture.lifecycleTrust())
	if stored != nil {
		t.Fatal("mismatched signer returned output")
	}
	assertLifecycleCode(t, err, codeSignerKeyRejected)
}
