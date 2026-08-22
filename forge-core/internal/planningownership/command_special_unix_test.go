//go:build linux || darwin || freebsd || openbsd || netbsd || dragonfly

package planningownership

import "syscall"

func createSpecialTestFile(path string) error { return syscall.Mkfifo(path, 0o600) }
