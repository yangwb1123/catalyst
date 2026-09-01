//go:build windows

package domain

import (
	"os"
	"path/filepath"
	"syscall"
	"testing"
)

func sourceLinkCount(path string, _ os.FileInfo) (uint64, bool) {
	name, err := syscall.UTF16PtrFromString(path)
	if err != nil {
		return 0, false
	}
	handle, err := syscall.CreateFile(
		name, 0,
		syscall.FILE_SHARE_READ|syscall.FILE_SHARE_WRITE|syscall.FILE_SHARE_DELETE,
		nil, syscall.OPEN_EXISTING, syscall.FILE_FLAG_OPEN_REPARSE_POINT, 0,
	)
	if err != nil {
		return 0, false
	}
	defer syscall.CloseHandle(handle)
	var metadata syscall.ByHandleFileInformation
	if err := syscall.GetFileInformationByHandle(handle, &metadata); err != nil {
		return 0, false
	}
	return uint64(metadata.NumberOfLinks), true
}

func TestWindowsSourceLinkCountUsesHandleMetadata(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "source.go")
	if err := os.WriteFile(path, []byte("package fixture\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	info, err := os.Lstat(path)
	if err != nil {
		t.Fatal(err)
	}
	if links, ok := sourceLinkCount(path, info); !ok || links != 1 {
		t.Fatalf("regular source link count = %d, %v", links, ok)
	}
	alias := filepath.Join(root, "alias.go")
	if err := os.Link(path, alias); err != nil {
		t.Skipf("hard links are unavailable: %v", err)
	}
	info, err = os.Lstat(path)
	if err != nil {
		t.Fatal(err)
	}
	if links, ok := sourceLinkCount(path, info); !ok || links != 2 {
		t.Fatalf("hard-linked source link count = %d, %v", links, ok)
	}
}
