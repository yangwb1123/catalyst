//go:build aix || solaris

package authenticatedadrapprovalauthority

import (
	"os"
	"syscall"
)

func lockExclusive(file *os.File) error {
	lock := syscall.Flock_t{Type: syscall.F_WRLCK, Whence: 0, Start: 0, Len: 0}
	return syscall.FcntlFlock(file.Fd(), syscall.F_SETLK, &lock)
}

func unlock(file *os.File) error {
	lock := syscall.Flock_t{Type: syscall.F_UNLCK, Whence: 0, Start: 0, Len: 0}
	return syscall.FcntlFlock(file.Fd(), syscall.F_SETLK, &lock)
}
