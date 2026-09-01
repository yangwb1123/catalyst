//go:build unix

package appserver

import (
	"errors"
	"io/fs"
	"testing"
)

type failingRootStat struct{ closed bool }

func (*failingRootStat) Stat(string) (fs.FileInfo, error) {
	return nil, errors.New("injected root stat failure")
}

func (value *failingRootStat) Close() error {
	value.closed = true
	return nil
}

type failingFileStat struct{ closed bool }

func (*failingFileStat) Stat() (fs.FileInfo, error) {
	return nil, errors.New("injected file stat failure")
}

func (value *failingFileStat) Close() error {
	value.closed = true
	return nil
}

func TestStatFailuresCloseNewHandlesImmediately(t *testing.T) {
	root := &failingRootStat{}
	if _, err := statRootOrClose(root); err == nil || !root.closed {
		t.Fatalf("root stat error = %v, closed = %t", err, root.closed)
	}
	file := &failingFileStat{}
	if _, err := statFileOrClose(file); err == nil || !file.closed {
		t.Fatalf("file stat error = %v, closed = %t", err, file.closed)
	}
}
