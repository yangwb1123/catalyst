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
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// TestADR0002_ZeroExternalDeps verifies ADR-0002's core decision:
// forge-core (Go runtime) has ZERO external Go dependencies — only the
// Go standard library. This is enforced by checking that go.mod has no
// `require (` block. Any external dependency added to go.mod will fail
// this test and must be justified against ADR-0002.
func TestADR0002_ZeroExternalDeps(t *testing.T) {
	// go.mod is at forge-core/go.mod, one level up from cmd/forge/.
	gm := filepath.Join("..", "..", "go.mod")
	data, err := os.ReadFile(gm)
	if err != nil {
		// If the test runs from a different working directory, try absolute.
		// This is a best-effort path; CI runs from forge-core/ so relative works.
		gm = filepath.Join("forge-core", "go.mod")
		data, err = os.ReadFile(gm)
		if err != nil {
			t.Skipf("ADR-0002: cannot read go.mod (%v) — skipping zero-dependency check", err)
			return
		}
	}
	if bytes.Contains(data, []byte("require (")) {
		t.Error("ADR-0002 violation: forge-core must have zero external Go dependencies (go.mod has require block)")
	}
	// Check that no indirect dependencies slipped in via toolchain.
	for _, line := range strings.Split(string(data), "\n") {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, "require") && !strings.HasPrefix(trimmed, "require go") {
			t.Errorf("ADR-0002 violation: external dependency found in go.mod: %s", trimmed)
		}
	}
}
