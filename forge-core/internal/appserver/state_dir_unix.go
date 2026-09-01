//go:build unix

package appserver

import (
	"bytes"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"syscall"
)

const (
	stateIdentityName = "app-server.identity"
	stateLockName     = "server.lock"
	controlDBName     = "control.db"
)

var controlStateNames = map[string]struct{}{
	controlDBName: {}, "control.db-journal": {}, "control.db-shm": {}, "control.db-wal": {},
}

var (
	stateIdentity = []byte("forgeos.app-server-state/v1\n")
	stateLock     = []byte("forgeos.app-server-lock/v1\n")
)

func openAppState(path string) (*os.Root, error) {
	parent, name, err := openTrustedParent(path)
	if err != nil {
		return nil, fmt.Errorf("bind state directory parent: %w", err)
	}
	defer func() { _ = parent.Close() }()
	info, err := parent.Lstat(name)
	if errors.Is(err, fs.ErrNotExist) {
		return createAppState(parent, name)
	}
	if err != nil {
		return nil, fmt.Errorf("inspect state directory: %w", err)
	}
	root, opened, err := openBoundChild(parent, name, info)
	if err != nil {
		return nil, fmt.Errorf("bind state directory: %w", err)
	}
	if err := verifyExistingAppState(root, opened); err != nil {
		_ = root.Close()
		return nil, err
	}
	return root, nil
}

func openTrustedParent(path string) (*os.Root, string, error) {
	clean := filepath.Clean(path)
	parentPath := filepath.Dir(clean)
	before, err := inspectRealDirectoryPath(parentPath)
	if err != nil || requireStateAnchor(before) != nil {
		return nil, "", fmt.Errorf("state parent must be an effective-user-owned private directory")
	}
	root, err := os.OpenRoot(parentPath)
	if err != nil {
		return nil, "", err
	}
	opened, openErr := statRootOrClose(root)
	if openErr != nil {
		return nil, "", fmt.Errorf("state parent changed while binding")
	}
	after, afterErr := inspectRealDirectoryPath(parentPath)
	if afterErr != nil || validateBoundStateAnchor(before, opened, after) != nil {
		_ = root.Close()
		return nil, "", fmt.Errorf("state parent changed while binding")
	}
	return root, filepath.Base(clean), nil
}

func validateBoundStateAnchor(before, opened, after fs.FileInfo) error {
	return validateBoundStateAnchorWith(before, opened, after, os.SameFile)
}

func validateBoundStateAnchorWith(
	before, opened, after fs.FileInfo,
	same func(fs.FileInfo, fs.FileInfo) bool,
) error {
	for _, info := range []fs.FileInfo{before, opened, after} {
		if err := requireStateAnchor(info); err != nil {
			return err
		}
	}
	if !same(before, opened) || !same(opened, after) {
		return fmt.Errorf("state parent identity changed")
	}
	return nil
}

func inspectRealDirectoryPath(path string) (fs.FileInfo, error) {
	cursor := string(filepath.Separator)
	info, err := os.Lstat(cursor)
	if err != nil {
		return nil, err
	}
	if err := requirePathAncestor(info); err != nil {
		return nil, err
	}
	relative := strings.TrimPrefix(path, cursor)
	for _, component := range strings.Split(relative, string(filepath.Separator)) {
		if component == "" {
			continue
		}
		cursor = filepath.Join(cursor, component)
		info, err = os.Lstat(cursor)
		if err != nil || info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
			return nil, fmt.Errorf("path component %q must be a real directory", component)
		}
		if err := requirePathAncestor(info); err != nil {
			return nil, fmt.Errorf("untrusted path component %q: %w", component, err)
		}
	}
	return info, nil
}

func openBoundChild(parent *os.Root, name string, expected fs.FileInfo) (*os.Root, fs.FileInfo, error) {
	before, err := parent.Lstat(name)
	if err != nil || before.Mode()&os.ModeSymlink != 0 || !before.IsDir() {
		return nil, nil, fmt.Errorf("%s must be a real directory", name)
	}
	if expected != nil && !os.SameFile(expected, before) {
		return nil, nil, fmt.Errorf("%s changed before binding", name)
	}
	child, err := parent.OpenRoot(name)
	if err != nil {
		return nil, nil, err
	}
	opened, openErr := statRootOrClose(child)
	if openErr != nil {
		return nil, nil, fmt.Errorf("%s changed while binding", name)
	}
	after, afterErr := parent.Lstat(name)
	if afterErr != nil || !os.SameFile(before, opened) || !os.SameFile(opened, after) {
		_ = child.Close()
		return nil, nil, fmt.Errorf("%s changed while binding", name)
	}
	return child, opened, nil
}

