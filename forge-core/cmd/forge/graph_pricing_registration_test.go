package main

import (
	"strings"
	"testing"
)

func TestGraphNodePricingSnapshotIsRegisteredAndDocumented(t *testing.T) {
	if _, ok := subcommands["graph-node-pricing-snapshot"]; !ok {
		t.Fatal("graph-node-pricing-snapshot must be registered")
	}
	usage := captureUsageStderr(t)
	if !strings.Contains(usage, "forge graph-node-pricing-snapshot --model MODEL") {
		t.Fatalf("usage omits graph-node-pricing-snapshot:\n%s", usage)
	}
}
