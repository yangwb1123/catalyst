//go:build windows

package gitworktreesource

import (
	"fmt"
	"os"
	"syscall"
)

func requireSingleLinkSourceFile(path string, file *os.File, _ os.FileInfo) error {
	if file == nil {
		return nil
	}
	var info syscall.ByHandleFileInformation
	if err := syscall.GetFileInformationByHandle(syscall.Handle(file.Fd()), &info); err != nil {
		return fmt.Errorf("source path %q link count is unavailable: %w", path, err)
	}
	if info.NumberOfLinks != 1 {
		return fmt.Errorf("source path %q must be a single-link regular file", path)
	}
	return nil
}
