// Package statefs provides fail-closed filesystem primitives for Forge's
// repository-local control state. Repository contents are untrusted input, so
// a pre-planted symlink or hard link must never turn a state write into an
// out-of-repository mutation.
package statefs

import (
	"bytes"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
)

// EnsurePrivateDir creates path as a private directory or verifies the existing
// leaf is a real directory. Mkdir (not MkdirAll) avoids accepting a symlink as
// the state-directory leaf; callers must provide an existing parent.
func EnsurePrivateDir(path string) error {
	err := os.Mkdir(path, 0o700)
	created := err == nil
	if err != nil && !errors.Is(err, fs.ErrExist) {
		return fmt.Errorf("statefs: create directory %s: %w", path, err)
	}
	before, err := os.Lstat(path)
	if err != nil {
		return fmt.Errorf("statefs: inspect directory %s: %w", path, err)
	}
	if before.Mode()&os.ModeSymlink != 0 || !before.IsDir() {
		return fmt.Errorf("statefs: %s must be a real directory", path)
	}
	if err := os.Chmod(path, 0o700); err != nil {
		return fmt.Errorf("statefs: secure directory %s: %w", path, err)
	}
	after, err := os.Lstat(path)
	if err != nil || !os.SameFile(before, after) || !after.IsDir() {
		return fmt.Errorf("statefs: directory %s changed while securing it", path)
	}
	if err := syncSecuredDirectory(path, created); err != nil {
		return fmt.Errorf("statefs: persist directory %s: %w", path, err)
	}
	return nil
}

// EnsurePrivateDirTree treats the nearest existing directory as the caller's
// trust anchor and creates only missing descendants beneath it. Security-
// sensitive callers use a direct <trusted-root>/.forge leaf, so
// EnsurePrivateDir still rejects that leaf when it is a symlink. Treating the
// existing ancestor as an anchor preserves intentional/macOS path aliases.
func EnsurePrivateDirTree(path string) error {
	path = filepath.Clean(path)
	var missing []string
	var anchor string
	for cursor := path; ; cursor = filepath.Dir(cursor) {
		_, err := os.Lstat(cursor)
		if err == nil {
			anchor, err = filepath.EvalSymlinks(cursor)
			if err != nil {
				return fmt.Errorf("statefs: resolve directory anchor %s: %w", cursor, err)
			}
			resolved, err := os.Stat(anchor)
			if err != nil || !resolved.IsDir() {
				return fmt.Errorf("statefs: directory anchor %s is not a directory", cursor)
			}
			break
		}
		if errors.Is(err, fs.ErrNotExist) {
			missing = append(missing, filepath.Base(cursor))
			parent := filepath.Dir(cursor)
			if parent == cursor {
				return fmt.Errorf("statefs: no existing directory anchor for %s", path)
			}
			continue
		}
		if err != nil {
			return fmt.Errorf("statefs: inspect directory anchor %s: %w", cursor, err)
		}
	}
	for i := len(missing) - 1; i >= 0; i-- {
		anchor = filepath.Join(anchor, missing[i])
		if err := EnsurePrivateDir(anchor); err != nil {
			return err
		}
	}
	return EnsurePrivateDir(path)
}

