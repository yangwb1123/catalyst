package main

import (
	"strings"
	"testing"
)

func TestGraphScheduledReconcileIsRegisteredAndDocumented(t *testing.T) {
	if _, ok := subcommands["graph-scheduled-reconcile"]; !ok {
		t.Fatal("graph-scheduled-reconcile must be registered")
	}
	output := captureUsageStderr(t)
	if !strings.Contains(output, "forge graph-scheduled-reconcile --snapshot FILE|-") ||
		!strings.Contains(output, "forge graph-scheduled-reconcile --protocol-version") {
		t.Fatalf("usage omits scheduled reconcile command:\n%s", output)
	}
}