func createAppState(parent *os.Root, name string) (*os.Root, error) {
	if err := parent.Mkdir(name, 0o700); err != nil {
		return nil, fmt.Errorf("create dedicated state directory: %w", err)
	}
	root, info, err := openBoundChild(parent, name, nil)
	if err != nil {
		return nil, fmt.Errorf("bind new state directory: %w", err)
	}
	if err := root.Chmod(".", 0o700); err != nil {
		_ = root.Close()
		return nil, fmt.Errorf("set new state directory mode: %w", err)
	}
	info, err = root.Stat(".")
	if err != nil || requireStateDirectory(info) != nil {
		_ = root.Close()
		return nil, fmt.Errorf("secure new state directory")
	}
	if err := createStateFile(root, stateLockName, stateLock); err != nil {
		_ = root.Close()
		return nil, err
	}
	if err := createStateFile(root, stateIdentityName, stateIdentity); err != nil {
		_ = root.Close()
		return nil, err
	}
	return root, nil
}

func verifyExistingAppState(root *os.Root, info fs.FileInfo) error {
	if err := requireStateDirectory(info); err != nil {
		return err
	}
	if err := verifyStateContents(root); err != nil {
		return err
	}
	if err := verifyStateFile(root, stateIdentityName, stateIdentity); err != nil {
		return err
	}
	return verifyStateFile(root, stateLockName, stateLock)
}

func verifyStateContents(root *os.Root) error {
	directory, err := root.Open(".")
	if err != nil {
		return fmt.Errorf("open state directory: %w", err)
	}
	defer func() { _ = directory.Close() }()
	entries, err := directory.ReadDir(7)
	if err != nil {
		return fmt.Errorf("read state directory: %w", err)
	}
	allowed := map[string]bool{stateIdentityName: false, stateLockName: false}
	hasDatabase := false
	for _, entry := range entries {
		name := entry.Name()
		if _, ok := controlStateNames[name]; ok {
			if err := verifyControlStateEntry(root, name); err != nil {
				return err
			}
			hasDatabase = hasDatabase || name == controlDBName
			continue
		}
		if _, ok := allowed[name]; !ok {
			return fmt.Errorf("state directory contains unknown entry %q", entry.Name())
		}
		allowed[name] = true
	}
	if !allowed[stateIdentityName] || !allowed[stateLockName] {
		return fmt.Errorf("state directory layout is incomplete")
	}
	if !hasDatabase && len(entries) != len(allowed) {
		return fmt.Errorf("control database sidecar has no database")
	}
	return nil
}

func verifyControlStateLayout(root *os.Root) error {
	return verifyStateContents(root)
}

func verifyControlStateEntry(root *os.Root, name string) error {
	file, err := openBoundStateFile(root, name, os.O_RDWR)
	if err != nil {
		return fmt.Errorf("control state file %s is not private: %w", name, err)
	}
	return file.Close()
}

func createStateFile(root *os.Root, name string, content []byte) error {
	file, err := root.OpenFile(name, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		return fmt.Errorf("create state identity %s: %w", name, err)
	}
	if err = file.Chmod(0o600); err == nil {
		var written int64
		written, err = io.Copy(file, bytes.NewReader(content))
		if err == nil && written != int64(len(content)) {
			err = io.ErrShortWrite
		}
	}
	if err == nil {
		err = file.Sync()
	}
	info, statErr := file.Stat()
	closeErr := file.Close()
	if err != nil || statErr != nil || closeErr != nil || requireStateFile(info) != nil {
		return fmt.Errorf("write private state identity %s", name)
	}
	current, currentErr := root.Lstat(name)
	if currentErr != nil || !os.SameFile(info, current) {
		return fmt.Errorf("state identity %s changed after creation", name)
	}
	return syncStateRoot(root)
}

func verifyStateFile(root *os.Root, name string, want []byte) error {
	file, err := openBoundStateFile(root, name, os.O_RDONLY)
	if err != nil {
		return err
	}
	defer func() { _ = file.Close() }()
	got, err := io.ReadAll(io.LimitReader(file, int64(len(want)+1)))
	if err != nil || !bytes.Equal(got, want) {
		return fmt.Errorf("state identity %s is invalid", name)
	}
	return nil
}

