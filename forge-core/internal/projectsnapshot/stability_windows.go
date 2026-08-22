//go:build windows

package projectsnapshot

import (
	"fmt"
	"os"
	"syscall"
)

func stableChangeIdentity(_, _ os.FileInfo) bool { return true }

func requireSingleLink(file *os.File, _ os.FileInfo) error {
	if file == nil {
		return nil
	}
	var facts syscall.ByHandleFileInformation
	if err := syscall.GetFileInformationByHandle(syscall.Handle(file.Fd()), &facts); err != nil || facts.NumberOfLinks != 1 {
		return fmt.Errorf("project source regular file link count is unavailable or not one")
	}
	return nil
}
