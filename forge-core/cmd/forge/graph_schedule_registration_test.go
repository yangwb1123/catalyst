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
