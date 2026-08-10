package evolvelocatorobservationproducer

import (
	"fmt"
	"os"
	"strings"
)

type rootedTree struct {
	handle   *os.Root
	identity os.FileInfo
	path     string
}

type directoryBinding struct {
	child    *os.Root
	identity os.FileInfo
	name     string
	parent   *os.Root
	path     string
}

type rootedParent struct {
	bindings []directoryBinding
	leaf     string
	leafRoot *os.Root
	owned    []*os.Root
	path     string
	tree     *rootedTree
}

func openRootedTree(path string) (*rootedTree, error) {
	before, err := os.Lstat(path)
	if err != nil || before.Mode()&os.ModeSymlink != 0 || !before.IsDir() {
		return nil, fmt.Errorf("repository root must be an available real directory")
	}
	handle, err := os.OpenRoot(path)
	if err != nil {
		return nil, fmt.Errorf("open repository root: %w", err)
	}
	opened, openErr := handle.Lstat(".")
	after, afterErr := os.Lstat(path)
	if openErr != nil || afterErr != nil || !stableDirectory(before, opened) ||
		!stableDirectory(opened, after) {
		_ = handle.Close()
		return nil, fmt.Errorf("repository root changed while opening")
	}
	return &rootedTree{handle: handle, identity: opened, path: path}, nil
}

func (tree *rootedTree) verify() error {
	opened, openErr := tree.handle.Lstat(".")
	current, currentErr := os.Lstat(tree.path)
	if openErr != nil || currentErr != nil || !stableDirectory(tree.identity, opened) ||
		!stableDirectory(tree.identity, current) {
		return fmt.Errorf("repository root changed during locator capture")
	}
	return nil
}

func openRootedParent(tree *rootedTree, path string) (*rootedParent, error) {
	components := strings.Split(path, "/")
	parent := &rootedParent{
		leaf: components[len(components)-1], leafRoot: tree.handle, path: path, tree: tree,
	}
	current := tree.handle
	for index, component := range components[:len(components)-1] {
		prefix := strings.Join(components[:index+1], "/")
		before, err := current.Lstat(component)
		if err != nil || before.Mode()&os.ModeSymlink != 0 || !before.IsDir() {
			parent.close()
			return nil, fmt.Errorf("locator path %q has unavailable, symlink, or non-directory parent %q", path, prefix)
		}
		child, err := current.OpenRoot(component)
		if err != nil {
			parent.close()
			return nil, fmt.Errorf("open locator path %q parent %q: %w", path, prefix, err)
		}
		opened, openErr := child.Lstat(".")
		after, afterErr := current.Lstat(component)
		if openErr != nil || afterErr != nil || !stableDirectory(before, opened) ||
			!stableDirectory(opened, after) {
			_ = child.Close()
			parent.close()
			return nil, fmt.Errorf("locator path %q parent %q changed while opening", path, prefix)
		}
		parent.bindings = append(parent.bindings, directoryBinding{
			child: child, identity: opened, name: component, parent: current, path: prefix,
		})
		parent.owned = append(parent.owned, child)
		parent.leafRoot, current = child, child
	}
	if err := parent.verify(); err != nil {
		parent.close()
		return nil, err
	}
	return parent, nil
}

func (parent *rootedParent) verify() error {
	if err := parent.tree.verify(); err != nil {
		return err
	}
	for _, binding := range parent.bindings {
		current, currentErr := binding.parent.Lstat(binding.name)
		opened, openErr := binding.child.Lstat(".")
		if currentErr != nil || openErr != nil || !stableDirectory(binding.identity, current) ||
			!stableDirectory(binding.identity, opened) {
			return fmt.Errorf("locator path %q parent %q changed during capture", parent.path, binding.path)
		}
	}
	return nil
}

func (parent *rootedParent) close() {
	for index := len(parent.owned) - 1; index >= 0; index-- {
		_ = parent.owned[index].Close()
	}
}

func stableDirectory(expected, current os.FileInfo) bool {
	return expected != nil && current != nil && expected.IsDir() && current.IsDir() &&
		expected.Mode()&os.ModeSymlink == 0 && current.Mode()&os.ModeSymlink == 0 &&
		os.SameFile(expected, current)
}

func stableFile(expected, current os.FileInfo) bool {
	return expected != nil && current != nil && expected.Mode().IsRegular() &&
		current.Mode().IsRegular() && os.SameFile(expected, current) &&
		expected.Size() == current.Size() && expected.Mode() == current.Mode() &&
		expected.ModTime().Equal(current.ModTime())
}
