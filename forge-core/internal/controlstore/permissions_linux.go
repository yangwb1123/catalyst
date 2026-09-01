//go:build linux && !android

package controlstore

import (
	"fmt"
	"os"
	"syscall"
)

func validatePrivateDatabaseFile(info os.FileInfo) error {
	special := os.ModeSetuid | os.ModeSetgid | os.ModeSticky
	if info == nil || !info.Mode().IsRegular() || info.Mode().Perm() != 0o600 || info.Mode()&special != 0 {
		return fmt.Errorf("database file must have exact mode 0600")
	}
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !ok || int(stat.Uid) != os.Geteuid() || stat.Nlink != 1 {
		return fmt.Errorf("database file must be effective-user-owned and single-link")
	}
	return nil
}
