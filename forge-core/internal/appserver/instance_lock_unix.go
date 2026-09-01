//go:build linux && !android

package appserver

import (
	"errors"
	"os"
	"syscall"
)

func instanceLockSupported() bool { return true }

func tryInstanceLock(file *os.File) error {
	err := syscall.Flock(int(file.Fd()), syscall.LOCK_EX|syscall.LOCK_NB)
	if err == nil {
		return nil
	}
	if errors.Is(err, syscall.EWOULDBLOCK) || errors.Is(err, syscall.EAGAIN) {
		return ErrAlreadyRunning
	}
	return err
}

func unlockInstance(file *os.File) error {
	return syscall.Flock(int(file.Fd()), syscall.LOCK_UN)
}
