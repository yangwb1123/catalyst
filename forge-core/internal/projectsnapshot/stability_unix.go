//go:build linux

package projectsnapshot

import (
	"fmt"
	"os"
	"syscall"
)

func stableChangeIdentity(expected, current os.FileInfo) bool {
	first, firstOK := expected.Sys().(*syscall.Stat_t)
	second, secondOK := current.Sys().(*syscall.Stat_t)
	return firstOK && secondOK && first.Ctim == second.Ctim
}

func requireSingleLink(_ *os.File, info os.FileInfo) error {
	facts, ok := info.Sys().(*syscall.Stat_t)
	if !ok || facts.Nlink != 1 {
		return fmt.Errorf("project source regular file must have exactly one link")
	}
	return nil
}
