//go:build unix

package outputbindingstore

import (
	"os"
	"path/filepath"
	"testing"
)

func TestAppendRejectsHardLinkedLedgerWithoutChangingEitherName(t *testing.T) {
	root := t.TempDir()
	if err := os.Mkdir(filepath.Join(root, ".forge"), 0o700); err != nil {
		t.Fatal(err)
	}
	original := filepath.Join(t.TempDir(), "original-ledger")
	want := []byte("outside-sentinel")
	if err := os.WriteFile(original, want, 0o640); err != nil {
		t.Fatal(err)
	}
	if err := os.Link(original, Path(root)); err != nil {
		t.Fatal(err)
	}
	if _, err := New(root).Append(receiptDraft(t, "blocked")); err == nil {
		t.Fatal("hard-linked ledger was accepted")
	}
	assertLedgerImage(t, original, want, 0o640)
	assertLedgerImage(t, Path(root), want, 0o640)
}
