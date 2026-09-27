package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"testing"
)

type runnerCommandDigestVectorsFixture struct {
	SchemaVersion string                      `json:"schema_version"`
	DigestDomain  string                      `json:"digest_domain"`
	Vectors       []runnerCommandDigestVector `json:"vectors"`
}

type runnerCommandDigestVector struct {
	Name          string                `json:"name"`
	Command       RunnerTerminalCommand `json:"command"`
	CommandSHA256 string                `json:"command_sha256"`
}

func TestRunnerCommandDigestVectorsContract(t *testing.T) {
	path := os.Getenv("FORGE_RUNNER_COMMAND_DIGEST_FIXTURE")
	if path == "" {
		t.Skip("FORGE_RUNNER_COMMAND_DIGEST_FIXTURE is set by the cross-repository contract test")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := rejectDuplicateFields(encoded); err != nil {
		t.Fatalf("digest vector fixture has duplicate fields: %v", err)
	}
	var fixture runnerCommandDigestVectorsFixture
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode digest vector fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		t.Fatalf("digest vector fixture has trailing JSON: %v", err)
	}
	if fixture.SchemaVersion != "forge.runner-command-digest/v1" ||
		fixture.DigestDomain != "forge.runtime.runner-command.v1" || len(fixture.Vectors) != 3 {
		t.Fatalf("unexpected digest vector envelope: %#v", fixture)
	}
	seen := make(map[string]struct{}, len(fixture.Vectors))
	for _, vector := range fixture.Vectors {
		if vector.Name == "" {
			t.Fatal("digest vector name is empty")
		}
		if _, exists := seen[vector.Name]; exists {
			t.Fatalf("duplicate digest vector %q", vector.Name)
		}
		seen[vector.Name] = struct{}{}
		actual, err := vector.Command.CommandSHA256()
		if err != nil {
			t.Fatalf("compute digest for %q: %v", vector.Name, err)
		}
		if actual != vector.CommandSHA256 {
			t.Fatalf("digest vector %q = %s, want %s", vector.Name, actual, vector.CommandSHA256)
		}
	}
}

func TestRunnerCommandDigestVectorsRejectDuplicateKeys(t *testing.T) {
	path := os.Getenv("FORGE_RUNNER_COMMAND_DIGEST_FIXTURE")
	if path == "" {
		t.Skip("FORGE_RUNNER_COMMAND_DIGEST_FIXTURE is set by the cross-repository contract test")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	mutated := bytes.Replace(encoded, []byte(`"schema_version": "forge.runner-command-digest/v1",`), []byte(`"schema_version": "forge.runner-command-digest/v1", "schema_version": "forge.runner-command-digest/v1",`), 1)
	if bytes.Equal(mutated, encoded) || rejectDuplicateFields(mutated) == nil {
		t.Fatal("duplicate digest vector field was accepted")
	}
}
