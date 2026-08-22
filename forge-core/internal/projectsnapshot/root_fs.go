package projectsnapshot

import (
	"context"
	"fmt"
	"os"
	"strings"
)

type treeRoot struct {
	anchorBindings []parentBinding
	anchored       bool
	handle         *os.Root
	identity       os.FileInfo
	owned          []*os.Root
	path           string
}

type parentBinding struct {
	child    *os.Root
	identity os.FileInfo
	name     string
	parent   *os.Root
	path     string
}

type sourceParent struct {
	bindings []parentBinding
	ctx      context.Context
	leaf     string
	leafRoot *os.Root
	missing  *missingParent
	owned    []*os.Root
	path     string
	tree     *treeRoot
}

type missingParent struct {
	name   string
	parent *os.Root
	path   string
}

func openTreeRoot(path string) (*treeRoot, error) {
	before, err := os.Lstat(path)
	if err != nil || !before.IsDir() || before.Mode()&os.ModeSymlink != 0 {
		return nil, fmt.Errorf("repository root is not a real directory")
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
	return &treeRoot{handle: handle, identity: opened, path: path}, nil
}

func (root *treeRoot) close() {
	if len(root.owned) == 0 {
		_ = root.handle.Close()
		return
	}
	for index := len(root.owned) - 1; index >= 0; index-- {
		_ = root.owned[index].Close()
	}
}

func (root *treeRoot) verify() error {
	if root.anchored {
		return root.verifyAnchor()
	}
	opened, openErr := root.handle.Lstat(".")
	current, currentErr := os.Lstat(root.path)
	if openErr != nil || currentErr != nil || !stableDirectory(root.identity, opened) ||
		!stableDirectory(root.identity, current) {
		return fmt.Errorf("repository root changed during project snapshot")
	}
	return nil
}

func (root *treeRoot) verifyAnchor() error {
	opened, err := root.handle.Lstat(".")
	if err != nil || !stableDirectory(root.identity, opened) {
		return fmt.Errorf("repository root anchor changed during project snapshot")
	}
	for _, binding := range root.anchorBindings {
		current, currentErr := binding.parent.Lstat(binding.name)
		child, childErr := binding.child.Lstat(".")
		if currentErr != nil || childErr != nil ||
			!sameDirectory(binding.identity, current) ||
			!sameDirectory(binding.identity, child) {
			return fmt.Errorf("repository ancestor %q changed during project snapshot", binding.path)
		}
	}
	return nil
}

func openParent(ctx context.Context, root *treeRoot, path string) (*sourceParent, error) {
	components := strings.Split(path, "/")
	parent := &sourceParent{
		ctx: ctx, leaf: components[len(components)-1], leafRoot: root.handle,
		path: path, tree: root,
	}
	if err := parent.verify(); err != nil {
		return nil, err
	}
	current := root.handle
	var prefix strings.Builder
	for index, component := range components[:len(components)-1] {
		if err := ctx.Err(); err != nil {
			parent.close()
			return nil, fmt.Errorf("inspect project path: %w", err)
		}
		if index != 0 {
			prefix.WriteByte('/')
		}
		prefix.WriteString(component)
		before, missing, err := inspectParentComponent(current, component, prefix.String())
		if err != nil {
			parent.close()
			return nil, err
		}
		if missing {
			parent.missing = &missingParent{
				name: component, parent: current, path: prefix.String(),
			}
			if err := parent.verify(); err != nil {
				parent.close()
				return nil, err
			}
			return parent, nil
		}
		child, err := parent.bindComponent(current, component, prefix.String(), before)
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

func inspectParentComponent(
	current *os.Root,
	component, prefix string,
) (os.FileInfo, bool, error) {
	before, err := current.Lstat(component)
	if os.IsNotExist(err) {
		return nil, true, nil
	}
	if err != nil {
		return nil, false, fmt.Errorf("inspect project parent %q: %w", prefix, err)
	}
	if before.Mode()&os.ModeSymlink != 0 || !before.IsDir() {
		return nil, false, fmt.Errorf("project parent %q is not a real directory", prefix)
	}
	return before, false, nil
}

func (parent *sourceParent) bindComponent(
	current *os.Root,
	component, prefix string,
	before os.FileInfo,
) (*os.Root, error) {
	child, err := current.OpenRoot(component)
	if err != nil {
		return nil, fmt.Errorf("open project parent %q: %w", prefix, err)
	}
	opened, openErr := child.Lstat(".")
	after, afterErr := current.Lstat(component)
	if openErr != nil || afterErr != nil || !stableDirectory(before, opened) ||
		!stableDirectory(opened, after) {
		_ = child.Close()
		return nil, fmt.Errorf("project parent %q changed while opening", prefix)
	}
	parent.bindings = append(parent.bindings, parentBinding{
		child: child, identity: opened, name: component, parent: current, path: prefix,
	})
	parent.owned = append(parent.owned, child)
	parent.leafRoot = child
	return child, nil
}

func (parent *sourceParent) verify() error {
	if err := parent.ctx.Err(); err != nil {
		return fmt.Errorf("inspect project path: %w", err)
	}
	if err := parent.tree.verify(); err != nil {
		return err
	}
	for _, binding := range parent.bindings {
		current, currentErr := binding.parent.Lstat(binding.name)
		opened, openErr := binding.child.Lstat(".")
		if currentErr != nil || openErr != nil || !stableDirectory(binding.identity, current) ||
			!stableDirectory(binding.identity, opened) {
			return fmt.Errorf("project parent %q changed during inspection", binding.path)
		}
	}
	if parent.missing != nil {
		_, err := parent.missing.parent.Lstat(parent.missing.name)
		if !os.IsNotExist(err) {
			return fmt.Errorf("project parent %q changed during inspection", parent.missing.path)
		}
	}
	return nil
}

func (parent *sourceParent) openRegular(before os.FileInfo) (*os.File, os.FileInfo, error) {
	if err := parent.verify(); err != nil {
		return nil, nil, err
	}
	file, err := openRegularLeaf(parent.leafRoot, parent.leaf)
	if err != nil {
		return nil, nil, fmt.Errorf("open project source path: %w", err)
	}
	opened, openErr := file.Stat()
	current, currentErr := parent.leafRoot.Lstat(parent.leaf)
	if openErr != nil || currentErr != nil || !opened.Mode().IsRegular() ||
		!stableFile(before, opened) || !stableFile(opened, current) {
		_ = file.Close()
		return nil, nil, fmt.Errorf("project source path changed while opening")
	}
	if err := requireSingleLink(file, opened); err != nil {
		_ = file.Close()
		return nil, nil, err
	}
	if err := parent.verify(); err != nil {
		_ = file.Close()
		return nil, nil, err
	}
	return file, opened, nil
}

func (parent *sourceParent) verifyRegular(file *os.File, expected os.FileInfo) error {
	opened, openErr := file.Stat()
	current, currentErr := parent.leafRoot.Lstat(parent.leaf)
	if openErr != nil || currentErr != nil || !stableFile(expected, opened) ||
		!stableFile(expected, current) {
		return fmt.Errorf("project source path changed while reading")
	}
	if err := requireSingleLink(file, opened); err != nil {
		return err
	}
	if err := requireSingleLink(nil, current); err != nil {
		return err
	}
	return parent.verify()
}

func (parent *sourceParent) close() {
	for index := len(parent.owned) - 1; index >= 0; index-- {
		_ = parent.owned[index].Close()
	}
}

func stableDirectory(expected, current os.FileInfo) bool {
	return sameDirectory(expected, current) && stableChangeIdentity(expected, current)
}

func sameDirectory(expected, current os.FileInfo) bool {
	return expected != nil && current != nil && expected.IsDir() && current.IsDir() &&
		expected.Mode()&os.ModeSymlink == 0 && current.Mode()&os.ModeSymlink == 0 &&
		os.SameFile(expected, current)
}

func stableFile(expected, current os.FileInfo) bool {
	return expected != nil && current != nil && os.SameFile(expected, current) &&
		expected.Size() == current.Size() && expected.Mode() == current.Mode() &&
		expected.ModTime().Equal(current.ModTime()) && stableChangeIdentity(expected, current)
}
