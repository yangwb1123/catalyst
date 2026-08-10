package localcommandobservationproducer

import (
	"context"
	"fmt"
	"os"
	"strings"
)

// sourceTreeRoot and sourceParent keep directory handles anchored beneath the
// canonical repository root. Every intermediate component is required to be a
// real directory. Holding each parent open prevents a namespace replacement
// from redirecting a leaf read, while the repeated Lstat checks fail closed if
// any component changes during the snapshot.
type sourceTreeRoot struct {
	handle   *os.Root
	identity os.FileInfo
	path     string
}

type sourceParentBinding struct {
	child    *os.Root
	identity os.FileInfo
	name     string
	parent   *os.Root
	path     string
}

type missingSourceParent struct {
	name   string
	parent *os.Root
	path   string
}

type sourceParent struct {
	bindings []sourceParentBinding
	leaf     string
	leafRoot *os.Root
	missing  *missingSourceParent
	owned    []*os.Root
	path     string
	tree     *sourceTreeRoot
}

func openSourceTreeRoot(path string) (*sourceTreeRoot, error) {
	before, err := os.Lstat(path)
	if err != nil {
		return nil, fmt.Errorf("inspect canonical repository root: %w", err)
	}
	if before.Mode()&os.ModeSymlink != 0 || !before.IsDir() {
		return nil, fmt.Errorf("canonical repository root %q is not a real directory", path)
	}
	handle, err := os.OpenRoot(path)
	if err != nil {
		return nil, fmt.Errorf("open canonical repository root: %w", err)
	}
	opened, openErr := handle.Lstat(".")
	after, afterErr := os.Lstat(path)
	if openErr != nil || afterErr != nil || !stableSourceDirectory(before, opened) ||
		!stableSourceDirectory(opened, after) {
		_ = handle.Close()
		return nil, fmt.Errorf("canonical repository root %q changed while opening", path)
	}
	return &sourceTreeRoot{handle: handle, identity: opened, path: path}, nil
}

func (root *sourceTreeRoot) verify() error {
	opened, openErr := root.handle.Lstat(".")
	current, currentErr := os.Lstat(root.path)
	if openErr != nil || currentErr != nil || !stableSourceDirectory(root.identity, opened) ||
		!stableSourceDirectory(root.identity, current) {
		return fmt.Errorf("canonical repository root %q changed during source snapshot", root.path)
	}
	return nil
}

func openSourceParent(root *sourceTreeRoot, path string) (*sourceParent, error) {
	components := strings.Split(path, "/")
	parent := &sourceParent{
		leaf: components[len(components)-1], leafRoot: root.handle, path: path, tree: root,
	}
	if err := parent.verify(); err != nil {
		return nil, err
	}
	current := root.handle
	for index, component := range components[:len(components)-1] {
		prefix := strings.Join(components[:index+1], "/")
		before, missing, err := parent.inspectComponent(current, component, prefix)
		if err != nil {
			parent.close()
			return nil, err
		}
		if missing {
			return parent, nil
		}
		child, err := parent.bindComponent(current, component, prefix, before)
		if err != nil {
			parent.close()
			return nil, err
		}
		current = child
	}
	if err := parent.verify(); err != nil {
		parent.close()
		return nil, err
	}
	return parent, nil
}

func (parent *sourceParent) inspectComponent(
	current *os.Root,
	component string,
	prefix string,
) (os.FileInfo, bool, error) {
	before, err := current.Lstat(component)
	if os.IsNotExist(err) {
		parent.missing = &missingSourceParent{name: component, parent: current, path: prefix}
		if err := parent.verify(); err != nil {
			return nil, false, err
		}
		return nil, true, nil
	}
	if err != nil {
		return nil, false, fmt.Errorf("inspect source path %q parent %q: %w", parent.path, prefix, err)
	}
	if before.Mode()&os.ModeSymlink != 0 {
		return nil, false, fmt.Errorf("source path %q has forbidden symlink parent %q", parent.path, prefix)
	}
	if !before.IsDir() {
		return nil, false, fmt.Errorf("source path %q has non-directory parent %q", parent.path, prefix)
	}
	if err := parent.verify(); err != nil {
		return nil, false, err
	}
	return before, false, nil
}

func (parent *sourceParent) bindComponent(
	current *os.Root,
	component string,
	prefix string,
	before os.FileInfo,
) (*os.Root, error) {
	child, err := current.OpenRoot(component)
	if err != nil {
		return nil, fmt.Errorf("open source path %q parent %q: %w", parent.path, prefix, err)
	}
	opened, openErr := child.Lstat(".")
	after, afterErr := current.Lstat(component)
	if openErr != nil || afterErr != nil || !stableSourceDirectory(before, opened) ||
		!stableSourceDirectory(opened, after) {
		_ = child.Close()
		return nil, fmt.Errorf("source path %q parent %q changed while opening", parent.path, prefix)
	}
	parent.bindings = append(parent.bindings, sourceParentBinding{
		child: child, identity: opened, name: component, parent: current, path: prefix,
	})
	parent.owned = append(parent.owned, child)
	parent.leafRoot = child
	return child, nil
}

