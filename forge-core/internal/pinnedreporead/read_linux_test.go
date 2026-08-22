//go:build linux && (amd64 || arm64)

package pinnedreporead

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"syscall"
	"testing"
	"time"
)

func TestReadReturnsExactBinaryBytes(t *testing.T) {
	if err := CheckPlatform(); err != nil {
		t.Fatal(err)
	}
	root := t.TempDir()
	content := []byte{0, 1, 'x', '\n', 0xff}
	writeFile(t, root, "data/blob.bin", content)
	handle := openRoot(t, root)
	defer func() { _ = handle.Close() }()
	files, err := Read(context.Background(), handle, []ExpectedEntry{
		entry("data/blob.bin", content),
	}, testLimits())
	if err != nil {
		t.Fatal(err)
	}
	if len(files) != 1 || string(files[0].Content) != string(content) {
		t.Fatalf("files = %#v", files)
	}
}

func TestReadAllowsOneFileToUseTheFullAggregateCeiling(t *testing.T) {
	root := t.TempDir()
	content := bytes.Repeat([]byte("x"), int(MaxTotalBytes))
	writeFile(t, root, "large.bin", content)
	handle := openRoot(t, root)
	defer func() { _ = handle.Close() }()
	files, err := Read(context.Background(), handle,
		[]ExpectedEntry{entry("large.bin", content)}, testLimits())
	if err != nil || len(files) != 1 || len(files[0].Content) != len(content) {
		t.Fatalf("full-ceiling read = %d files, %v", len(files), err)
	}
}

func TestReadDoesNotAdvanceRegularFileAccessTime(t *testing.T) {
	root := t.TempDir()
	content := []byte("atime-sensitive")
	writeFile(t, root, "item", content)
	path := filepath.Join(root, "item")
	old := time.Unix(946684800, 0)
	if err := os.Chtimes(path, old, old); err != nil {
		t.Fatal(err)
	}
	before := fileAccessTime(t, path)
	handle := openRoot(t, root)
	defer func() { _ = handle.Close() }()
	if _, err := Read(context.Background(), handle,
		[]ExpectedEntry{entry("item", content)}, testLimits()); err != nil {
		t.Fatal(err)
	}
	if after := fileAccessTime(t, path); after != before {
		t.Fatalf("repository atime changed from %d to %d", before, after)
	}
}

func fileAccessTime(t *testing.T, path string) int64 {
	t.Helper()
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !ok {
		t.Fatal("regular file stat is unavailable")
	}
	return stat.Atim.Sec*1_000_000_000 + stat.Atim.Nsec
}

func TestReadRejectsAliasesSpecialFilesAndControlPaths(t *testing.T) {
	root := t.TempDir()
	content := []byte("value")
	writeFile(t, root, "plain", content)
	if err := os.Link(filepath.Join(root, "plain"), filepath.Join(root, "hard")); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("plain", filepath.Join(root, "link")); err != nil {
		t.Fatal(err)
	}
	if err := syscall.Mkfifo(filepath.Join(root, "pipe"), 0o600); err != nil {
		t.Fatal(err)
	}
	handle := openRoot(t, root)
	defer func() { _ = handle.Close() }()
	for _, candidate := range []ExpectedEntry{
		entry("hard", content), entry("link", content), entry("pipe", nil),
		entry(".git/config", content), entry(".FORGE/state", content),
	} {
		if files, err := Read(context.Background(), handle,
			[]ExpectedEntry{candidate}, testLimits()); err == nil || files != nil {
			t.Errorf("unsafe path %q was read: %#v", candidate.Path, files)
		}
	}
}

func TestPathOnlyPrecheckRejectsFIFOWithoutActiveOpen(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "pipe")
	if err := syscall.Mkfifo(path, 0o600); err != nil {
		t.Fatal(err)
	}
	handle := openRoot(t, root)
	defer func() { _ = handle.Close() }()
	activeCalled := false
	openActive := func(*os.File, string) (*os.File, error) {
		activeCalled = true
		return nil, fmt.Errorf("active open must not run")
	}
	if file, _, err := openVerifiedLeafWith(context.Background(), handle, entry("pipe", nil),
		openActive, nil); err == nil || file != nil || activeCalled {
		t.Fatalf("FIFO precheck = %#v, %v, active=%v", file, err, activeCalled)
	}
}

func TestPathPrecheckCancellationPreventsActiveOpen(t *testing.T) {
	root := t.TempDir()
	content := []byte("value")
	writeFile(t, root, "item", content)
	handle := openRoot(t, root)
	defer func() { _ = handle.Close() }()
	ctx, cancel := context.WithCancel(context.Background())
	activeCalled := false
	openActive := func(*os.File, string) (*os.File, error) {
		activeCalled = true
		return nil, fmt.Errorf("active open must not run after cancellation")
	}
	file, _, err := openVerifiedLeafWith(ctx, handle, entry("item", content),
		openActive, cancel)
	if ErrorCode(err) != CodeTimeoutExceeded || file != nil || activeCalled {
		t.Fatalf("canceled precheck = %#v, %q, %v, active=%v",
			file, ErrorCode(err), err, activeCalled)
	}
}

