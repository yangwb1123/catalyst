package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestNonRegularRejectionMarkerFailsBeforeWorkAndRemains(t *testing.T) {
	root := t.TempDir()
	wf := rejectableWorkflow()
	marker := rejectionPath(root, wf.Stage)
	if err := os.MkdirAll(marker, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(marker, "keep"), []byte("x"), 0o600); err != nil {
		t.Fatal(err)
	}
	var logs []string
	start, rejected, err := resolveRejectionStartPhase(wf, root, func(line string) { logs = append(logs, line) })
	if err == nil || !strings.Contains(err.Error(), "regular non-symlink") ||
		start != 0 || rejected {
		t.Fatalf("start=%d rejected=%v err=%v, want pre-work alias rejection", start, rejected, err)
	}
	for _, line := range logs {
		if strings.Contains(line, "marker consumed") {
			t.Fatalf("failed deletion was narrated as consumed: %v", logs)
		}
	}
	if _, err := os.Stat(marker); err != nil {
		t.Fatalf("failed marker must remain for retry: %v", err)
	}
}
