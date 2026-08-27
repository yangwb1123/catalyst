package main

import (
	"strings"
	"testing"
)

func TestGraphExecutionScheduleIsRegisteredAndDocumented(t *testing.T) {
	if _, ok := subcommands["graph-execution-schedule"]; !ok {
		t.Fatal("graph-execution-schedule must be registered")
	}
	if output := captureUsageStderr(t); !strings.Contains(output,
		"forge graph-execution-schedule --control FILE|-") {
		t.Fatalf("usage omits graph-execution-schedule:\n%s", output)
	}
}

func TestGraphScheduledNodeContractIsRegisteredAndDocumented(t *testing.T) {
	if _, ok := subcommands["graph-scheduled-node-contract"]; !ok {
		t.Fatal("graph-scheduled-node-contract must be registered")
	}
	output := captureUsageStderr(t)
	if !strings.Contains(output, "forge graph-scheduled-node-contract --control FILE|-") ||
		!strings.Contains(output, "--schedule-sha256 SHA256") ||
		!strings.Contains(output, "forge graph-scheduled-node-contract --protocol-version") {
		t.Fatalf("usage omits scheduled-node contract command:\n%s", output)
	}
}
