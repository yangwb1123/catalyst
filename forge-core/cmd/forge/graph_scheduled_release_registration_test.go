package main

import (
	"strings"
	"testing"
)

func TestGraphScheduledNodeDispatchAuthorizeIsRegisteredAndDocumented(t *testing.T) {
	if _, ok := subcommands["graph-scheduled-node-dispatch-authorize"]; !ok {
		t.Fatal("graph-scheduled-node-dispatch-authorize must be registered")
	}
	output := captureUsageStderr(t)
	if !strings.Contains(output, "forge graph-scheduled-node-dispatch-authorize --control FILE|-") {
		t.Fatalf("usage omits scheduled-node dispatch authorization:\n%s", output)
	}
}

func TestGraphScheduledReadyNodeDispatchAuthorizeIsRegisteredAndDocumented(t *testing.T) {
	name := "graph-scheduled-ready-node-dispatch-authorize"
	if _, ok := subcommands[name]; !ok {
		t.Fatalf("%s must be registered", name)
	}
	output := captureUsageStderr(t)
	if !strings.Contains(output, "forge "+name+" --control FILE|-") ||
		!strings.Contains(output, "forge "+name+" --protocol-version") {
		t.Fatalf("usage omits scheduled ready-node authorization:\n%s", output)
	}
}
