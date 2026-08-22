//go:build unix

package authenticatedadrapprovalauthority

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"syscall"
)

const specialModeBits = os.ModeSetuid | os.ModeSetgid | os.ModeSticky

func requireOwnedDirectory(info fs.FileInfo, mode fs.FileMode, label string) error {
	if info == nil || !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("%s must be a real directory", label)
	}
	if info.Mode().Perm() != mode.Perm() || info.Mode()&specialModeBits != 0 {
		return fmt.Errorf("%s must have mode %04o", label, mode.Perm())
	}
	return requireOwner(info, label)
}

func requireOwnedRegular(info fs.FileInfo, mode fs.FileMode, label string) error {
	if info == nil || !info.Mode().IsRegular() || info.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("%s must be a real regular file", label)
	}
	if info.Mode().Perm() != mode.Perm() || info.Mode()&specialModeBits != 0 {
		return fmt.Errorf("%s must have mode %04o", label, mode.Perm())
	}
	if err := requireOwner(info, label); err != nil {
		return err
	}
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !ok || stat.Nlink != 1 {
		return fmt.Errorf("%s must have exactly one link", label)
	}
	return nil
}

func requireOwner(info fs.FileInfo, label string) error {
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !ok || int(stat.Uid) != os.Geteuid() {
		return fmt.Errorf("%s must be owned by the effective user", label)
	}
	return nil
}

func inspectNoSymlinkAbsolute(value string) (fs.FileInfo, error) {
	separator := string(filepath.Separator)
	current, err := os.OpenRoot(separator)
	if err != nil {
		return nil, err
	}
	var final fs.FileInfo
	for _, component := range strings.Split(strings.TrimPrefix(value, separator), separator) {
		if component == "" {
			continue
		}
		next, info, openErr := openChildDirectory(current, component)
		_ = current.Close()
		current = next
		if openErr != nil {
			return nil, fmt.Errorf("inspect path component: %w", openErr)
		}
		final = info
	}
	if current == nil {
		return nil, fmt.Errorf("inspect path component failed")
	}
	defer func() { _ = current.Close() }()
	if final == nil {
		info, err := current.Lstat(".")
		if err != nil {
			return nil, fmt.Errorf("inspect path component: %w", err)
		}
		final = info
	}
	return final, nil
}

func sameStableFile(first, second fs.FileInfo) bool {
	return first != nil && second != nil && os.SameFile(first, second) &&
		first.Mode() == second.Mode() && first.Size() == second.Size() &&
		first.ModTime().Equal(second.ModTime())
}
