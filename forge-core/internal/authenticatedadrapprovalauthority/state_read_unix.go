//go:build unix

package authenticatedadrapprovalauthority

import (
	"bytes"
	"crypto/sha256"
	"crypto/subtle"
	"fmt"
	"io"
	"io/fs"
	"os"
	"strings"
	"syscall"
)

type authorityLeafBinding struct {
	digest  [sha256.Size]byte
	info    fs.FileInfo
	maximum int64
	mode    fs.FileMode
}

func (s *unixState) current() (stateSnapshot, error) {
	if err := s.verifyBindings(); err != nil {
		return stateSnapshot{}, err
	}
	data, present, err := readStateLeaf(s.state, stateLedgerFile, s.maxBytes)
	if err != nil {
		return stateSnapshot{}, err
	}
	return stateSnapshot{Data: data, Present: present}, nil
}

func (s *unixState) readLeaf(relative string, maximum int64,
	mode fs.FileMode) ([]byte, error) {
	if err := requireRelative(relative, "authority leaf"); err != nil {
		return nil, err
	}
	if maximum <= 0 || maximum > maxLedgerBytes || mode.Perm() != mode || mode == 0 {
		return nil, fmt.Errorf("authority leaf bounds or required mode are invalid")
	}
	if err := s.verifyBindings(); err != nil {
		return nil, err
	}
	data, info, err := readAuthorityLeafBound(s.authority, relative, maximum, mode)
	if err != nil {
		return nil, err
	}
	digest := sha256.Sum256(data)
	if prior, ok := s.authorityLeaves[relative]; ok &&
		(!os.SameFile(prior.info, info) || prior.digest != digest ||
			prior.maximum != maximum || prior.mode != mode) {
		return discardBytes(data), fmt.Errorf("authority leaf binding changed")
	}
	s.authorityLeaves[relative] = authorityLeafBinding{digest: digest, info: info,
		maximum: maximum, mode: mode}
	if err := s.verifyBindings(); err != nil {
		return discardBytes(data), err
	}
	return data, nil
}

func readAuthorityLeaf(root *os.Root, relative string, maximum int64,
	mode fs.FileMode) ([]byte, error) {
	data, _, err := readAuthorityLeafBound(root, relative, maximum, mode)
	return data, err
}

func readAuthorityLeafBound(root *os.Root, relative string, maximum int64,
	mode fs.FileMode) ([]byte, fs.FileInfo, error) {
	parts := strings.Split(relative, "/")
	parent, owned, err := openLeafParent(root, parts[:len(parts)-1])
	if err != nil {
		return nil, nil, err
	}
	if owned {
		defer func() { _ = parent.Close() }()
	}
	return readRequiredLeaf(parent, parts[len(parts)-1], maximum, mode)
}

func openLeafParent(root *os.Root, parts []string) (*os.Root, bool, error) {
	current, owned := root, false
	for _, component := range parts {
		next, info, err := openChildDirectory(current, component)
		if owned {
			_ = current.Close()
		}
		if err != nil || requireOwnedDirectory(info, privateDirMode,
			"authority leaf parent") != nil {
			if next != nil {
				_ = next.Close()
			}
			return nil, false, fmt.Errorf("authority leaf parent rejected")
		}
		current, owned = next, true
	}
	return current, owned, nil
}

func readRequiredLeaf(root *os.Root, name string, maximum int64,
	mode fs.FileMode) ([]byte, fs.FileInfo, error) {
	file, err := openExistingRegular(root, name, false)
	if err != nil {
		return nil, nil, err
	}
	defer func() { _ = file.Close() }()
	before, err := validateOpenLeaf(file, mode, "authority leaf")
	if err != nil {
		return nil, nil, err
	}
	data, err := readBounded(file, before, maximum)
	if err != nil {
		return nil, nil, err
	}
	if err := verifyNamedIdentity(root, name, before, mode); err != nil {
		return discardBytes(data), nil, err
	}
	return data, before, nil
}

func (s *unixState) verifyAuthorityLeaves() error {
	for relative, binding := range s.authorityLeaves {
		data, info, err := readAuthorityLeafBound(s.authority, relative,
			binding.maximum, binding.mode)
		if err != nil {
			return err
		}
		digest := sha256.Sum256(data)
		clearBytes(data)
		if !os.SameFile(binding.info, info) ||
			subtle.ConstantTimeCompare(binding.digest[:], digest[:]) != 1 {
			return fmt.Errorf("authority leaf binding changed")
		}
	}
	return nil
}

func readStateLeaf(root *os.Root, name string, maximum int64) ([]byte, bool, error) {
	file, err := openExistingRegular(root, name, false)
	if os.IsNotExist(err) {
		return nil, false, nil
	}
	if err != nil {
		return nil, false, err
	}
	defer func() { _ = file.Close() }()
	before, err := validateOpenLeaf(file, privateMode, "approval state leaf")
	if err != nil {
		return nil, true, err
	}
	data, err := readBounded(file, before, maximum)
	if err != nil {
		return nil, true, err
	}
	if err := verifyNamedIdentity(root, name, before, privateMode); err != nil {
		return discardBytes(data), true, err
	}
	return data, true, nil
}

func readBounded(file *os.File, before fs.FileInfo, maximum int64) ([]byte, error) {
	if before.Size() > maximum {
		return nil, fmt.Errorf("leaf exceeds %d bytes", maximum)
	}
	data, err := io.ReadAll(io.LimitReader(file, maximum+1))
	if err != nil {
		return discardBytes(data), fmt.Errorf("read bounded leaf: %w", err)
	}
	if int64(len(data)) > maximum {
		return discardBytes(data), fmt.Errorf("leaf exceeds %d bytes", maximum)
	}
	after, err := file.Stat()
	if err != nil || !sameStableFile(before, after) {
		return discardBytes(data), fmt.Errorf("leaf changed while reading")
	}
	return data, nil
}

func validateOpenLeaf(file *os.File, mode fs.FileMode, label string) (fs.FileInfo, error) {
	info, err := file.Stat()
	if err != nil {
		return nil, err
	}
	if err := requireOwnedRegular(info, mode, label); err != nil {
		return nil, err
	}
	return info, nil
}

func verifyNamedIdentity(root *os.Root, name string, expected fs.FileInfo,
	mode fs.FileMode) error {
	file, err := openExistingRegular(root, name, false)
	if err != nil {
		return err
	}
	defer func() { _ = file.Close() }()
	info, err := validateOpenLeaf(file, mode, "bound leaf")
	if err != nil || !os.SameFile(expected, info) {
		return fmt.Errorf("leaf identity changed")
	}
	return nil
}

func openExistingRegular(root *os.Root, name string, write bool) (*os.File, error) {
	before, err := root.Lstat(name)
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
	file, err := root.OpenFile(name, flags, 0)
	if err != nil {
		return nil, err
	}
	opened, openErr := file.Stat()
	after, afterErr := root.Lstat(name)
	if openErr != nil || afterErr != nil || !os.SameFile(before, opened) ||
		!os.SameFile(opened, after) {
		_ = file.Close()
		return nil, fmt.Errorf("leaf changed while opening")
	}
	return file, nil
}

func snapshotsEqual(first, second stateSnapshot) bool {
	return first.Present == second.Present && bytes.Equal(first.Data, second.Data)
}
