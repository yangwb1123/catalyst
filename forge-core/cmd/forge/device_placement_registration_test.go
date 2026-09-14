package main

import (
	"strings"
	"testing"
)

func TestDevicePlacementDryRunIsRegisteredAndDocumented(t *testing.T) {
	if _, ok := subcommands["device-placement"]; !ok {
		t.Fatal("device-placement must be registered")
	}
	usage := captureUsageStderr(t)
	if !strings.Contains(usage, "forge device-placement dry-run --input FILE|-") {
		t.Fatalf("usage omits offline dry-run syntax:\n%s", usage)
	}
}
