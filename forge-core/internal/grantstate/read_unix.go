//go:build unix

package grantstate

import (
	"bytes"
	"fmt"
	"io"
	"io/fs"
	"os"
	"syscall"
)

func (s *unixSession) current() (Snapshot, error) {
	if err := s.verifyBindings(); err != nil {
		return Snapshot{}, err
	}
	data, present, err := readStateLeaf(s.state, s.layout.ledger, s.maxBytes)
	if err != nil {
		return Snapshot{}, newError(CodeUnsafe, "read ledger", "ledger rejected", err)
	}
	return Snapshot{Data: data, Present: present}, nil
}

func (s *unixSession) readLeaf(relative string, max int64, mode fs.FileMode) ([]byte, error) {
	if err := validateRelative(relative, "authority leaf"); err != nil {
		return nil, newError(CodeInvalid, "read leaf", err.Error(), nil)
	}
	if max <= 0 || max > AbsoluteMaxBytes {
		return nil, newError(CodeInvalid, "read leaf", "invalid byte limit", nil)
	}
	if mode.Perm() != mode || mode == 0 {
		return nil, newError(CodeInvalid, "read leaf", "required mode must be exact permission bits", nil)
	}
	if err := s.verifyBindings(); err != nil {
		return nil, err
	}
	data, err := readAuthorityLeaf(s.authority, relative, max, mode)
	if err != nil {
		return nil, newError(CodeUnsafe, "read leaf", "authority leaf rejected", err)
	}
	if err := s.verifyBindings(); err != nil {
		return discardBytes(data), err
	}
	return data, nil
}

func readAuthorityLeaf(root *os.Root, relative string, max int64, mode fs.FileMode) ([]byte, error) {
	components := splitRelative(relative)
	parent, closeParent, err := openLeafParent(root, components[:len(components)-1])
	if err != nil {
		return nil, err
	}
	if closeParent {
		defer func() { _ = parent.Close() }()
	}
	return readRequiredLeaf(parent, components[len(components)-1], max, mode)
}

func openLeafParent(root *os.Root, components []string) (*os.Root, bool, error) {
	current := root
	owned := false
	for _, component := range components {
		next, info, err := openChildDirectory(current, component)
		if owned {
			_ = current.Close()
		}
		if err != nil {
			return nil, false, err
		}
		if requireOwner(info, "authority leaf parent") != nil {
			_ = next.Close()
			return nil, false, fmt.Errorf("authority leaf parent rejected")
		}
		current, owned = next, true
	}
	return current, owned, nil
}

func readRequiredLeaf(parent *os.Root, name string, max int64, mode fs.FileMode) ([]byte, error) {
	file, err := openExistingRegular(parent, name, false)
	if err != nil {
		return nil, err
	}
	defer func() { _ = file.Close() }()
	before, err := validateOpenStateLeaf(file, mode, "authority leaf")
	if err != nil {
		return nil, err
	}
	data, err := readBounded(file, before, max)
	if err != nil {
		return nil, err
	}
	if err := verifyNamedIdentity(parent, name, before, mode); err != nil {
		return discardBytes(data), err
	}
	return data, nil
}

func readStateLeaf(parent *os.Root, name string, max int64) ([]byte, bool, error) {
	file, err := openExistingRegular(parent, name, false)
	if isMissing(err) {
		return nil, false, nil
	}
	if err != nil {
		return nil, false, err
	}
	defer func() { _ = file.Close() }()
	before, err := validateOpenStateLeaf(file, 0o600, "state leaf")
	if err != nil {
		return nil, true, err
	}
	data, err := readBounded(file, before, max)
	if err != nil {
		return nil, true, err
	}
	if err := verifyNamedIdentity(parent, name, before, 0o600); err != nil {
		return discardBytes(data), true, err
	}
	return data, true, nil
}

func readBounded(file *os.File, before fs.FileInfo, max int64) ([]byte, error) {
	if before.Size() > max {
		return nil, fmt.Errorf("leaf exceeds %d bytes", max)
	}
	data, err := io.ReadAll(io.LimitReader(file, max+1))
	if err != nil {
		return discardBytes(data), err
	}
	if int64(len(data)) > max {
		return discardBytes(data), fmt.Errorf("leaf exceeds %d bytes", max)
	}
	after, err := file.Stat()
	if err != nil || !sameStableFile(before, after) {
		return discardBytes(data), fmt.Errorf("leaf changed while reading")
	}
	return data, nil
}

func validateOpenStateLeaf(file *os.File, mode fs.FileMode, label string) (fs.FileInfo, error) {
	info, err := file.Stat()
	if err != nil {
		return nil, err
	}
	if err := requireOwnedRegular(info, mode, label); err != nil {
		return nil, err
	}
	return info, nil
}

func verifyNamedIdentity(parent *os.Root, name string, expected fs.FileInfo, mode fs.FileMode) error {
	current, err := openExistingRegular(parent, name, false)
	if err != nil {
		return err
	}
	defer func() { _ = current.Close() }()
	info, err := validateOpenStateLeaf(current, mode, "bound leaf")
	if err != nil {
		return err
	}
	if !os.SameFile(expected, info) {
		return fmt.Errorf("leaf identity changed")
	}
	return nil
}

func openExistingRegular(parent *os.Root, name string, write bool) (*os.File, error) {
	before, err := parent.Lstat(name)
	if err != nil {
		return nil, err
	}
	if before.Mode()&os.ModeSymlink != 0 || !before.Mode().IsRegular() {
		return nil, fmt.Errorf("leaf must be a real regular file")
	}
	flags := os.O_RDONLY | syscall.O_NONBLOCK
	if write {
		flags = os.O_RDWR | syscall.O_NONBLOCK
	}
	file, err := parent.OpenFile(name, flags, 0)
	if err != nil {
		return nil, err
	}
	opened, openErr := file.Stat()
	after, afterErr := parent.Lstat(name)
	if openErr != nil || afterErr != nil || !os.SameFile(before, opened) || !os.SameFile(opened, after) {
		_ = file.Close()
		return nil, fmt.Errorf("leaf changed while opening")
	}
	return file, nil
}

func snapshotsEqual(first, second Snapshot) bool {
	return first.Present == second.Present && bytes.Equal(first.Data, second.Data)
}

func isMissing(err error) bool {
	return err != nil && os.IsNotExist(err)
}
