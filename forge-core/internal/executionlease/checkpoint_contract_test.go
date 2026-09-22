package executionlease

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"
	"testing"
)

type checkpointFixture struct {
	SchemaVersion  string                  `json:"schema_version"`
	EvaluationMode string                  `json:"evaluation_mode"`
	Authority      checkpointAuthority     `json:"authority"`
	Cases          []checkpointFixtureCase `json:"cases"`
}

type checkpointAuthority struct {
	LeaseIssued         bool `json:"lease_issued"`
	TerminalPersisted   bool `json:"terminal_persisted"`
	ExecutionAuthorized bool `json:"execution_authorized"`
	DispatchPerformed   bool `json:"dispatch_performed"`
	AuditPublished      bool `json:"audit_published"`
}

type checkpointFixtureCase struct {
	Name       string             `json:"name"`
	Checkpoint LeaseCheckpoint    `json:"checkpoint"`
	Expected   checkpointExpected `json:"expected"`
}

type checkpointExpected struct {
	Accepted  bool   `json:"accepted"`
	Error     string `json:"error"`
	Terminal  bool   `json:"terminal"`
	Uncertain bool   `json:"uncertain"`
}

func TestLeaseCheckpointContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE")
	if path == "" {
		path = filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "forge-execution-lease-checkpoint-v1.json")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var fixture checkpointFixture
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode lease checkpoint fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("lease checkpoint fixture has trailing JSON: %v", err)
	}
	if fixture.SchemaVersion != ExecutionLeaseCheckpointSchemaVersion ||
		fixture.EvaluationMode != ExecutionLeaseCheckpointEvaluationMode ||
		fixture.Authority != (checkpointAuthority{}) || len(fixture.Cases) != 4 {
		t.Fatalf("invalid lease checkpoint fixture envelope: %#v", fixture)
	}
	for _, testCase := range fixture.Cases {
		t.Run(testCase.Name, func(t *testing.T) {
			state, err := RestoreCheckpoint(testCase.Checkpoint)
			if testCase.Expected.Accepted {
				if err != nil {
					t.Fatalf("restore error=%v", err)
				}
				if (state.terminal != nil) != testCase.Expected.Terminal {
					t.Fatalf("terminal=%v, want %v", state.terminal != nil, testCase.Expected.Terminal)
				}
				if state.terminal != nil && state.terminal.Disposition.IsUncertain() != testCase.Expected.Uncertain {
					t.Fatalf("uncertain=%v, want %v", state.terminal.Disposition.IsUncertain(), testCase.Expected.Uncertain)
				}
				return
			}
			if err == nil || err.Error() != testCase.Expected.Error {
				t.Fatalf("restore error=%v, want %q", err, testCase.Expected.Error)
			}
		})
	}
}

func TestLeaseCheckpointRoundTripsDefensivelyAndRejectsMutation(t *testing.T) {
	grant, err := Issue("attempt-1", "runner-1", 1, "fence-1", 100, 10_000)
	if err != nil {
		t.Fatal(err)
	}
	state, err := NewState(grant)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := state.SubmitTerminal(grant.Proof(), TerminalDisposition{Kind: "uncertain", Reason: "effect boundary"}, 200); err != nil {
		t.Fatal(err)
	}
	checkpoint := state.Checkpoint()
	restored, err := RestoreCheckpoint(checkpoint)
	if err != nil {
		t.Fatal(err)
	}
	if restored.Checkpoint().Terminal == nil || !restored.Checkpoint().Terminal.Disposition.IsUncertain() {
		t.Fatalf("restored checkpoint=%#v", restored.Checkpoint())
	}
	checkpoint.Grant.FencingToken = "mutated"
	if state.Grant().FencingToken != "fence-1" {
		t.Fatal("checkpoint mutation changed live state")
	}
	if restored.Checkpoint().Grant.FencingToken != "fence-1" {
		t.Fatal("checkpoint mutation changed restored state")
	}
	mutated := restored.Checkpoint()
	mutated.Terminal.Proof.TargetID = "foreign-target"
	if _, err := RestoreCheckpoint(mutated); err != ErrInvalidCheckpoint {
		t.Fatalf("foreign terminal proof error=%v", err)
	}
}

func TestLeaseCheckpointRejectsSchemaAuthorityAndInactiveReceipt(t *testing.T) {
	grant, err := Issue("attempt-1", "runner-1", 1, "fence-1", 100, 10_000)
	if err != nil {
		t.Fatal(err)
	}
	state, err := NewState(grant)
	if err != nil {
		t.Fatal(err)
	}
	checkpoint := state.Checkpoint()
	checkpoint.SchemaVersion = "other"
	if _, err := RestoreCheckpoint(checkpoint); err != ErrInvalidCheckpoint {
		t.Fatalf("schema error=%v", err)
	}
	checkpoint = state.Checkpoint()
	checkpoint.Terminal = &TerminalReceipt{
		V: ExecutionLeaseABIVersion, Proof: grant.Proof(),
		Disposition: TerminalDisposition{Kind: "failed", Reason: "late"}, ObservedAtMS: grant.ExpiresAtMS,
	}
	if _, err := RestoreCheckpoint(checkpoint); err != ErrInvalidCheckpoint {
		t.Fatalf("inactive receipt error=%v", err)
	}
}
