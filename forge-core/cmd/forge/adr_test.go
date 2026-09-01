// Package adr_test holds automated tests that verify ForgeOS Architecture
// Decision Records (ADRs) are still being followed. Each ADR from docs/adr/
// should have at least one test here that asserts its key decision is still
// true in the codebase — otherwise a decision can silently decay without
// anyone noticing (eighth-wave-adr-decay.md §方向1).
//
// These tests run as part of `go test ./...` and are subject to the same
// CI enforcement as any other Go test.
package main

import (
	"os"
	"os/exec"
	"path/filepath"
	"testing"

	"forgeos/forge-core/internal/doctor"
)

// TestControlStoreDependencyPolicy verifies the reviewed SQLite closure and
// sole direct-import boundary. It replaces the historical zero-module check:
// ADR-0002 specifies a Go core and even anticipates procured infrastructure.
func TestControlStoreDependencyPolicy(t *testing.T) {
	forgeDir := filepath.Clean(filepath.Join("..", ".."))
	if err := doctor.CheckModuleDependencyPolicy(forgeDir); err != nil {
		t.Fatal(err)
	}
}

// TestADR0002_ForgeBuildsWithoutCGO protects the static Go CLI target while
// allowing the reviewed CGo-free App Server storage driver.
func TestADR0002_ForgeBuildsWithoutCGO(t *testing.T) {
	forgeDir := filepath.Clean(filepath.Join("..", ".."))
	output := filepath.Join(t.TempDir(), "forge")
	cmd := exec.Command("go", "build", "-o", output, "./cmd/forge")
	cmd.Dir = forgeDir
	cmd.Env = append(os.Environ(), "CGO_ENABLED=0")
	if combined, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("ADR-0002 Go CLI must build with CGO disabled: %v\n%s", err, combined)
	}
}
