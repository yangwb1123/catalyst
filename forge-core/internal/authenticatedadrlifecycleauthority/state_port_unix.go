//go:build unix

package authenticatedadrlifecycleauthority

import (
	"crypto/rand"
	"fmt"
	"io/fs"
	"os"
)

type osCommitPort struct{}

func (osCommitPort) fillRandom(value []byte) error { _, err := rand.Read(value); return err }

func (osCommitPort) createExclusive(root *os.Root, name string,
	mode fs.FileMode) (*os.File, error) {
	file, err := root.OpenFile(name, os.O_WRONLY|os.O_CREATE|os.O_EXCL, mode)
	if err != nil {
		return nil, err
	}
	if err = file.Chmod(mode); err != nil {
		_ = file.Close()
		return nil, err
	}
	return file, nil
}

func (osCommitPort) writeAll(file *os.File, data []byte) error {
	for len(data) > 0 {
		count, err := file.Write(data)
		if err != nil {
			return err
		}
		if count <= 0 {
			return fmt.Errorf("state write made no progress")
		}
		data = data[count:]
	}
	return nil
}

func (osCommitPort) syncFile(file *os.File) error        { return file.Sync() }
func (osCommitPort) closeFile(file *os.File) error       { return file.Close() }
func (osCommitPort) syncDirectory(file *os.File) error   { return file.Sync() }
func (osCommitPort) beforeRename(*os.Root, string) error { return nil }
func (osCommitPort) rename(root *os.Root, oldName, newName string) error {
	return root.Rename(oldName, newName)
}
func (osCommitPort) remove(root *os.Root, name string) error {
	err := root.Remove(name)
	if os.IsNotExist(err) {
		return nil
	}
	return err
}
