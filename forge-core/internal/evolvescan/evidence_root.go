package evolvescan

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

const (
	evidenceAfterRootLstat = "after-root-lstat"
	evidenceAfterLeafLstat = "after-leaf-lstat"
)

type evidencePathObserver func(stage string)

type evidenceRoot struct {
	handle   *os.Root
	identity os.FileInfo
	path     string
}

type evidenceParentBinding struct {
	child    *os.Root
	identity os.FileInfo
	name     string
	parent   *os.Root
	prefix   string
}

type openedEvidencePath struct {
	bindings []evidenceParentBinding
	expected os.FileInfo
	file     *os.File
	leaf     string
	leafRoot *os.Root
	name     string
	owned    []*os.Root
	root     *evidenceRoot
}

func openEvidencePath(root, name string, observer evidencePathObserver) (*openedEvidencePath, error) {
	tree, err := openEvidenceRoot(root, observer)
	if err != nil {
		return nil, err
	}
	opened := &openedEvidencePath{root: tree, leafRoot: tree.handle, name: name}
	components := strings.Split(name, "/")
	opened.leaf = components[len(components)-1]
	current := tree.handle
	for index, component := range components[:len(components)-1] {
		prefix := strings.Join(components[:index+1], "/")
		child, err := opened.openParent(current, component, prefix, observer)
		if err != nil {
			opened.close()
			return nil, err
		}
		current = child
	}
	if err := opened.openLeaf(observer); err != nil {
		opened.close()
		return nil, err
	}
	return opened, nil
}

func openEvidenceRoot(path string, observer evidencePathObserver) (*evidenceRoot, error) {
	absolute, err := filepath.Abs(path)
	if err != nil {
		return nil, fmt.Errorf("resolve repository root: %w", err)
	}
	before, err := os.Lstat(absolute)
	if err != nil || !before.IsDir() || before.Mode()&os.ModeSymlink != 0 {
		return nil, fmt.Errorf("repository root must be an available non-symlink directory")
	}
	observeEvidencePath(observer, evidenceAfterRootLstat)
	handle, err := os.OpenRoot(absolute)
	if err != nil {
		return nil, fmt.Errorf("repository root must be an available non-symlink directory")
	}
	opened, openErr := handle.Lstat(".")
	after, afterErr := os.Lstat(absolute)
	if openErr != nil || afterErr != nil || !stableEvidenceDirectory(before, opened) ||
		!stableEvidenceDirectory(opened, after) {
		_ = handle.Close()
		return nil, fmt.Errorf("repository root must be an available non-symlink directory")
	}
	return &evidenceRoot{handle: handle, identity: opened, path: absolute}, nil
}

func (opened *openedEvidencePath) openParent(
	current *os.Root,
	component string,
	prefix string,
	observer evidencePathObserver,
) (*os.Root, error) {
	before, err := current.Lstat(component)
	if err != nil {
		return nil, fmt.Errorf("path %q is unavailable: %w", opened.name, err)
	}
	if before.Mode()&os.ModeSymlink != 0 {
		return nil, fmt.Errorf("path %q traverses a symlink", opened.name)
	}
	if !before.IsDir() {
		return nil, fmt.Errorf("path %q has a non-directory prefix", opened.name)
	}
	observeEvidencePath(observer, "after-parent-lstat:"+prefix)
	child, err := current.OpenRoot(component)
	if err != nil {
		if changed, statErr := current.Lstat(component); statErr == nil && changed.Mode()&os.ModeSymlink != 0 {
			return nil, fmt.Errorf("path %q traverses a symlink", opened.name)
		}
		return nil, fmt.Errorf("path %q is unavailable: %w", opened.name, err)
	}
	childInfo, childErr := child.Lstat(".")
	after, afterErr := current.Lstat(component)
	if afterErr == nil && after.Mode()&os.ModeSymlink != 0 {
		_ = child.Close()
		return nil, fmt.Errorf("path %q traverses a symlink", opened.name)
	}
	if childErr != nil || afterErr != nil || !stableEvidenceDirectory(before, childInfo) ||
		!stableEvidenceDirectory(childInfo, after) {
		_ = child.Close()
		return nil, fmt.Errorf("path %q is unavailable: parent %q changed identity", opened.name, prefix)
	}
	opened.bindings = append(opened.bindings, evidenceParentBinding{
		child: child, identity: childInfo, name: component, parent: current, prefix: prefix,
	})
	opened.owned = append(opened.owned, child)
	opened.leafRoot = child
	return child, nil
}