func TestReadRejectsExpectedContentAndNamedIdentityDrift(t *testing.T) {
	root := t.TempDir()
	content := []byte("expected")
	writeFile(t, root, "item", content)
	handle := openRoot(t, root)
	defer func() { _ = handle.Close() }()
	bad := entry("item", []byte("different"))
	bad.Bytes = int64(len(content))
	if _, err := Read(context.Background(), handle, []ExpectedEntry{bad}, testLimits()); err == nil {
		t.Fatal("digest mismatch was accepted")
	}
	replace := func() {
		if err := os.Rename(filepath.Join(root, "item"), filepath.Join(root, "old")); err != nil {
			t.Fatal(err)
		}
		writeFile(t, root, "item", content)
	}
	if value, err := readEntryWith(context.Background(), handle, entry("item", content),
		MaxFileBytes, replace); err == nil || value.Content != nil {
		t.Fatalf("named replacement was accepted: %#v, %v", value, err)
	}
}

func TestReadFailureCodesAreContentFreeAndStable(t *testing.T) {
	root := t.TempDir()
	content := []byte("actual")
	writeFile(t, root, "item", content)
	handle := openRoot(t, root)
	defer func() { _ = handle.Close() }()
	expected := entry("item", []byte("wanted"))
	expected.Bytes = int64(len(content))
	_, err := Read(context.Background(), handle, []ExpectedEntry{expected}, testLimits())
	if ErrorCode(err) != CodeContentMismatch {
		t.Fatalf("content failure code = %q, %v", ErrorCode(err), err)
	}
	canceled, cancel := context.WithCancel(context.Background())
	cancel()
	_, err = Read(canceled, handle, []ExpectedEntry{entry("item", content)}, testLimits())
	if ErrorCode(err) != CodeTimeoutExceeded {
		t.Fatalf("canceled failure code = %q, %v", ErrorCode(err), err)
	}
	if _, err = Read(canceled, nil, []ExpectedEntry{entry("item", content)},
		testLimits()); ErrorCode(err) != CodeTimeoutExceeded {
		t.Fatalf("canceled read touched repository before timeout classification: %v", err)
	}
	late, stop := context.WithCancel(context.Background())
	value, err := readEntryWith(late, handle, entry("item", content), MaxFileBytes, stop)
	if ErrorCode(err) != CodeTimeoutExceeded || value.Content != nil {
		t.Fatalf("post-syscall cancellation = %#v, %q, %v", value, ErrorCode(err), err)
	}
}

func TestReadFailsBeforeLeafOnUnsupportedFilesystemAndCancellation(t *testing.T) {
	proc, err := os.Open("/proc")
	if err == nil {
		defer func() { _ = proc.Close() }()
		if _, err := Read(context.Background(), proc,
			[]ExpectedEntry{entry("version", nil)}, testLimits()); err == nil {
			t.Fatal("proc filesystem was accepted")
		}
	}
	root := t.TempDir()
	writeFile(t, root, "item", []byte("x"))
	handle := openRoot(t, root)
	defer func() { _ = handle.Close() }()
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := Read(ctx, handle, []ExpectedEntry{entry("item", []byte("x"))}, testLimits()); err == nil {
		t.Fatal("canceled read was accepted")
	}
}

func TestFilesystemAllowlistIsClosedAndNamed(t *testing.T) {
	for _, magic := range []int64{fsExt, fsXFS, fsBtrfs, fsTmpfs, fsOverlay, fsZFS} {
		if !allowedFilesystem(magic) {
			t.Errorf("documented local filesystem %#x was rejected", magic)
		}
	}
	for _, magic := range []int64{0x65735546, 0x6969, 0x1021997, 0} {
		if allowedFilesystem(magic) {
			t.Errorf("unlisted filesystem %#x was accepted", magic)
		}
	}
}

func TestValidationRejectsPartialOrAmbiguousSets(t *testing.T) {
	valid := entry("a", nil)
	for _, entries := range [][]ExpectedEntry{
		nil, {valid, valid}, {entry("b", nil), valid},
		{{Bytes: 0, ContentSHA256: valid.ContentSHA256, Kind: "symlink", Path: "a"}},
	} {
		if _, err := Read(context.Background(), nil, entries, testLimits()); err == nil {
			t.Fatalf("invalid entries accepted: %#v", entries)
		}
	}
}

func entry(path string, content []byte) ExpectedEntry {
	digest := sha256.Sum256(content)
	return ExpectedEntry{Bytes: int64(len(content)), ContentSHA256: hex.EncodeToString(digest[:]),
		Kind: "regular", Path: path}
}

func testLimits() Limits {
	return Limits{MaxFiles: MaxFiles, MaxFileBytes: MaxFileBytes, MaxTotalBytes: MaxTotalBytes}
}

func openRoot(t *testing.T, path string) *os.File {
	t.Helper()
	file, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	return file
}

func writeFile(t *testing.T, root, relative string, content []byte) {
	t.Helper()
	name := filepath.Join(root, filepath.FromSlash(relative))
	if err := os.MkdirAll(filepath.Dir(name), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(name, content, 0o600); err != nil {
		t.Fatal(err)
	}
}
