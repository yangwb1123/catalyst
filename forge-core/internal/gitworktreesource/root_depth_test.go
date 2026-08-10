package gitworktreesource

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestOpenSourceParentHandlesDeepPathAndCancellation(t *testing.T) {
	rootPath := t.TempDir()
	components := make([]string, 128)
	for index := range components {
		components[index] = "d"
	}
	directory := filepath.Join(append([]string{rootPath}, components...)...)
	if err := os.MkdirAll(directory, 0o755); err != nil {
		t.Fatal(err)
	}
	root, err := openSourceTreeRoot(rootPath)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = root.handle.Close() }()
	path := strings.Join(components, "/") + "/file.go"
	parent, err := openSourceParent(context.Background(), root, path)
	if err != nil {
		t.Fatal(err)
	}
	parent.close()
	canceled, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := openSourceParent(canceled, root, path); !errors.Is(err, context.Canceled) {
		t.Fatalf("canceled deep traversal error = %v", err)
	}
}

func BenchmarkOpenSourceParentDeepPath(b *testing.B) {
	rootPath := b.TempDir()
	components := make([]string, 128)
	for index := range components {
		components[index] = "d"
	}
	if err := os.MkdirAll(filepath.Join(append([]string{rootPath}, components...)...), 0o755); err != nil {
		b.Fatal(err)
	}
	root, err := openSourceTreeRoot(rootPath)
	if err != nil {
		b.Fatal(err)
	}
	defer func() { _ = root.handle.Close() }()
	path := strings.Join(components, "/") + "/file.go"
	b.ResetTimer()
	for iteration := 0; iteration < b.N; iteration++ {
		parent, err := openSourceParent(context.Background(), root, path)
		if err != nil {
			b.Fatal(err)
		}
		parent.close()
	}
}
