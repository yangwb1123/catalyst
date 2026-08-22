//go:build unix && !linux

package projectsnapshot

import (
	"fmt"
	"os"
	"syscall"
)

func stableChangeIdentity(_, _ os.FileInfo) bool { return true }

func requireSingleLink(_ *os.File, info os.FileInfo) error {
	facts, ok := info.Sys().(*syscall.Stat_t)
	if !ok || facts.Nlink != 1 {
		return fmt.Errorf("project source regular file must have exactly one link")
	}
	return nil
}
