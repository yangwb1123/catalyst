//go:build unix

package gitworktreesource

import (
	"fmt"
	"os"
	"syscall"
)

func requireSingleLinkSourceFile(path string, _ *os.File, info os.FileInfo) error {
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !ok {
		return fmt.Errorf("source path %q link count is unavailable", path)
	}
	if stat.Nlink != 1 {
		return fmt.Errorf("source path %q must be a single-link regular file", path)
	}
	return nil
}
