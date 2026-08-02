package main

import (
	"strings"
	"testing"
)

func TestGraphNodeTerminalReceiptIsRegisteredAndDocumented(t *testing.T) {
	if _, ok := subcommands["graph-node-terminal-receipt"]; !ok {
		t.Fatal("graph-node-terminal-receipt must be registered")
	}
	usage := captureUsageStderr(t)
	if !strings.Contains(usage, "forge graph-node-terminal-receipt --control FILE|-") ||
		!strings.Contains(usage, "forge graph-node-terminal-receipt --protocol-version") {
		t.Fatalf("usage omits graph-node-terminal-receipt:\n%s", usage)
	}
}
