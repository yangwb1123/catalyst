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
