//go:build unix

package grantstate

import (
	"fmt"
	"os"
	"syscall"
)

func (s *unixSession) verifyBindings() error {
	if s.lock == nil {
		return newError(CodeClosed, "verify", "session is closed", nil)
	}
	if err := s.verifyAuthorityBinding(); err != nil {
		return newError(CodeUnsafe, "verify", "authority root binding changed", err)
	}
	if s.repositoryDir != nil && s.layout == issuanceLayout {
		if err := s.verifyBoundRepository(); err != nil {
			return err
		}
	}
	if err := s.verifyStateBinding(); err != nil {
		return newError(CodeUnsafe, "verify", "state directory binding changed", err)
	}
	if err := s.verifyLockBinding(); err != nil {
		return newError(CodeUnsafe, "verify", "lock binding changed", err)
	}
	return nil
}

func (s *unixSession) verifyBoundRepository() error {
	if err := s.verifyRepositoryBinding(); err != nil {
		return newError(CodeUnsafe, "verify", "repository root binding changed", err)
	}
	if err := requireRootsDisjoint(s.authorityPath, s.authorityInfo, s.repositoryBinding()); err != nil {
		return newError(CodeUnsafe, "verify", "authority and repository roots overlap", err)
	}
	if err := s.verifyAuthorityBinding(); err != nil {
		return newError(CodeUnsafe, "verify", "authority root changed during overlap check", err)
	}
	if err := s.verifyRepositoryBinding(); err != nil {
		return newError(CodeUnsafe, "verify", "repository root changed during overlap check", err)
	}
	return nil
}

func (s *unixSession) repositoryBinding() *directoryBinding {
	return &directoryBinding{
		file: s.repositoryDir, info: s.repositoryInfo,
		path: s.repositoryPath, source: s.repositorySource,
	}
}

func (s *unixSession) setRepository(binding *directoryBinding) {
	s.repositoryDir = binding.file
	s.repositoryInfo = binding.info
	s.repositoryPath = binding.path
	s.repositorySource = binding.source
}

func (s *unixSession) bindRepository(source string) error {
	if s.repositoryDir != nil {
		return newError(CodeConflict, "bind repository", "repository is already bound", nil)
	}
	if err := s.verifyBindings(); err != nil {
		return err
	}
	binding, err := bindRepository(source)
	if err != nil {
		return newError(CodeUnsafe, "bind repository", "repository root rejected", err)
	}
	if err := requireRootsDisjoint(s.authorityPath, s.authorityInfo, binding); err != nil {
		_ = binding.file.Close()
		return newError(CodeUnsafe, "bind repository", "authority and repository overlap", err)
	}
	s.setRepository(binding)
	if err := s.verifyBoundRepository(); err != nil {
		_ = s.repositoryDir.Close()
		s.clearRepository()
		return err
	}
	if err := s.verifyBindings(); err != nil {
		_ = s.repositoryDir.Close()
		s.clearRepository()
		return err
	}
	return nil
}

func (s *unixSession) duplicateRepositoryRoot() (*os.File, error) {
	if err := s.verifyRepository(); err != nil {
		return nil, err
	}
	fd, err := syscall.Dup(int(s.repositoryDir.Fd()))
	if err != nil {
		return nil, newError(CodeUnsafe, "duplicate repository", "duplicate directory handle", err)
	}
	syscall.CloseOnExec(fd)
	file := os.NewFile(uintptr(fd), "bound-repository-root")
	if file == nil {
		_ = syscall.Close(fd)
		return nil, newError(CodeUnsafe, "duplicate repository", "construct directory handle", nil)
	}
	if err := s.verifyRepository(); err != nil {
		_ = file.Close()
		return nil, err
	}
	return file, nil
}

func (s *unixSession) verifyRepository() error {
	if s.repositoryDir == nil {
		return newError(CodeInvalid, "verify repository", "repository is not bound", nil)
	}
	return s.verifyBoundRepository()
}

func (s *unixSession) clearRepository() {
	s.repositoryDir = nil
	s.repositoryInfo = nil
	s.repositoryPath = ""
	s.repositorySource = ""
}

func (s *unixSession) verifyRepositoryBinding() error {
	return verifyDirectoryBinding(s.repositoryBinding())
}

func (s *unixSession) verifyAuthorityBinding() error {
	current, err := os.Lstat(s.authorityPath)
	if err != nil || current.Mode()&os.ModeSymlink != 0 || !os.SameFile(s.authorityInfo, current) {
		return fmt.Errorf("authority path no longer names the bound root")
	}
	opened, err := s.authority.Lstat(".")
	if err != nil || !os.SameFile(s.authorityInfo, opened) {
		return fmt.Errorf("authority root handle lost identity")
	}
	dirInfo, err := s.authorityDir.Stat()
	if err != nil || !os.SameFile(opened, dirInfo) {
		return fmt.Errorf("authority directory handle lost identity")
	}
	return requireOwnedDirectory(opened, 0o700, "authority root")
}

func (s *unixSession) verifyStateBinding() error {
	current, currentDir, info, err := openRelativeDirectory(s.authority, s.stateDir)
	if err != nil {
		return err
	}
	defer func() { _ = current.Close() }()
	defer func() { _ = currentDir.Close() }()
	opened, openErr := s.state.Lstat(".")
	if openErr != nil || !os.SameFile(s.stateInfo, opened) || !os.SameFile(opened, info) {
		return fmt.Errorf("state directory no longer names the bound directory")
	}
	dirInfo, err := s.stateDirFile.Stat()
	if err != nil || !os.SameFile(opened, dirInfo) {
		return fmt.Errorf("state directory handle lost identity")
	}
	return requireOwnedDirectory(opened, 0o700, "state directory")
}

func (s *unixSession) verifyLockBinding() error {
	opened, err := validateOpenStateLeaf(s.lock, 0o600, "lock file")
	if err != nil || !os.SameFile(s.lockInfo, opened) {
		return fmt.Errorf("held lock lost identity")
	}
	return verifyNamedIdentity(s.state, s.layout.lock, s.lockInfo, 0o600)
}
