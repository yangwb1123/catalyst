//go:build unix

package grantstate

import (
	"crypto/rand"
	"fmt"
	"io/fs"
	"os"
)

type osCommitPort struct{}

func (osCommitPort) fillRandom(value []byte) error {
	_, err := rand.Read(value)
	return err
}

func (osCommitPort) createExclusive(dir *os.Root, name string, mode fs.FileMode) (*os.File, error) {
	file, err := dir.OpenFile(name, os.O_WRONLY|os.O_CREATE|os.O_EXCL, mode)
	if err != nil {
		return nil, err
	}
	if err := file.Chmod(mode); err != nil {
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
			return fmt.Errorf("write made no progress")
		}
		data = data[count:]
	}
	return nil
}

func (osCommitPort) syncFile(file *os.File) error { return file.Sync() }

func (osCommitPort) closeFile(file *os.File) error { return file.Close() }

func (osCommitPort) rename(dir *os.Root, oldName, newName string) error {
	return dir.Rename(oldName, newName)
}

func (osCommitPort) syncDirectory(dir *os.File) error { return dir.Sync() }

func (osCommitPort) remove(dir *os.Root, name string) error {
	err := dir.Remove(name)
	if os.IsNotExist(err) {
		return nil
	}
	return err
}
