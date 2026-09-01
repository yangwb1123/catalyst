//go:build linux

package appserver

import (
	"strings"
	"testing"
)

func TestUIDMapTrustsOverflowOwnerOnlyInNonInitialNamespace(t *testing.T) {
	const overflowUID = 65534
	tests := []struct {
		name, mapping string
		owner         int
		want          bool
	}{
		{"initial", "0 0 4294967295\n", overflowUID, false},
		{"rootless", "1000 1000 1\n", overflowUID, true},
		{"multi-range-unmapped", "0 1000 1\n1 200000 60000\n", overflowUID, true},
		{"mapped-overflow", "65534 200000 1\n", overflowUID, false},
		{"range-includes-overflow", "1 200000 65536\n", overflowUID, false},
		{"mapped-owner", "1000 1000 1\n", 1000, false},
		{"malformed", "not a map\n", overflowUID, false},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			got := uidMapTrustsUnmappedOwner(test.owner, strings.NewReader(test.mapping))
			if got != test.want {
				t.Fatalf("trust = %v, want %v", got, test.want)
			}
		})
	}
}

func TestConfiguredOverflowUIDIsBoundedAndNumeric(t *testing.T) {
	overflow, present := configuredOverflowUID()
	if !present || overflow < 0 {
		t.Fatalf("configured overflow UID = %d, %v", overflow, present)
	}
}

func TestOverflowUIDParserFailsClosed(t *testing.T) {
	tests := []struct {
		input   string
		want    int
		present bool
	}{
		{"65534\n", 65534, true},
		{" 42 \n", 42, true},
		{"", 0, false},
		{"not-a-uid\n", 0, false},
		{strings.Repeat("1", 33), 0, false},
	}
	for _, test := range tests {
		got, present := parseOverflowUID(strings.NewReader(test.input))
		if got != test.want || present != test.present {
			t.Fatalf("parse %q = %d, %v", test.input, got, present)
		}
	}
}
