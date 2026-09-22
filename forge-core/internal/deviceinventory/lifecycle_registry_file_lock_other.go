//go:build !unix || aix || solaris

package deviceinventory

import (
	"errors"
)

// Unsupported hosts keep the candidate adapter buildable but refuse writes:
// silently dropping the inter-process CAS lock would make the persistence
// guarantee platform-dependent.
type lifecycleRegistryFileLock struct{}

var errLifecycleRegistryFileLockUnsupported = errors.New("lifecycle registry file CAS lock is unsupported on this host")

func acquireLifecycleRegistryFileLock(string) (lifecycleRegistryFileLock, error) {
	return lifecycleRegistryFileLock{}, errLifecycleRegistryFileLockUnsupported
}

func (lifecycleRegistryFileLock) Close() error { return nil }