func (parent *sourceParent) verify() error {
	if err := parent.tree.verify(); err != nil {
		return err
	}
	for _, binding := range parent.bindings {
		current, currentErr := binding.parent.Lstat(binding.name)
		opened, openErr := binding.child.Lstat(".")
		if currentErr != nil || openErr != nil ||
			!stableSourceDirectory(binding.identity, current) ||
			!stableSourceDirectory(binding.identity, opened) {
			return fmt.Errorf("source path %q parent %q changed during inspection", parent.path, binding.path)
		}
	}
	if parent.missing != nil {
		_, err := parent.missing.parent.Lstat(parent.missing.name)
		if !os.IsNotExist(err) {
			return fmt.Errorf("source path %q parent %q changed during inspection", parent.path, parent.missing.path)
		}
	}
	return nil
}

func (parent *sourceParent) openRegular(before os.FileInfo) (*os.File, os.FileInfo, error) {
	if err := parent.verify(); err != nil {
		return nil, nil, err
	}
	file, err := parent.leafRoot.Open(parent.leaf)
	if err != nil {
		return nil, nil, fmt.Errorf("open source path %q: %w", parent.path, err)
	}
	opened, openErr := file.Stat()
	current, currentErr := parent.leafRoot.Lstat(parent.leaf)
	if openErr != nil || currentErr != nil || !opened.Mode().IsRegular() ||
		!stableSourceFile(before, opened) || !stableSourceFile(opened, current) {
		_ = file.Close()
		return nil, nil, fmt.Errorf("source path %q changed while opening", parent.path)
	}
	if err := parent.verify(); err != nil {
		_ = file.Close()
		return nil, nil, err
	}
	return file, opened, nil
}

func (parent *sourceParent) verifyRegularLeaf(expected os.FileInfo) error {
	current, err := parent.leafRoot.Lstat(parent.leaf)
	if err != nil || !current.Mode().IsRegular() || !stableSourceFile(expected, current) {
		return fmt.Errorf("source path %q changed while hashing", parent.path)
	}
	if err := parent.verify(); err != nil {
		return err
	}
	return nil
}

func (parent *sourceParent) verifySymlinkLeaf(expected os.FileInfo) error {
	current, err := parent.leafRoot.Lstat(parent.leaf)
	if err != nil || current.Mode()&os.ModeSymlink == 0 || !stableSourceFile(expected, current) {
		return fmt.Errorf("source symlink %q changed while reading", parent.path)
	}
	if err := parent.verify(); err != nil {
		return err
	}
	return nil
}

func (parent *sourceParent) verifyMissingLeaf() error {
	if err := parent.verify(); err != nil {
		return err
	}
	_, err := parent.leafRoot.Lstat(parent.leaf)
	if !os.IsNotExist(err) {
		return fmt.Errorf("tracked source path %q changed while recording deletion", parent.path)
	}
	return parent.verify()
}

func (parent *sourceParent) close() {
	for index := len(parent.owned) - 1; index >= 0; index-- {
		_ = parent.owned[index].Close()
	}
}

func stableSourceDirectory(expected, current os.FileInfo) bool {
	return expected != nil && current != nil && expected.IsDir() && current.IsDir() &&
		expected.Mode()&os.ModeSymlink == 0 && current.Mode()&os.ModeSymlink == 0 &&
		os.SameFile(expected, current)
}

func stableSourceFile(expected, current os.FileInfo) bool {
	return expected != nil && current != nil && os.SameFile(expected, current) &&
		expected.Size() == current.Size() && expected.Mode() == current.Mode() &&
		expected.ModTime().Equal(current.ModTime())
}

func hashOpenedSourceFile(ctx context.Context, path string, file *os.File, before os.FileInfo) (string, int64, error) {
	if before.Size() > maxIndividualFileBytes {
		return "", 0, fmt.Errorf("file %q exceeds %d bytes", path, maxIndividualFileBytes)
	}
	digest, count, err := hashReaderWithLimit(ctx, path, file, maxIndividualFileBytes)
	if err != nil {
		return "", 0, err
	}
	after, err := file.Stat()
	if err != nil {
		return "", 0, fmt.Errorf("stat source path %q after hashing: %w", path, err)
	}
	if count != before.Size() || !stableSourceFile(before, after) {
		return "", 0, fmt.Errorf("source path %q changed while hashing", path)
	}
	return digest, count, nil
}
