package localcommandobservationproducer

import (
	"reflect"
	"strings"
	"testing"
)

func TestCommandForClassFreezesExactArgv(t *testing.T) {
	tests := map[string][]string{
		CommandGate:     {"node", "harness/gate.mjs"},
		CommandCheck:    {"python3", "harness/check.py", "."},
		CommandAccept:   {"node", "harness/acceptance.mjs"},
		CommandProbeAll: {"node", "harness/acceptance.mjs", "--json"},
	}
	for class, want := range tests {
		profile, err := commandForClass(class)
		if err != nil || !reflect.DeepEqual(profile.Argv, want) || profile.EvidenceType != "gate_result" {
			t.Fatalf("commandForClass(%q) = %#v, %v; want %v", class, profile, err, want)
		}
	}
	if _, err := commandForClass("arbitrary"); err == nil {
		t.Fatal("arbitrary command class must be rejected")
	}
}

func TestEnvironmentSnapshotScrubsSecretsAndSorts(t *testing.T) {
	parent := []string{
		"ZETA=last", "OPENAI_API_KEY=must-not-hash", "PATH=/usr/bin:/bin",
		"AWS_REGION=must-not-pass", "LANG=C", "HTTPS_PROXY=must-not-pass",
		"ALPHA=first", "GITHUB_TOKEN=must-not-pass",
	}
	manifest, digest, child, err := environmentSnapshot(parent)
	if err != nil {
		t.Fatal(err)
	}
	want := []EnvironmentVariable{
		{Name: "ALPHA", Value: "first"}, {Name: "LANG", Value: "C"},
		{Name: "PATH", Value: "/usr/bin:/bin"}, {Name: "ZETA", Value: "last"},
	}
	if !reflect.DeepEqual(manifest.Variables, want) || len(digest) != 64 {
		t.Fatalf("environment manifest = %#v digest=%q", manifest, digest)
	}
	if !reflect.DeepEqual(child, []string{"ALPHA=first", "LANG=C", "PATH=/usr/bin:/bin", "ZETA=last"}) {
		t.Fatalf("child environment = %v", child)
	}
	encoded, _, err := digestManifest(environmentDigestDomain, manifest)
	if err != nil || strings.Contains(string(encoded), "must-not") {
		t.Fatalf("secret leaked into manifest: %s (%v)", encoded, err)
	}
}

func TestEnvironmentSnapshotRejectsAmbiguousOrUnsafeInput(t *testing.T) {
	tests := []struct {
		name string
		env  []string
	}{
		{"missing path", []string{"LANG=C"}},
		{"duplicate", []string{"PATH=/bin", "PATH=/usr/bin"}},
		{"malformed", []string{"PATH=/bin", "NO_EQUALS"}},
		{"control", []string{"PATH=/bin", "SAFE=bad\nvalue"}},
		{"empty path", []string{"PATH="}},
		{"relative path", []string{"PATH=relative"}},
		{"later relative path", []string{"PATH=/bin:relative"}},
		{"empty path component", []string{"PATH=/bin:"}},
		{"nonnormalized path", []string{"PATH=/usr/bin/../bin"}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if _, _, _, err := environmentSnapshot(test.env); err == nil {
				t.Fatalf("unsafe environment accepted: %v", test.env)
			}
		})
	}
}

func TestEnvironmentDigestChangesOnlyWithRetainedValues(t *testing.T) {
	_, first, _, err := environmentSnapshot([]string{"PATH=/bin", "SAFE=one", "TOKEN=a"})
	if err != nil {
		t.Fatal(err)
	}
	_, secretChanged, _, _ := environmentSnapshot([]string{"PATH=/bin", "SAFE=one", "TOKEN=b"})
	_, safeChanged, _, _ := environmentSnapshot([]string{"PATH=/bin", "SAFE=two", "TOKEN=b"})
	if first != secretChanged || first == safeChanged {
		t.Fatalf("environment identity boundary first=%s secret=%s safe=%s", first, secretChanged, safeChanged)
	}
}
