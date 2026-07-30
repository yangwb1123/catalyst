//go:build unix

package statefs

import "path/filepath"

func syncSecuredDirectory(path string, created bool) error {
	if err := SyncDir(path); err != nil {
		return err
	}
	if !created {
		return nil
	}
	parent, err := filepath.EvalSymlinks(filepath.Dir(path))
	if err != nil {
		return err
	}
	return SyncDir(parent)
}