func openStateLock(root *os.Root) (*os.File, error) {
	file, err := openBoundStateFile(root, stateLockName, os.O_RDWR)
	if err != nil {
		return nil, fmt.Errorf("open instance lock: %w", err)
	}
	got, err := io.ReadAll(io.LimitReader(file, int64(len(stateLock)+1)))
	if err != nil || !bytes.Equal(got, stateLock) {
		_ = file.Close()
		return nil, fmt.Errorf("instance lock identity is invalid")
	}
	return file, nil
}

func openBoundStateFile(root *os.Root, name string, flag int) (*os.File, error) {
	before, err := root.Lstat(name)
	if err != nil || requireStateFile(before) != nil {
		return nil, fmt.Errorf("state file %s is not private", name)
	}
	file, err := root.OpenFile(name, flag, 0)
	if err != nil {
		return nil, err
	}
	opened, openErr := statFileOrClose(file)
	if openErr != nil {
		return nil, fmt.Errorf("state file %s changed while opening", name)
	}
	after, afterErr := root.Lstat(name)
	if afterErr != nil || requireStateFile(opened) != nil ||
		!os.SameFile(before, opened) || !os.SameFile(opened, after) {
		_ = file.Close()
		return nil, fmt.Errorf("state file %s changed while opening", name)
	}
	return file, nil
}

type rootStatCloser interface {
	Stat(string) (fs.FileInfo, error)
	Close() error
}

func statRootOrClose(root rootStatCloser) (fs.FileInfo, error) {
	info, err := root.Stat(".")
	if err != nil {
		_ = root.Close()
	}
	return info, err
}

type fileStatCloser interface {
	Stat() (fs.FileInfo, error)
	Close() error
}

func statFileOrClose(file fileStatCloser) (fs.FileInfo, error) {
	info, err := file.Stat()
	if err != nil {
		_ = file.Close()
	}
	return info, err
}

func requireStateAnchor(info fs.FileInfo) error {
	if info == nil || !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("state parent must be a real directory")
	}
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !ok || int(stat.Uid) != os.Geteuid() {
		return fmt.Errorf("state parent must be owned by the effective user")
	}
	if info.Mode().Perm()&0o022 != 0 {
		return fmt.Errorf("state parent must not be group or world writable")
	}
	return nil
}

func requirePathAncestor(info fs.FileInfo) error {
	if info == nil || !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("path ancestor must be a real directory")
	}
	stat, ok := info.Sys().(*syscall.Stat_t)
	owner := -1
	if ok {
		owner = int(stat.Uid)
	}
	if !ok || owner != 0 && owner != os.Geteuid() && !trustedUnmappedOwner(owner) {
		return fmt.Errorf("path ancestor owner is not trusted")
	}
	if info.Mode().Perm()&0o022 != 0 && info.Mode()&os.ModeSticky == 0 {
		return fmt.Errorf("path ancestor is replaceable by another user")
	}
	return nil
}

func requireStateDirectory(info fs.FileInfo) error {
	special := os.ModeSetuid | os.ModeSetgid | os.ModeSticky
	if info == nil || !info.IsDir() || info.Mode().Perm() != 0o700 || info.Mode()&special != 0 {
		return fmt.Errorf("state directory must have exact mode 0700")
	}
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !ok || int(stat.Uid) != os.Geteuid() {
		return fmt.Errorf("state directory must be owned by the effective user")
	}
	return nil
}

func requireStateFile(info fs.FileInfo) error {
	special := os.ModeSetuid | os.ModeSetgid | os.ModeSticky
	if info == nil || !info.Mode().IsRegular() || info.Mode().Perm() != 0o600 || info.Mode()&special != 0 {
		return fmt.Errorf("state file must be a regular file with exact mode 0600")
	}
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !ok || int(stat.Uid) != os.Geteuid() || stat.Nlink != 1 {
		return fmt.Errorf("state file must be effective-user-owned and single-link")
	}
	return nil
}

func syncStateRoot(root *os.Root) error {
	directory, err := root.Open(".")
	if err != nil {
		return err
	}
	defer func() { _ = directory.Close() }()
	return directory.Sync()
}
