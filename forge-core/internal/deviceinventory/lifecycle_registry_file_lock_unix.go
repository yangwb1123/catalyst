//go:build unix && !aix && !solaris

package deviceinventory

import (
	"os"
	"path/filepath"
	"syscall"

	"forgeos/forge-core/internal/statefs"
)

// lifecycleRegistryFileLock is a candidate-only inter-process lock.  The
// lock file is intentionally retained after release so a process cannot
// replace it between two CAS operations and bypass flock ownership.
type lifecycleRegistryFileLock struct {
	file *os.File
}

func acquireLifecycleRegistryFileLock(path string) (lifecycleRegistryFileLock, error) {
	lockPath := filepath.Clean(path) + ".lock"
	file, err := statefs.OpenRegular(lockPath, os.O_RDWR|os.O_CREATE, 0o600)
	if err != nil {
		return lifecycleRegistryFileLock{}, err
	}
	if err := syscall.Flock(int(file.Fd()), syscall.LOCK_EX); err != nil {
		_ = file.Close()
		return lifecycleRegistryFileLock{}, err
	}
	return lifecycleRegistryFileLock{file: file}, nil
}

func (lock lifecycleRegistryFileLock) Close() error {
	if lock.file == nil {
		return nil
	}
	unlockErr := syscall.Flock(int(lock.file.Fd()), syscall.LOCK_UN)
	closeErr := lock.file.Close()
	if unlockErr != nil {
		return unlockErr
	}
	return closeErr
}
