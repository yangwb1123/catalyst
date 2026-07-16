//go:build unix

package runlock

import (
	"errors"
	"os"
	"syscall"
)

// tryLock claims an exclusive, non-blocking flock(2) on f. flock is tied to
// the OPEN FILE DESCRIPTION, not the process or the fd number, so the kernel
// releases it automatically on ANY holder process exit — clean, signal-
// terminated, or even SIGKILLed — with no separate crash-recovery path
// needed.
//
// LOCK_EX|LOCK_NB never blocks: if another open file description already
// holds the exclusive lock, flock returns EWOULDBLOCK immediately, which
// this maps to errLockHeld so Acquire can render its actionable message.
func tryLock(f *os.File) error {
	err := syscall.Flock(int(f.Fd()), syscall.LOCK_EX|syscall.LOCK_NB)
	if err == nil {
		return nil
	}
	if errors.Is(err, syscall.EWOULDBLOCK) {
		return errLockHeld
	}
	return err
}

// unlock releases f's flock. Best-effort: called only from Release, which
// already tolerates any error from it.
func unlock(f *os.File) error {
	return syscall.Flock(int(f.Fd()), syscall.LOCK_UN)
}
