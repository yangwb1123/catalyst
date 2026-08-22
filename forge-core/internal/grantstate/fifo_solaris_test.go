//go:build solaris

package grantstate

import "syscall"

func makeFIFO(path string, mode uint32) error {
	return syscall.Mknod(path, syscall.S_IFIFO|mode, 0)
}
