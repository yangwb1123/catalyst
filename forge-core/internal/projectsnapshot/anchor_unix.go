//go:build linux

package projectsnapshot

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

func openCaptureAnchorWith(path string, observer captureObserver) (*treeRoot, error) {
	absolute, err := filepath.Abs(path)
	if err != nil {
		return nil, fmt.Errorf("resolve requested repository root: %w", err)
	}
	absolute = filepath.Clean(absolute)
	base, err := os.OpenRoot("/")
	if err != nil {
		return nil, fmt.Errorf("open filesystem root: %w", err)
	}
	root := &treeRoot{anchored: true, owned: []*os.Root{base}, path: absolute}
	current := base
	for _, component := range strings.Split(strings.TrimPrefix(absolute, "/"), "/") {
		if component == "" {
			continue
		}
		child, bindErr := bindAnchorComponent(root, current, component, observer)
		if bindErr != nil {
			root.close()
			return nil, bindErr
		}
		current = child
	}
	root.handle = current
	root.identity, err = current.Lstat(".")
	if err != nil || !root.identity.IsDir() {
		root.close()
		return nil, fmt.Errorf("inspect repository root anchor")
	}
	if err := root.verifyAnchor(); err != nil {
		root.close()
		return nil, err
	}
	return root, nil
}

func bindAnchorComponent(
	root *treeRoot,
	parent *os.Root,
	component string,
	observer captureObserver,
) (*os.Root, error) {
	before, err := parent.Lstat(component)
	if err != nil || !before.IsDir() || before.Mode()&os.ModeSymlink != 0 {
		return nil, fmt.Errorf("repository ancestor is not a real directory")
	}
	prefix := anchorPrefix(root, component)
	observe(observer, observeBeforeAnchorOpen, prefix)
	child, err := parent.OpenRoot(component)
	if err != nil {
		return nil, fmt.Errorf("open repository ancestor: %w", err)
	}
	opened, openErr := child.Lstat(".")
	after, afterErr := parent.Lstat(component)
	if openErr != nil || afterErr != nil || !stableDirectory(before, opened) ||
		!stableDirectory(opened, after) {
		_ = child.Close()
		return nil, fmt.Errorf("repository ancestor changed while opening")
	}
	root.anchorBindings = append(root.anchorBindings, parentBinding{
		child: child, identity: opened, name: component, parent: parent, path: prefix,
	})
	root.owned = append(root.owned, child)
	return child, nil
}

func anchorPrefix(root *treeRoot, component string) string {
	if len(root.anchorBindings) == 0 {
		return "/" + component
	}
	return root.anchorBindings[len(root.anchorBindings)-1].path + "/" + component
}