// InspectDir distinguishes a missing directory from an alias or non-directory.
func InspectDir(path string) (os.FileInfo, bool, error) {
	info, err := os.Lstat(path)
	if errors.Is(err, fs.ErrNotExist) {
		return nil, false, nil
	}
	if err != nil {
		return nil, false, fmt.Errorf("statefs: inspect directory %s: %w", path, err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return nil, false, fmt.Errorf("statefs: %s must be a real directory", path)
	}
	return info, true, nil
}

// InspectRegular accepts a missing leaf or a regular single-link file. It uses
// Lstat so a symlink is rejected rather than followed.
func InspectRegular(path string) (os.FileInfo, bool, error) {
	if _, present, err := InspectDir(filepath.Dir(path)); err != nil || !present {
		return nil, false, err
	}
	info, err := os.Lstat(path)
	if errors.Is(err, fs.ErrNotExist) {
		return nil, false, nil
	}
	if err != nil {
		return nil, false, fmt.Errorf("statefs: inspect %s: %w", path, err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
		return nil, false, fmt.Errorf("statefs: %s must be a regular non-symlink file", path)
	}
	if !singleLink(info) {
		return nil, false, fmt.Errorf("statefs: %s must be a single-link file", path)
	}
	return info, true, nil
}

// OpenRegular opens a state leaf without following a final symlink on
// supported hosts, then verifies identity, type, link count, and permissions
// before the caller can mutate it.
func OpenRegular(path string, flag int, perm os.FileMode) (*os.File, error) {
	if _, present, err := InspectDir(filepath.Dir(path)); err != nil || !present {
		if err == nil {
			err = fmt.Errorf("statefs: parent directory is missing")
		}
		return nil, err
	}
	before, present, err := InspectRegular(path)
	if err != nil {
		return nil, err
	}
	file, err := openFileNoFollow(path, flag, perm)
	if err != nil {
		return nil, fmt.Errorf("statefs: open %s: %w", path, err)
	}
	opened, err := file.Stat()
	if err != nil || !opened.Mode().IsRegular() || !singleLink(opened) {
		_ = file.Close()
		return nil, fmt.Errorf("statefs: opened leaf %s is not a regular single-link file", path)
	}
	if present && !os.SameFile(before, opened) {
		_ = file.Close()
		return nil, fmt.Errorf("statefs: %s changed while opening", path)
	}
	current, nowPresent, err := InspectRegular(path)
	if err != nil || !nowPresent || !os.SameFile(opened, current) {
		_ = file.Close()
		return nil, fmt.Errorf("statefs: %s identity changed while opening", path)
	}
	if err := file.Chmod(perm); err != nil {
		_ = file.Close()
		return nil, fmt.Errorf("statefs: secure %s: %w", path, err)
	}
	return file, nil
}

// OpenRegularReadOnly securely opens an existing state leaf without changing
// its bytes, timestamps, or permissions.
func OpenRegularReadOnly(path string) (*os.File, error) {
	before, present, err := InspectRegular(path)
	if err != nil {
		return nil, err
	}
	if !present {
		return nil, fmt.Errorf("statefs: state leaf %s is missing", path)
	}
	file, err := openFileNoFollow(path, os.O_RDONLY, 0)
	if err != nil {
		return nil, fmt.Errorf("statefs: open %s read-only: %w", path, err)
	}
	opened, err := file.Stat()
	if err != nil || !opened.Mode().IsRegular() || !singleLink(opened) ||
		!os.SameFile(before, opened) {
		_ = file.Close()
		return nil, fmt.Errorf("statefs: state leaf %s changed while opening", path)
	}
	current, stillPresent, err := InspectRegular(path)
	if err != nil || !stillPresent || !os.SameFile(opened, current) {
		_ = file.Close()
		return nil, fmt.Errorf("statefs: state leaf %s identity changed while opening", path)
	}
	return file, nil
}

// ReadRegular reads a secure leaf and distinguishes a missing file from an
// empty one. A positive maxBytes bounds state-file memory use.
func ReadRegular(path string, maxBytes int64) ([]byte, bool, error) {
	info, present, err := InspectRegular(path)
	if err != nil || !present {
		return nil, present, err
	}
	if maxBytes > 0 && info.Size() > maxBytes {
		return nil, true, fmt.Errorf("statefs: %s exceeds %d bytes", path, maxBytes)
	}
	file, err := OpenRegular(path, os.O_RDONLY, 0o600)
	if err != nil {
		return nil, true, err
	}
	defer func() { _ = file.Close() }()
	reader := io.Reader(file)
	if maxBytes > 0 {
		reader = io.LimitReader(file, maxBytes+1)
	}
	data, err := io.ReadAll(reader)
	if err != nil {
		return nil, true, fmt.Errorf("statefs: read %s: %w", path, err)
	}
	if maxBytes > 0 && int64(len(data)) > maxBytes {
		return nil, true, fmt.Errorf("statefs: %s exceeds %d bytes", path, maxBytes)
	}
	return data, true, nil
}

// ReadRegularUnmodified is the side-effect-free counterpart to ReadRegular.
// It retains the alias, identity, and size checks but never chmods the leaf.
func ReadRegularUnmodified(path string, maxBytes int64) ([]byte, bool, error) {
	info, present, err := InspectRegular(path)
	if err != nil || !present {
		return nil, present, err
	}
	if maxBytes > 0 && info.Size() > maxBytes {
		return nil, true, fmt.Errorf("statefs: %s exceeds %d bytes", path, maxBytes)
	}
	file, err := OpenRegularReadOnly(path)
	if err != nil {
		return nil, true, err
	}
	defer func() { _ = file.Close() }()
	reader := io.Reader(file)
	if maxBytes > 0 {
		reader = io.LimitReader(file, maxBytes+1)
	}
	data, err := io.ReadAll(reader)
	if err != nil {
		return nil, true, fmt.Errorf("statefs: read %s: %w", path, err)
	}
	if maxBytes > 0 && int64(len(data)) > maxBytes {
		return nil, true, fmt.Errorf("statefs: %s exceeds %d bytes", path, maxBytes)
	}
	current, stillPresent, err := InspectRegular(path)
	if err != nil || !stillPresent || !os.SameFile(info, current) {
		return nil, true, fmt.Errorf("statefs: %s changed while reading", path)
	}
	return data, true, nil
}

// AtomicWrite publishes data using an unpredictable O_EXCL sibling and rename,
// then fsyncs the containing directory before reporting success. Existing
// target aliases are rejected before any target mutation.
func AtomicWrite(path string, data []byte, perm os.FileMode) error {
	dir := filepath.Dir(path)
	if err := EnsurePrivateDir(dir); err != nil {
		return err
	}
	if _, _, err := InspectRegular(path); err != nil {
		return err
	}
	temp, err := os.CreateTemp(dir, "."+filepath.Base(path)+".tmp-*")
	if err != nil {
		return fmt.Errorf("statefs: create temporary file for %s: %w", path, err)
	}
	tempPath := temp.Name()
	defer func() { _ = os.Remove(tempPath) }()
	if err := writeTemp(temp, data, perm); err != nil {
		return fmt.Errorf("statefs: prepare %s: %w", path, err)
	}
	tempInfo, err := os.Lstat(tempPath)
	if err != nil || !tempInfo.Mode().IsRegular() || !singleLink(tempInfo) {
		return fmt.Errorf("statefs: temporary file for %s lost identity", path)
	}
	if _, _, err := InspectRegular(path); err != nil {
		return err
	}
	if err := os.Rename(tempPath, path); err != nil {
		return fmt.Errorf("statefs: commit %s: %w", path, err)
	}
	// The rename is not reported as a successful publication until its parent
	// directory entry is durable. Keeping this in the primitive gives callers
	// one unambiguous commit point instead of a live file followed by a separate
	// fallible SyncDir step.
	if err := SyncDir(dir); err != nil {
		return fmt.Errorf("statefs: sync directory for %s: %w", path, err)
	}
	published, present, err := InspectRegular(path)
	if err != nil || !present || !os.SameFile(tempInfo, published) {
		return fmt.Errorf("statefs: published file %s lost identity", path)
	}
	return nil
}

// ReadTracked reads a repository-tracked regular file without changing its
// permissions. Unlike private control-state reads, a tracked file may use any
// caller-selected permission bits. The direct parent and leaf must be real,
// and the leaf must have exactly one hard link.
func ReadTracked(path string, maxBytes int64) ([]byte, os.FileMode, bool, error) {
	info, present, err := InspectRegular(path)
	if err != nil || !present {
		return nil, 0, present, err
	}
	if maxBytes > 0 && info.Size() > maxBytes {
		return nil, 0, true, fmt.Errorf("statefs: %s exceeds %d bytes", path, maxBytes)
	}
	file, err := openFileNoFollow(path, os.O_RDONLY, 0)
	if err != nil {
		return nil, 0, true, fmt.Errorf("statefs: open tracked file %s: %w", path, err)
	}
	defer func() { _ = file.Close() }()
	opened, err := file.Stat()
	if err != nil || !opened.Mode().IsRegular() || !singleLink(opened) ||
		!os.SameFile(info, opened) {
		return nil, 0, true, fmt.Errorf("statefs: tracked file %s changed while opening", path)
	}
	reader := io.Reader(file)
	if maxBytes > 0 {
		reader = io.LimitReader(file, maxBytes+1)
	}
	data, err := io.ReadAll(reader)
	if err != nil {
		return nil, 0, true, fmt.Errorf("statefs: read tracked file %s: %w", path, err)
	}
	if maxBytes > 0 && int64(len(data)) > maxBytes {
		return nil, 0, true, fmt.Errorf("statefs: %s exceeds %d bytes", path, maxBytes)
	}
	current, stillPresent, err := InspectRegular(path)
	if err != nil || !stillPresent || !os.SameFile(opened, current) {
		return nil, 0, true, fmt.Errorf("statefs: tracked file %s changed while reading", path)
	}
	return data, opened.Mode().Perm(), true, nil
}

// AtomicWriteTrackedIfUnchanged atomically replaces or creates a tracked file
// only while its complete expected image remains current. This catches both
// rename-based edits and in-place content/permission edits during preparation.
func AtomicWriteTrackedIfUnchanged(
	path string,
	expectedData []byte,
	expectedMode os.FileMode,
	expectedPresent bool,
	data []byte,
	perm os.FileMode,
) error {
	dir := filepath.Dir(path)
	if _, present, err := InspectDir(dir); err != nil || !present {
		if err == nil {
			err = fmt.Errorf("statefs: tracked parent directory is missing")
		}
		return err
	}
	if err := requireTrackedSnapshot(
		path, expectedData, expectedMode, expectedPresent,
	); err != nil {
		return err
	}
	before, existed, err := InspectRegular(path)
	if err != nil {
		return err
	}
	temp, err := os.CreateTemp(dir, "."+filepath.Base(path)+".tmp-*")
	if err != nil {
		return fmt.Errorf("statefs: create tracked temporary file for %s: %w", path, err)
	}
	tempPath := temp.Name()
	defer func() { _ = os.Remove(tempPath) }()
	if err := writeTemp(temp, data, perm); err != nil {
		return fmt.Errorf("statefs: prepare tracked file %s: %w", path, err)
	}
	tempInfo, err := os.Lstat(tempPath)
	if err != nil || !tempInfo.Mode().IsRegular() || !singleLink(tempInfo) {
		return fmt.Errorf("statefs: tracked temporary file for %s lost identity", path)
	}
	current, present, err := InspectRegular(path)
	if err != nil || present != existed || (existed && !os.SameFile(before, current)) {
		return fmt.Errorf("statefs: tracked target %s changed before commit", path)
	}
	if err := requireTrackedSnapshot(
		path, expectedData, expectedMode, expectedPresent,
	); err != nil {
		return err
	}
	return commitTrackedTemp(dir, tempPath, path, tempInfo)
}

func commitTrackedTemp(
	dir, tempPath, path string,
	tempInfo os.FileInfo,
) error {
	if err := os.Rename(tempPath, path); err != nil {
		return fmt.Errorf("statefs: commit tracked file %s: %w", path, err)
	}
	if err := SyncDir(dir); err != nil {
		return fmt.Errorf("statefs: sync tracked directory for %s: %w", path, err)
	}
	published, present, err := InspectRegular(path)
	if err != nil || !present || !os.SameFile(tempInfo, published) {
		return fmt.Errorf("statefs: published tracked file %s lost identity", path)
	}
	return nil
}

func requireTrackedSnapshot(
	path string,
	expectedData []byte,
	expectedMode os.FileMode,
	expectedPresent bool,
) error {
	maxBytes := int64(len(expectedData)) + 1
	data, mode, present, err := ReadTracked(path, maxBytes)
	if err != nil {
		return fmt.Errorf("statefs: verify tracked target %s: %w", path, err)
	}
	if present != expectedPresent ||
		(present && (mode != expectedMode.Perm() || !bytes.Equal(data, expectedData))) {
		return fmt.Errorf("statefs: tracked target %s no longer matches expected image", path)
	}
	return nil
}

// SyncDir persists directory-entry changes on filesystems that require an
// explicit directory fsync after rename or removal.
func SyncDir(path string) error {
	info, present, err := InspectDir(path)
	if err != nil || !present {
		if err == nil {
			err = fmt.Errorf("statefs: directory is missing")
		}
		return err
	}
	dir, err := os.Open(path)
	if err != nil {
		return fmt.Errorf("statefs: open directory %s: %w", path, err)
	}
	defer func() { _ = dir.Close() }()
	opened, err := dir.Stat()
	if err != nil || !opened.IsDir() || !os.SameFile(info, opened) {
		return fmt.Errorf("statefs: directory %s changed while opening", path)
	}
	if err := dir.Sync(); err != nil {
		return fmt.Errorf("statefs: sync directory %s: %w", path, err)
	}
	return nil
}

func writeTemp(file *os.File, data []byte, perm os.FileMode) error {
	if err := file.Chmod(perm); err != nil {
		_ = file.Close()
		return err
	}
	if _, err := file.Write(data); err != nil {
		_ = file.Close()
		return err
	}
	if err := file.Sync(); err != nil {
		_ = file.Close()
		return err
	}
	return file.Close()
}

// RemoveRegular removes a state leaf only after rejecting aliases and special
// files. Missing leaves are already absent and therefore succeed.
func RemoveRegular(path string) error {
	_, present, err := InspectRegular(path)
	if err != nil || !present {
		return err
	}
	if err := os.Remove(path); err != nil {
		return fmt.Errorf("statefs: remove %s: %w", path, err)
	}
	return nil
}
