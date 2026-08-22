//go:build unix

package authenticatedadrapprovalauthority

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"syscall"
)

func (s *unixState) acquireLock() error {
	lock, created, err := s.openOrCreateLock()
	if err != nil {
		return fmt.Errorf("open stable approval lock: %w", err)
	}
	if created {
		if err := lock.Sync(); err != nil || s.stateDir.Sync() != nil {
			_ = lock.Close()
			return fmt.Errorf("persist stable approval lock")
		}
	}
	info, err := validateOpenLeaf(lock, privateMode, "approval lock")
	if err != nil {
		_ = lock.Close()
		return err
	}
	if err := lockExclusive(lock); err != nil {
		_ = lock.Close()
		if errors.Is(err, syscall.EWOULDBLOCK) || errors.Is(err, syscall.EAGAIN) ||
			errors.Is(err, syscall.EACCES) {
			return fmt.Errorf("%w: another authority holds the lock", errStateBusy)
		}
		return err
	}
	s.lock, s.lockInfo = lock, info
	return s.verifyBindings()
}

func (s *unixState) openOrCreateLock() (*os.File, bool, error) {
	lock, err := openExistingRegular(s.state, stateLockFile, true)
	if err == nil {
		return lock, false, nil
	}
	if !errors.Is(err, fs.ErrNotExist) {
		return nil, false, err
	}
	lock, err = s.state.OpenFile(stateLockFile,
		os.O_RDWR|os.O_CREATE|os.O_EXCL|syscall.O_NONBLOCK, privateMode)
	if errors.Is(err, fs.ErrExist) {
		lock, err = openExistingRegular(s.state, stateLockFile, true)
		return lock, false, err
	}
	if err != nil {
		return nil, false, err
	}
	if err := lock.Chmod(privateMode); err != nil {
		_ = lock.Close()
		return nil, true, err
	}
	return lock, true, nil
}
