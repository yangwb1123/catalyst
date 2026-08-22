package main

import (
	"strings"
	"testing"
)

func TestProjectSnapshotIsRegisteredAndDocumented(t *testing.T) {
	if _, ok := subcommands["project-snapshot"]; !ok {
		t.Fatal("project-snapshot must be registered")
	}
	usage := captureUsageStderr(t)
	expected := "forge project-snapshot capture --project-id ID --run-id ID --root DIR"
	if !strings.Contains(usage, expected) {
		t.Fatalf("usage omits %q:\n%s", expected, usage)
	}
}
