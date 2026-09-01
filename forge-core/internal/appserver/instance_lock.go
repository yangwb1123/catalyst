package appserver

import (
	"errors"
	"fmt"
	"os"
)

// ErrAlreadyRunning reports a live holder for the same state directory.
var ErrAlreadyRunning = errors.New("forge-server is already running")

type instanceLock struct {
	file *os.File
	root *os.Root
}

func acquireInstanceLock(stateDir string) (*instanceLock, error) {
	if !instanceLockSupported() {
		return nil, fmt.Errorf("forge-server single-instance lock is unsupported on this platform")
	}
	root, err := openAppState(stateDir)
	if err != nil {
		return nil, err
	}
	file, err := openStateLock(root)
	if err != nil {
		_ = root.Close()
		return nil, err
	}
	if err := tryInstanceLock(file); err != nil {
		_ = file.Close()
		_ = root.Close()
		if errors.Is(err, ErrAlreadyRunning) {
			return nil, fmt.Errorf("%w for state directory %s", err, stateDir)
		}
		return nil, fmt.Errorf("acquire instance lock: %w", err)
	}
	return &instanceLock{file: file, root: root}, nil
}

func (lock *instanceLock) Close() error {
	if lock == nil || lock.file == nil {
		return nil
	}
	file := lock.file
	lock.file = nil
	_ = unlockInstance(file)
	fileErr := file.Close()
	rootErr := lock.root.Close()
	lock.root = nil
	if fileErr != nil {
		return fileErr
	}
	return rootErr
}
