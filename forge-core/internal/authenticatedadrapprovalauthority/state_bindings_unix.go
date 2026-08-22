//go:build unix

package authenticatedadrapprovalauthority

import (
	"fmt"
	"os"
)

func (s *unixState) verifyBindings() error {
	if s == nil || s.lock == nil {
		return fmt.Errorf("approval state is closed")
	}
	if err := s.verifyAuthority(); err != nil {
		return err
	}
	if err := s.verifyAuthorityLeaves(); err != nil {
		return err
	}
	if err := s.verifyRepository(); err != nil {
		return err
	}
	if err := s.verifyState(); err != nil {
		return err
	}
	return s.verifyLock()
}

func (s *unixState) verifyAuthority() error {
	if err := verifyAbsoluteDirectoryPath(s.authorityPath, s.authorityInfo,
		"authority root"); err != nil {
		return err
	}
	current, err := os.Lstat(s.authorityPath)
	opened, openErr := s.authority.Lstat(".")
	dirInfo, dirErr := s.authorityDir.Stat()
	if err != nil || openErr != nil || dirErr != nil || current.Mode()&os.ModeSymlink != 0 ||
		!os.SameFile(s.authorityInfo, current) || !os.SameFile(current, opened) ||
		!os.SameFile(opened, dirInfo) {
		return fmt.Errorf("authority root binding changed")
	}
	return requireOwnedDirectory(opened, privateDirMode, "authority root")
}

func (s *unixState) verifyRepository() error {
	if err := verifyDirectoryBinding(s.repository); err != nil {
		return fmt.Errorf("repository root binding changed: %w", err)
	}
	overlap, err := rootsOverlap(s.authorityPath, s.authorityInfo, s.repository)
	if err != nil || overlap {
		return fmt.Errorf("authority and repository roots overlap or changed")
	}
	return nil
}

func (s *unixState) verifyState() error {
	root, dir, current, err := openRelativeDirectory(s.authority, s.stateRelative)
	if err != nil {
		return err
	}
	defer func() { _ = root.Close() }()
	defer func() { _ = dir.Close() }()
	opened, openErr := s.state.Lstat(".")
	dirInfo, dirErr := s.stateDir.Stat()
	if openErr != nil || dirErr != nil || !os.SameFile(s.stateInfo, current) ||
		!os.SameFile(current, opened) || !os.SameFile(opened, dirInfo) {
		return fmt.Errorf("approval state directory binding changed")
	}
	return requireOwnedDirectory(opened, privateDirMode, "approval state directory")
}

func (s *unixState) verifyLock() error {
	opened, err := validateOpenLeaf(s.lock, privateMode, "approval lock")
	if err != nil || !os.SameFile(s.lockInfo, opened) {
		return fmt.Errorf("approval lock binding changed")
	}
	return verifyNamedIdentity(s.state, stateLockFile, s.lockInfo, privateMode)
}
