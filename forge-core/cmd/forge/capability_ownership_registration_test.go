package main

import (
	"strings"
	"testing"
)

func TestCapabilityOwnershipIsRegisteredAndDocumented(t *testing.T) {
	if _, ok := subcommands["capability-ownership"]; !ok {
		t.Fatal("capability-ownership must be registered")
	}
	usage := captureUsageStderr(t)
	if expected := "forge capability-ownership project --catalog FILE|- --mapping FILE|-"; !strings.Contains(usage, expected) {
		t.Fatalf("usage omits %q:\n%s", expected, usage)
	}
}
