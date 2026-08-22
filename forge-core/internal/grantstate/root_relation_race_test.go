//go:build unix

package grantstate

import (
	"errors"
	"io/fs"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestStableDirectoryOpenRejectsFIFOReplacementWithoutBlocking(t *testing.T) {
	requireFIFOFixture(t)
	for _, name := range []string{"repository-endpoint", "ancestor-component"} {
		t.Run(name, func(t *testing.T) {
			path := filepath.Join(t.TempDir(), name)
			if err := os.Mkdir(path, 0o700); err != nil {
				t.Fatal(err)
			}
			result := make(chan error, 1)
			go func() {
				binding, err := bindStableDirectoryWith(path, replaceWithFIFO)
				if binding != nil {
					_ = binding.file.Close()
				}
				result <- err
			}()
			select {
			case err := <-result:
				if err == nil {
					t.Fatal("FIFO replacement was accepted")
				}
			case <-time.After(2 * time.Second):
				t.Fatal("directory open blocked on FIFO replacement")
			}
		})
	}
}

func TestAncestorInspectionRejectsFIFOReplacementWithoutBlocking(t *testing.T) {
	requireFIFOFixture(t)
	base := t.TempDir()
	child := filepath.Join(base, "parent", "child")
	if err := os.MkdirAll(child, 0o700); err != nil {
		t.Fatal(err)
	}
	target, err := os.Stat(base)
	if err != nil {
		t.Fatal(err)
	}
	parent := filepath.Dir(child)
	probe := rootIdentityProbe{inspect: fifoReplacingInspector(parent), same: os.SameFile}
	result := make(chan error, 1)
	go func() {
		_, err := ancestorIdentityAppears(child, target, probe)
		result <- err
	}()
	select {
	case err := <-result:
		if err == nil {
			t.Fatal("ancestor FIFO replacement was accepted")
		}
	case <-time.After(2 * time.Second):
		t.Fatal("ancestor inspection blocked on FIFO replacement")
	}
}

func requireFIFOFixture(t *testing.T) {
	t.Helper()
	path := filepath.Join(t.TempDir(), "availability-probe")
	if err := makeFIFO(path, 0o600); err != nil {
		if errors.Is(err, errFIFOFixtureUnsupported) {
			t.Skip(err)
		}
		t.Fatal(err)
	}
}

func fifoReplacingInspector(target string) func(string) (fs.FileInfo, error) {
	return func(path string) (fs.FileInfo, error) {
		if path != target {
			return inspectStableDirectory(path)
		}
		binding, err := bindStableDirectoryWith(path, replaceWithFIFO)
		if binding != nil {
			_ = binding.file.Close()
		}
		if err != nil {
			return nil, err
		}
		return binding.info, nil
	}
}

func replaceWithFIFO(path string) error {
	moved := path + "-moved"
	if err := os.Rename(path, moved); err != nil {
		return err
	}
	if err := makeFIFO(path, 0o600); err != nil {
		return err
	}
	return nil
}
