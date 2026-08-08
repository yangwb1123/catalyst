//go:build unix

package firecracker

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"syscall"
	"testing"
)

func TestCopyTreeRejectsSpecialTemplateEntries(t *testing.T) {
	source := t.TempDir()
	pipe := filepath.Join(source, "guest-pipe")
	if err := syscall.Mkfifo(pipe, 0o600); err != nil {
		t.Fatal(err)
	}
	err := copyTree(context.Background(), source, filepath.Join(t.TempDir(), "copy"))
	if err == nil || !strings.Contains(err.Error(), "unsupported rootfs template entry") {
		t.Fatalf("special entry error = %v", err)
	}
}

func TestCopyTreeHonorsCancellationAndRemovesPartialTarget(t *testing.T) {
	source := t.TempDir()
	if err := os.WriteFile(filepath.Join(source, "large"), make([]byte, 2*copyBufferBytes), 0o600); err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	destination := filepath.Join(t.TempDir(), "copy")
	err := copyTree(ctx, source, destination)
	if !errors.Is(err, context.Canceled) {
		t.Fatalf("cancel error = %v", err)
	}
	if _, statErr := os.Stat(filepath.Join(destination, "large")); !os.IsNotExist(statErr) {
		t.Fatalf("partial target remains: %v", statErr)
	}
}

func TestInjectedFileRejectsSymlinkTarget(t *testing.T) {
	dir := t.TempDir()
	outside := filepath.Join(t.TempDir(), "outside")
	if err := os.WriteFile(outside, []byte("safe"), 0o600); err != nil {
		t.Fatal(err)
	}
	link := filepath.Join(dir, "init")
	if err := os.Symlink(outside, link); err != nil {
		t.Fatal(err)
	}
	if err := writeInjectedFile(link, []byte("unsafe"), 0o755); err == nil {
		t.Fatal("symlink injection target accepted")
	}
	content, err := os.ReadFile(outside)
	if err != nil || string(content) != "safe" {
		t.Fatalf("outside target changed: %q, %v", content, err)
	}
}

func TestCopyTreeRejectsSymlinkTemplateRoot(t *testing.T) {
	realRoot := t.TempDir()
	link := filepath.Join(t.TempDir(), "root-link")
	if err := os.Symlink(realRoot, link); err != nil {
		t.Fatal(err)
	}
	err := copyTree(context.Background(), link, filepath.Join(t.TempDir(), "copy"))
	if err == nil || !strings.Contains(err.Error(), "rootfs template root is not a directory") {
		t.Fatalf("symlink root error = %v", err)
	}
}
