//go:build unix

package grantstate

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

type rootIdentityProbe struct {
	inspect func(string) (fs.FileInfo, error)
	same    func(fs.FileInfo, fs.FileInfo) bool
}

var filesystemIdentityProbe = rootIdentityProbe{
	inspect: inspectStableDirectory,
	same:    os.SameFile,
}

func bindRepository(repository string) (*directoryBinding, error) {
	absolute, err := filepath.Abs(repository)
	if err != nil {
		return nil, fmt.Errorf("resolve repository root: %w", err)
	}
	resolved, err := filepath.EvalSymlinks(absolute)
	if err != nil {
		return nil, fmt.Errorf("resolve repository root: %w", err)
	}
	binding, err := bindStableDirectory(filepath.Clean(resolved))
	if err != nil {
		return nil, fmt.Errorf("bind repository root: %w", err)
	}
	rechecked, err := filepath.EvalSymlinks(absolute)
	if err != nil || filepath.Clean(rechecked) != binding.path {
		_ = binding.file.Close()
		return nil, fmt.Errorf("repository root changed while resolving")
	}
	binding.source = absolute
	return binding, nil
}

func bindStableDirectory(path string) (*directoryBinding, error) {
	return bindStableDirectoryWith(path, nil)
}

func bindStableDirectoryWith(path string, beforeOpen func(string) error) (*directoryBinding, error) {
	before, err := os.Lstat(path)
	if err != nil || before.Mode()&os.ModeSymlink != 0 || !before.IsDir() {
		return nil, fmt.Errorf("path must name a real directory")
	}
	if beforeOpen != nil {
		if err := beforeOpen(path); err != nil {
			return nil, err
		}
	}
	file, err := openDirectoryNoFollow(path)
	if err != nil {
		return nil, err
	}
	opened, openErr := file.Stat()
	after, afterErr := os.Lstat(path)
	if openErr != nil || afterErr != nil || !opened.IsDir() || after.Mode()&os.ModeSymlink != 0 ||
		!after.IsDir() || !os.SameFile(before, opened) || !os.SameFile(opened, after) {
		_ = file.Close()
		return nil, fmt.Errorf("directory changed while binding")
	}
	return &directoryBinding{file: file, info: opened, path: path, source: path}, nil
}

func openDirectoryNoFollow(path string) (*os.File, error) {
	flags := os.O_RDONLY | syscall.O_NONBLOCK | syscall.O_DIRECTORY | syscall.O_NOFOLLOW
	return os.OpenFile(path, flags, 0)
}

func verifyDirectoryBinding(binding *directoryBinding) error {
	if binding == nil || binding.file == nil || binding.info == nil {
		return fmt.Errorf("directory binding is unavailable")
	}
	resolved, err := filepath.EvalSymlinks(binding.source)
	if err != nil || filepath.Clean(resolved) != binding.path {
		return fmt.Errorf("directory source no longer resolves to the bound path")
	}
	opened, openErr := binding.file.Stat()
	current, currentErr := os.Lstat(binding.path)
	if openErr != nil || currentErr != nil || current.Mode()&os.ModeSymlink != 0 ||
		!current.IsDir() || !os.SameFile(binding.info, opened) || !os.SameFile(opened, current) {
		return fmt.Errorf("directory path no longer names the bound identity")
	}
	return nil
}

func requireRootsDisjoint(authorityPath string, authorityInfo fs.FileInfo, repository *directoryBinding) error {
	if err := verifyDirectoryBinding(repository); err != nil {
		return err
	}
	overlap, err := rootsOverlapByIdentity(
		authorityPath, authorityInfo, repository.path, repository.info, filesystemIdentityProbe,
	)
	if err != nil {
		return err
	}
	if overlap {
		return fmt.Errorf("authority and repository roots overlap by file identity")
	}
	if err := verifyDirectoryBinding(repository); err != nil {
		return err
	}
	return nil
}

func rootsOverlapByIdentity(
	authorityPath string,
	authorityInfo fs.FileInfo,
	repositoryPath string,
	repositoryInfo fs.FileInfo,
	probe rootIdentityProbe,
) (bool, error) {
	authorityBelowRepository, err := ancestorIdentityAppears(authorityPath, repositoryInfo, probe)
	if err != nil || authorityBelowRepository {
		return authorityBelowRepository, err
	}
	return ancestorIdentityAppears(repositoryPath, authorityInfo, probe)
}

func ancestorIdentityAppears(path string, target fs.FileInfo, probe rootIdentityProbe) (bool, error) {
	for current := filepath.Clean(path); ; current = filepath.Dir(current) {
		info, err := probe.inspect(current)
		if err != nil {
			return false, fmt.Errorf("inspect directory identity: %w", err)
		}
		if probe.same(info, target) {
			return true, nil
		}
		parent := filepath.Dir(current)
		if parent == current {
			return false, nil
		}
	}
}

func inspectStableDirectory(path string) (fs.FileInfo, error) {
	binding, err := bindStableDirectory(path)
	if err != nil {
		return nil, err
	}
	defer func() { _ = binding.file.Close() }()
	if err := verifyDirectoryBinding(binding); err != nil {
		return nil, err
	}
	return binding.info, nil
}
