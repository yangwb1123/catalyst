//go:build !unix

package statefs

import "os"

func openFileNoFollow(path string, flag int, perm os.FileMode) (*os.File, error) {
	return os.OpenFile(path, flag, perm)
}

// FileInfo does not expose a portable link count. The pre/post Lstat identity
// checks still reject static symlinks and special files on non-Unix hosts.
func singleLink(os.FileInfo) bool {
	return true
}
