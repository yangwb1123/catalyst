//go:build unix

package grantstate

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
		return fmt.Errorf("%s must be a regular file", label)
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

func inspectNoSymlinkAbsolute(path string) (fs.FileInfo, error) {
	root := string(filepath.Separator)
	relative := strings.TrimPrefix(path, root)
	cursor := root
	var final fs.FileInfo
	for _, component := range strings.Split(relative, string(filepath.Separator)) {
		if component == "" {
			continue
		}
		cursor = filepath.Join(cursor, component)
		info, err := os.Lstat(cursor)
		if err != nil {
			return nil, fmt.Errorf("inspect authority path component: %w", err)
		}
		if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
			return nil, fmt.Errorf("authority path has a symlink or non-directory ancestor")
		}
		final = info
	}
	if final == nil {
		final, _ = os.Lstat(root)
	}
	return final, nil
}

func sameStableFile(first, second fs.FileInfo) bool {
	if first == nil || second == nil || !os.SameFile(first, second) {
		return false
	}
	return first.Mode() == second.Mode() && first.Size() == second.Size() &&
		first.ModTime().Equal(second.ModTime())
}

func splitRelative(value string) []string {
	return strings.Split(value, "/")
}
