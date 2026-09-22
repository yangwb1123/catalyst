package devicefabricgate

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func acceptedManifest() Manifest {
	request := acceptedInventoryRequest()
	return Manifest{
		SchemaVersion: ActivationManifestSchemaVersion,
		Mode:          request.Mode,
		ADR0039:       request.ADR0039,
		ADR0113:       request.ADR0113,
		ADR0114:       request.ADR0114,
		P4:            request.P4,
		Evidence:      request.Evidence,
	}
}

func TestParseManifestRoundTripsAcceptedInventoryRequest(t *testing.T) {
	data, err := json.Marshal(acceptedManifest())
	if err != nil {
		t.Fatal(err)
	}
	manifest, err := ParseManifest(data)
	if err != nil {
		t.Fatal(err)
	}
	decision := Evaluate(manifest.Request())
	if !decision.Allowed || decision.Mode != ModeInventory || len(decision.Reasons) != 0 {
		t.Fatalf("parsed accepted inventory = %#v", decision)
	}
}

func TestParseManifestKeepsOffDefaultWithoutDecisionMetadata(t *testing.T) {
	manifest := Manifest{SchemaVersion: ActivationManifestSchemaVersion, Mode: ModeOff}
	data, err := json.Marshal(manifest)
	if err != nil {
		t.Fatal(err)
	}
	// The schema remains exact even for OFF; callers do not need to create a
	// manifest at all for the zero-value default, but an explicit OFF file is
	// still deterministic and reviewable.
	parsed, err := ParseManifest(data)
	if err != nil {
		t.Fatal(err)
	}
	if got := Evaluate(parsed.Request()); !got.Allowed || got.Mode != ModeOff {
		t.Fatalf("parsed off manifest = %#v", got)
	}
}

func TestParseManifestRejectsUnknownMissingAndDuplicateFields(t *testing.T) {
	valid, err := json.Marshal(acceptedManifest())
	if err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name string
		data string
		want string
	}{
		{
			name: "unknown top-level",
			data: strings.TrimSuffix(string(valid), "}") + `,"unknown":true}`,
			want: "unknown field",
		},
		{
			name: "missing evidence",
			data: strings.Replace(string(valid), `,"evidence":{`, `,"evidence":{`, 1),
			want: "",
		},
		{
			name: "duplicate nested status",
			data: strings.Replace(string(valid), `"status":"accepted"`, `"status":"accepted","status":"proposed"`, 1),
			want: "duplicate",
		},
	}
	// Build the missing-field case as a decoded map so the test does not rely
	// on field ordering from encoding/json.
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(valid, &fields); err != nil {
		t.Fatal(err)
	}
	delete(fields, "evidence")
	missing, err := json.Marshal(fields)
	if err != nil {
		t.Fatal(err)
	}
	tests[1].data = string(missing)
	tests[1].want = "missing field"

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if _, err := ParseManifest([]byte(test.data)); err == nil || !strings.Contains(err.Error(), test.want) {
				t.Fatalf("ParseManifest error=%v, want substring %q", err, test.want)
			}
		})
	}
}

func TestLoadManifestFileRequiresPrivateRegularFile(t *testing.T) {
	root := t.TempDir()
	if err := os.Chmod(root, 0o700); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "activation.json")
	data, err := json.Marshal(acceptedManifest())
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, data, 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := LoadManifestFile(path); err == nil || !strings.Contains(err.Error(), "must not be group/world readable") {
		t.Fatalf("public manifest error=%v", err)
	}
	if err := os.Chmod(path, 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := LoadManifestFile(path); err != nil {
		t.Fatalf("private manifest rejected: %v", err)
	}
	link := filepath.Join(root, "activation-link.json")
	if err := os.Symlink(path, link); err != nil {
		t.Fatal(err)
	}
	if _, err := LoadManifestFile(link); err == nil || !strings.Contains(err.Error(), "regular non-symlink") {
		t.Fatalf("symlink manifest error=%v", err)
	}
}

func TestLoadManifestFileRequiresPrivateParent(t *testing.T) {
	root := t.TempDir()
	parent := filepath.Join(root, "public-parent")
	if err := os.Mkdir(parent, 0o755); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(parent, "activation.json")
	data, err := json.Marshal(acceptedManifest())
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := LoadManifestFile(path); err == nil || !strings.Contains(err.Error(), "parent must be private") {
		t.Fatalf("public parent error=%v", err)
	}
}
