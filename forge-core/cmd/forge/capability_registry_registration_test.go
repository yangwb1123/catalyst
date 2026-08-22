package main

import (
	"strings"
	"testing"
)

func TestCapabilityRegistryIsRegisteredAndDocumented(t *testing.T) {
	if _, ok := subcommands["capability-registry"]; !ok {
		t.Fatal("capability-registry must be registered")
	}
	usage := captureUsageStderr(t)
	for _, expected := range []string{
		"forge capability-registry validate --registry FILE|-",
		"forge capability-registry resolve --registry FILE|- --request FILE|-",
	} {
		if !strings.Contains(usage, expected) {
			t.Fatalf("usage omits %q:\n%s", expected, usage)
		}
	}
}
