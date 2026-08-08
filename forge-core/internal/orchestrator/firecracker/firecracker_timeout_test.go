package firecracker

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestBuildRootfsHonorsRunDeadline(t *testing.T) {
	tool := blockingTool(t)
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Millisecond)
	defer cancel()
	started := time.Now()
	err := buildRootfs(ctx, tool, t.TempDir(), filepath.Join(t.TempDir(), "rootfs"))
	if !errors.Is(err, context.DeadlineExceeded) || time.Since(started) > time.Second {
		t.Fatalf("bounded rootfs build error = %v after %s", err, time.Since(started))
	}
}

func TestMarkerReadHonorsRunDeadline(t *testing.T) {
	tool := blockingTool(t)
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Millisecond)
	defer cancel()
	started := time.Now()
	_, _, err := readMarkerWithRetry(ctx, tool, filepath.Join(t.TempDir(), "rootfs"))
	if !errors.Is(err, context.DeadlineExceeded) || time.Since(started) > time.Second {
		t.Fatalf("bounded marker read error = %v after %s", err, time.Since(started))
	}
}

func TestFirecrackerContextErrorPreservesCancellationClass(t *testing.T) {
	cancelled := firecrackerContextError(context.Canceled, 0)
	if !errors.Is(cancelled, context.Canceled) {
		t.Fatalf("cancel error = %v", cancelled)
	}
	timedOut := firecrackerContextError(context.DeadlineExceeded, 25*time.Millisecond)
	if !errors.Is(timedOut, context.DeadlineExceeded) {
		t.Fatalf("deadline error = %v", timedOut)
	}
}

func TestHostToolResolutionUsesPathAndRequiresExecutable(t *testing.T) {
	dir := t.TempDir()
	tool := filepath.Join(dir, "forge-fake-tool")
	if err := os.WriteFile(tool, []byte("#!/bin/sh\nexit 0\n"), 0o700); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", dir)
	if err := checkHostTool("forge-fake-tool"); err != nil {
		t.Fatalf("PATH tool rejected: %v", err)
	}
	if err := os.Chmod(tool, 0o600); err != nil {
		t.Fatal(err)
	}
	if err := checkHostTool("forge-fake-tool"); err == nil {
		t.Fatal("non-executable tool accepted")
	}
}

func blockingTool(t *testing.T) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "blocking-tool")
	if err := os.WriteFile(path, []byte("#!/bin/sh\nexec sleep 10\n"), 0o700); err != nil {
		t.Fatal(err)
	}
	return path
}
