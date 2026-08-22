package main

import (
	"strings"
	"testing"
)

func TestGraphSnapshotIsRegisteredAndDocumented(t *testing.T) {
	if _, ok := subcommands["graph-snapshot"]; !ok {
		t.Fatal("graph-snapshot must be registered")
	}
	if output := captureUsageStderr(t); !strings.Contains(output,
		"forge graph-snapshot --project-id ID --graph-sha256 HEX --run-id ID [--profile PROFILE] [--input FILE|-]") {
		t.Fatalf("usage omits graph-snapshot:\n%s", output)
	}
}
