//go:build unix

package authenticatedadrapprovalauthority

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"syscall"
)

type directoryBinding struct {
	file   *os.File
	info   fs.FileInfo
	path   string
	source string
}

func bindRepository(source string) (*directoryBinding, error) {
	inspected, err := inspectNoSymlinkAbsolute(source)
	if err != nil || inspected == nil || !inspected.IsDir() {
		return nil, fmt.Errorf("repository root has a symlink or non-directory ancestor")
	}
	resolved, err := filepath.EvalSymlinks(source)
	if err != nil || filepath.Clean(resolved) != source {
		return nil, fmt.Errorf("repository root must resolve to itself")
	}
	binding, err := bindStableDirectory(source)
	if err != nil {
		return nil, err
	}
	rechecked, err := filepath.EvalSymlinks(source)
	if err != nil || filepath.Clean(rechecked) != binding.path {
		_ = binding.file.Close()
		return nil, fmt.Errorf("repository root changed while resolving")
	}
	binding.source = source
	return binding, nil
}

func bindStableDirectory(value string) (*directoryBinding, error) {
	before, err := os.Lstat(value)
	if err != nil || before.Mode()&os.ModeSymlink != 0 || !before.IsDir() {
		return nil, fmt.Errorf("path must name a real directory")
	}
	file, err := os.OpenFile(value,
		os.O_RDONLY|syscall.O_NONBLOCK|syscall.O_DIRECTORY|syscall.O_NOFOLLOW, 0)
	if err != nil {
		return nil, err
	}
	opened, openErr := file.Stat()
	after, afterErr := os.Lstat(value)
	if openErr != nil || afterErr != nil || !os.SameFile(before, opened) ||
		!os.SameFile(opened, after) {
		_ = file.Close()
		return nil, fmt.Errorf("directory changed while binding")
	}
	return &directoryBinding{file: file, info: opened, path: value, source: value}, nil
}

func verifyDirectoryBinding(binding *directoryBinding) error {
	if binding == nil || binding.file == nil || binding.info == nil {
		return fmt.Errorf("directory binding is unavailable")
	}
	if err := verifyAbsoluteDirectoryPath(binding.source, binding.info,
		"directory binding"); err != nil {
		return err
	}
	opened, openErr := binding.file.Stat()
	current, currentErr := os.Lstat(binding.path)
	if openErr != nil || currentErr != nil || current.Mode()&os.ModeSymlink != 0 ||
		!os.SameFile(binding.info, opened) || !os.SameFile(opened, current) {
		return fmt.Errorf("directory binding changed")
	}
	return nil
}

func verifyAbsoluteDirectoryPath(value string, expected fs.FileInfo,
	label string) error {
	inspected, err := inspectNoSymlinkAbsolute(value)
	if err != nil || inspected == nil || !os.SameFile(expected, inspected) {
		return fmt.Errorf("%s path identity or ancestry changed", label)
	}
	resolved, err := filepath.EvalSymlinks(value)
	if err != nil || filepath.Clean(resolved) != value {
		return fmt.Errorf("%s path acquired a symlink", label)
	}
	return nil
}

func rootsOverlap(authorityPath string, authorityInfo fs.FileInfo,
	repository *directoryBinding) (bool, error) {
	if err := verifyDirectoryBinding(repository); err != nil {
		return false, err
	}
	if found, err := ancestorHasIdentity(authorityPath, repository.info); err != nil || found {
		return found, err
	}
	return ancestorHasIdentity(repository.path, authorityInfo)
}

func ancestorHasIdentity(value string, target fs.FileInfo) (bool, error) {
	for current := filepath.Clean(value); ; current = filepath.Dir(current) {
		binding, err := bindStableDirectory(current)
		if err != nil {
			return false, err
		}
		matches := os.SameFile(binding.info, target)
		_ = binding.file.Close()
		if matches {
			return true, nil
		}
		if parent := filepath.Dir(current); parent == current {
			return false, nil
		}
	}
}
