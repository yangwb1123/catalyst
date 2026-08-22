//go:build unix

package authenticatedadrlifecycleauthority

import (
	"errors"
	"fmt"
	"syscall"
)

func (s *unixState) acquireLock() error {
	lock, err := openExistingRegular(s.state, lockFile, true)
	if err != nil {
		return fmt.Errorf("open lifecycle lock: %w", err)
	}
	info, err := validateOpenLeaf(lock, privateMode, "lifecycle lock")
	if err != nil {
		_ = lock.Close()
		return err
	}
	if err = lockExclusive(lock); err != nil {
		_ = lock.Close()
		if errors.Is(err, syscall.EWOULDBLOCK) || errors.Is(err, syscall.EAGAIN) || errors.Is(err, syscall.EACCES) {
			return fmt.Errorf("%w: another authority holds the lock", errStateBusy)
		}
		return err
	}
	s.lock, s.lockInfo = lock, info
	return s.verifyBindings()
}
