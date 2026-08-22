//go:build unix && !aix && !solaris

package grantstate

import "syscall"

func makeFIFO(path string, mode uint32) error {
	return syscall.Mkfifo(path, mode)
}