func (opened *openedEvidencePath) openLeaf(observer evidencePathObserver) error {
	before, err := opened.leafRoot.Lstat(opened.leaf)
	if err != nil {
		return fmt.Errorf("path %q is unavailable: %w", opened.name, err)
	}
	if before.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("path %q traverses a symlink", opened.name)
	}
	if !before.Mode().IsRegular() {
		return fmt.Errorf("path %q is not a regular file", opened.name)
	}
	observeEvidencePath(observer, evidenceAfterLeafLstat)
	file, err := opened.leafRoot.Open(opened.leaf)
	if err != nil {
		if changed, statErr := opened.leafRoot.Lstat(opened.leaf); statErr == nil && changed.Mode()&os.ModeSymlink != 0 {
			return fmt.Errorf("path %q traverses a symlink", opened.name)
		}
		return fmt.Errorf("path %q is not readable: %w", opened.name, err)
	}
	fileInfo, fileErr := file.Stat()
	if fileErr != nil {
		_ = file.Close()
		return fmt.Errorf("path %q cannot be inspected: %w", opened.name, fileErr)
	}
	after, afterErr := opened.leafRoot.Lstat(opened.leaf)
	if afterErr == nil && after.Mode()&os.ModeSymlink != 0 {
		_ = file.Close()
		return fmt.Errorf("path %q traverses a symlink", opened.name)
	}
	if afterErr != nil || !fileInfo.Mode().IsRegular() ||
		!stableEvidenceFile(before, fileInfo) || !stableEvidenceFile(fileInfo, after) {
		_ = file.Close()
		return fmt.Errorf("path %q changed identity while being validated", opened.name)
	}
	opened.expected = fileInfo
	opened.file = file
	return opened.verifyParents()
}

func (opened *openedEvidencePath) verify() error {
	current, err := opened.leafRoot.Lstat(opened.leaf)
	if err != nil || current.Mode()&os.ModeSymlink != 0 || !current.Mode().IsRegular() ||
		!stableEvidenceFile(opened.expected, current) {
		return fmt.Errorf("path %q changed identity while being validated", opened.name)
	}
	info, err := opened.file.Stat()
	if err != nil || !stableEvidenceFile(opened.expected, info) {
		return fmt.Errorf("path %q changed identity while being validated", opened.name)
	}
	return opened.verifyParents()
}

func (opened *openedEvidencePath) verifyParents() error {
	if err := opened.root.verify(); err != nil {
		return err
	}
	for _, binding := range opened.bindings {
		current, currentErr := binding.parent.Lstat(binding.name)
		child, childErr := binding.child.Lstat(".")
		if currentErr == nil && current.Mode()&os.ModeSymlink != 0 {
			return fmt.Errorf("path %q traverses a symlink", opened.name)
		}
		if currentErr != nil || childErr != nil || !stableEvidenceDirectory(binding.identity, current) ||
			!stableEvidenceDirectory(binding.identity, child) {
			return fmt.Errorf("path %q is unavailable: parent %q changed identity", opened.name, binding.prefix)
		}
	}
	return nil
}

func (root *evidenceRoot) verify() error {
	opened, openErr := root.handle.Lstat(".")
	current, currentErr := os.Lstat(root.path)
	if openErr != nil || currentErr != nil || !stableEvidenceDirectory(root.identity, opened) ||
		!stableEvidenceDirectory(root.identity, current) {
		return fmt.Errorf("repository root must be an available non-symlink directory")
	}
	return nil
}

func (opened *openedEvidencePath) close() {
	if opened.file != nil {
		_ = opened.file.Close()
	}
	for index := len(opened.owned) - 1; index >= 0; index-- {
		_ = opened.owned[index].Close()
	}
	if opened.root != nil && opened.root.handle != nil {
		_ = opened.root.handle.Close()
	}
}

func stableEvidenceDirectory(expected, current os.FileInfo) bool {
	return expected != nil && current != nil && expected.IsDir() && current.IsDir() &&
		expected.Mode()&os.ModeSymlink == 0 && current.Mode()&os.ModeSymlink == 0 &&
		os.SameFile(expected, current)
}

func stableEvidenceFile(expected, current os.FileInfo) bool {
	return expected != nil && current != nil && os.SameFile(expected, current) &&
		expected.Size() == current.Size() && expected.Mode() == current.Mode() &&
		expected.ModTime().Equal(current.ModTime())
}

func observeEvidencePath(observer evidencePathObserver, stage string) {
	if observer != nil {
		observer(stage)
	}
}
