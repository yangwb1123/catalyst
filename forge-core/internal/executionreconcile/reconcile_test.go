package executionreconcile

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

type contractFixture struct {
	SchemaVersion  string                `json:"schema_version"`
	EvaluationMode string                `json:"evaluation_mode"`
	Authority      Authority             `json:"authority"`
	Cases          []contractFixtureCase `json:"cases"`
}

type contractFixtureCase struct {
	Name     string           `json:"name"`
	Input    Input            `json:"input"`
	Expected contractExpected `json:"expected"`
}

type contractExpected struct {
	Accepted               bool   `json:"accepted"`
	NextObservation        string `json:"next_observation"`
	LeaseActive            bool   `json:"lease_active"`
	TerminalObserved       bool   `json:"terminal_observed"`
	TerminalDisposition    string `json:"terminal_disposition"`
	TerminalStateAligned   bool   `json:"terminal_state_aligned"`
	ReconciliationRequired bool   `json:"reconciliation_required"`
}

func readFixture(t *testing.T) contractFixture {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "forge-execution-reconciliation-observation-v1.json")
	bytes, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture contractFixture
	decoder := json.NewDecoder(strings.NewReader(string(bytes)))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode fixture: %v", err)
	}
	if fixture.SchemaVersion != SchemaVersion || fixture.EvaluationMode != EvaluationMode || fixture.Authority != (Authority{}) {
		t.Fatalf("invalid fixture envelope: %#v", fixture)
	}
	return fixture
}

func TestContractFixture(t *testing.T) {
	fixture := readFixture(t)
	if len(fixture.Cases) != 6 {
		t.Fatalf("cases=%d, want 6", len(fixture.Cases))
	}
	for _, testCase := range fixture.Cases {
		observation, err := Observe(testCase.Input)
		if !testCase.Expected.Accepted {
			if err == nil {
				t.Fatalf("%s accepted unexpectedly: %#v", testCase.Name, observation)
			}
			continue
		}
		if err != nil {
			t.Fatalf("%s: %v", testCase.Name, err)
		}
		if err := observation.Validate(); err != nil {
			t.Fatalf("%s output invalid: %v", testCase.Name, err)
		}
		want := testCase.Expected
		if observation.NextObservation != want.NextObservation ||
			observation.LeaseActive != want.LeaseActive ||
			observation.TerminalObserved != want.TerminalObserved ||
			observation.TerminalDisposition != want.TerminalDisposition ||
			observation.TerminalStateAligned != want.TerminalStateAligned ||
			observation.ReconciliationRequired != want.ReconciliationRequired {
			t.Fatalf("%s observation=%#v expected=%#v", testCase.Name, observation, want)
		}
	}
}

func TestObservationRejectsAuthorityAndUnsafeState(t *testing.T) {
	fixture := readFixture(t)
	base, err := Observe(fixture.Cases[0].Input)
	if err != nil {
		t.Fatal(err)
	}
	mutated := base
	mutated.Authority.DispatchPerformed = true
	if err := mutated.Validate(); err == nil {
		t.Fatal("authority mutation accepted")
	}
	mutated = base
	mutated.AutomaticRetry = true
	if err := mutated.Validate(); err == nil {
		t.Fatal("automatic retry accepted")
	}
	mutated = base
	mutated.NextObservation = "terminal_completed"
	if err := mutated.Validate(); err == nil {
		t.Fatal("classification mutation accepted")
	}
}

func TestObservationRejectsTerminalAfterLeaseExpiryAndFutureReceipt(t *testing.T) {
	fixture := readFixture(t)
	input := fixture.Cases[2].Input
	input.ObservedAtMS = input.Lease.ExpiresAtMS + 1
	input.Terminal.ObservedAtMS = input.Lease.ExpiresAtMS + 1
	if _, err := Observe(input); err == nil {
		t.Fatal("receipt after lease expiry accepted")
	}
	input = fixture.Cases[2].Input
	input.Terminal.ObservedAtMS = input.ObservedAtMS + 1
	if _, err := Observe(input); err == nil {
		t.Fatal("future terminal receipt accepted")
	}
}

func TestObserveRejectsUnsafeLeaseEpoch(t *testing.T) {
	fixture := readFixture(t)
	input := fixture.Cases[0].Input
	input.Lease.Epoch = uint64(maxSafeInteger) + 1
	if _, err := Observe(input); err == nil {
		t.Fatal("unsafe lease epoch accepted")
	}
}
