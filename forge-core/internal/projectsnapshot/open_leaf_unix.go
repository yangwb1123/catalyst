//go:build unix

package projectsnapshot

import (
	"os"
	"syscall"
)

func openRegularLeaf(root *os.Root, leaf string) (*os.File, error) {
	return root.OpenFile(leaf, os.O_RDONLY|syscall.O_NOFOLLOW|syscall.O_NONBLOCK, 0)
}
