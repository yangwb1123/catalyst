package main

import (
	"strings"
	"testing"
)

func TestGraphNodeDispatchAuthorizeIsRegisteredAndDocumented(t *testing.T) {
	if _, ok := subcommands["graph-node-dispatch-authorize"]; !ok {
		t.Fatal("graph-node-dispatch-authorize must be registered")
	}
	if output := captureUsageStderr(t); !strings.Contains(output, "forge graph-node-dispatch-authorize --control FILE|-") {
		t.Fatalf("usage omits graph-node-dispatch-authorize:\n%s", output)
	}
}
