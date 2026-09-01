//go:build !linux || android

package appserver

import "os"

func instanceLockSupported() bool { return false }

func tryInstanceLock(*os.File) error { return nil }

func unlockInstance(*os.File) error { return nil }
