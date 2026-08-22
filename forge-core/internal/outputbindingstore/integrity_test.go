package outputbindingstore

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestAppendRejectsMalformedCompleteTailWithoutMutation(t *testing.T) {
	root := t.TempDir()
	store := New(root)
	if _, err := store.Append(receiptDraft(t, "first")); err != nil {
		t.Fatal(err)
	}
	path := Path(root)
	file, err := os.OpenFile(path, os.O_WRONLY|os.O_APPEND, 0)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = file.WriteString("{}\n"); err != nil {
		_ = file.Close()
		t.Fatal(err)
	}
	if err = file.Close(); err != nil {
		t.Fatal(err)
	}
	before, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = store.Append(receiptDraft(t, "blocked")); err == nil ||
		!strings.Contains(err.Error(), "ledger line 2") {
		t.Fatalf("malformed tail error = %v", err)
	}
	assertLedgerBytes(t, path, before)
}

func TestAppendRequiresRegularLedgerLeaf(t *testing.T) {
	root := t.TempDir()
	if err := os.Mkdir(filepath.Join(root, ".forge"), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(Path(root), 0o700); err != nil {
		t.Fatal(err)
	}
	if _, err := New(root).Append(receiptDraft(t, "blocked")); err == nil ||
		!strings.Contains(err.Error(), "regular") {
		t.Fatalf("non-regular ledger error = %v", err)
	}
	info, err := os.Lstat(Path(root))
	if err != nil || !info.IsDir() {
		t.Fatalf("ledger directory changed: %v, %v", info, err)
	}
}

func TestLoadIsSideEffectFreeForExistingPermissions(t *testing.T) {
	root := t.TempDir()
	store := New(root)
	if _, err := store.Append(receiptDraft(t, "first")); err != nil {
		t.Fatal(err)
	}
	path := Path(root)
	if err := os.Chmod(path, 0o640); err != nil {
		t.Fatal(err)
	}
	before, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := store.Load(); err != nil {
		t.Fatal(err)
	}
	assertLedgerImage(t, path, before, 0o640)
}
