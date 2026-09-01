//go:build !linux || android

package appserver

import (
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestUnsupportedLockFailsBeforeStateMutation(t *testing.T) {
	stateDir := filepath.Join(t.TempDir(), "state")
	if _, err := acquireInstanceLock(stateDir); err == nil {
		t.Fatal("unsupported platform acquired an instance lock")
	}
	if _, err := os.Lstat(stateDir); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("unsupported lock mutated state: %v", err)
	}
}
