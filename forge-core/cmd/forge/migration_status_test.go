package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestStatusDisplaysCompletedMigrationOperations(t *testing.T) {
	root := fakeLifecycleRepo(t, "explorer", "mvp")
	code, output := captureChainOutput(t, func() int {
		return cmdMigrate([]string{
			"--to-lifecycle", "production", "--apply", "--root", root,
		})
	})
	if code != 0 {
		t.Fatalf("seed migration exit=%d output:\n%s", code, output)
	}
	text := captureStdout(t, func() { cmdStatus([]string{"--root", root}) })
	if !strings.Contains(text, "migration: pending=false") ||
		!strings.Contains(text, "lifecycle-production-v1") {
		t.Fatalf("status text omitted completed migration:\n%s", text)
	}
	raw := captureStdout(t, func() {
		cmdStatus([]string{"--root", root, "--json"})
	})
	var status statusJSON
	if err := json.Unmarshal([]byte(raw), &status); err != nil {
		t.Fatalf("decode status JSON: %v\n%s", err, raw)
	}
	if status.Migration == nil ||
		status.Migration.Pending ||
		len(status.Migration.Operations) != 1 {
		t.Fatalf("migration JSON = %+v", status.Migration)
	}
}

func TestStatusDisplaysOpaquePendingMigrationRecovery(t *testing.T) {
	root := t.TempDir()
	stateDir := filepath.Join(root, ".forge", "migrations")
	if err := os.MkdirAll(stateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	pending := filepath.Join(stateDir, "pending.v1.json")
	if err := os.WriteFile(pending, nil, 0o640); err != nil {
		t.Fatal(err)
	}
	if err := os.Truncate(pending, 49<<20); err != nil {
		t.Fatal(err)
	}
	text := captureStdout(t, func() { cmdStatus([]string{"--root", root}) })
	for _, want := range []string{
		"migration: pending=true",
		"forge migrate --to-lifecycle production --apply",
		"forge migrate --to engineering --apply",
	} {
		if !strings.Contains(text, want) {
			t.Fatalf("pending migration status omitted %q:\n%s", want, text)
		}
	}
}
