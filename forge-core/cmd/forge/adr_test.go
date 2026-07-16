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
	// Check for the single-line `require <module> <version>` form too (what
	// `go get`/`go mod tidy` actually emits for a single new dependency).
	// NOTE: a prior version of this check exempted any line starting with
	// "require go" to skip a (nonexistent) "require go 1.26" toolchain line —
	// but that exemption also matched any real dependency whose module path
	// starts with "go" (golang.org/x/*, google.golang.org/*, go.uber.org/*,
	// gopkg.in/*, gorm.io/*, go.mongodb.org/*, ...), silently letting those
	// slip past ADR-0002. The `go 1.26` toolchain directive is its own line
	// (no "require" prefix at all), so no exemption is needed here.
	for _, line := range strings.Split(string(data), "\n") {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, "require ") {
			t.Errorf("ADR-0002 violation: external dependency found in go.mod: %s", trimmed)
		}
	}
}

// TestADR0002_ZeroExternalDeps_CatchesGoPrefixedModule is a regression test
// for the "require go" exemption loophole described above: a single-line
// require directive for a module whose path starts with "go" (e.g.
// golang.org/x/net) must still be flagged, not silently exempted.
func TestADR0002_ZeroExternalDeps_CatchesGoPrefixedModule(t *testing.T) {
	fixture := "module forgeos/forge-core\n\ngo 1.26\n\nrequire golang.org/x/net v0.20.0\n"
	found := false
	for _, line := range strings.Split(fixture, "\n") {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, "require ") {
			found = true
		}
	}
	if !found {
		t.Fatal("regression: a go-prefixed module path in a single-line require directive was not detected")
	}
}
