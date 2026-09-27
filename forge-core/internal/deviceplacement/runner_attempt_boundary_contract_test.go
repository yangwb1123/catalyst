package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"testing"
)

func TestRunnerAttemptBoundaryContractFixture(t *testing.T) {
	fixture := readRunnerAttemptBoundaryFixture(t)
	if err := fixture.Validate(); err != nil {
		t.Fatalf("invalid Runner Attempt boundary fixture: %v", err)
	}
	expected, err := ObserveRunnerAttemptBoundary(RunnerAttemptBoundaryRequest{
		Boundary:   observedRunnerExecutionBoundary(t, runnerExecutionBoundaryRequestForAttemptBoundary(t)),
		Transition: "begin_starting",
	})
	if err != nil {
		t.Fatalf("observe Runner Attempt boundary: %v", err)
	}
	if err := expected.Validate(); err != nil {
		t.Fatalf("generated Runner Attempt boundary is invalid: %v", err)
	}
	if !jsonEqual(fixture, expected) {
		want, _ := json.Marshal(expected)
		got, _ := json.Marshal(fixture)
		t.Fatalf("generated Runner Attempt boundary differs from fixture:\nwant %s\ngot  %s", want, got)
	}
}

func TestRunnerAttemptBoundaryContractFixtureRejectsWireDrift(t *testing.T) {
	path := os.Getenv("FORGE_RUNNER_ATTEMPT_BOUNDARY_FIXTURE")
	if path == "" {
		t.Skip("FORGE_RUNNER_ATTEMPT_BOUNDARY_FIXTURE is set by the contract script")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	mutations := map[string][]byte{
		"unknown":   append(bytes.TrimSuffix(bytes.TrimSpace(encoded), []byte("}")), []byte(`,"unexpected":true}`)...),
		"duplicate": append(bytes.TrimSuffix(bytes.TrimSpace(encoded), []byte("}")), []byte(`,"schema_version":"forge.runner-attempt-boundary/v1"}`)...),
		"trailing":  append(bytes.TrimSpace(encoded), []byte(" {}")...),
	}
	for name, mutation := range mutations {
		t.Run(name, func(t *testing.T) {
			if _, err := decodeRunnerAttemptBoundaryFixture(mutation); err == nil {
				t.Fatal("wire drift was accepted")
			}
		})
	}

	mutated := bytes.Replace(encoded, []byte(`"attempt_boundary_ready": true`), []byte(`"attempt_boundary_ready": false`), 1)
	fixture, err := decodeRunnerAttemptBoundaryFixture(mutated)
	if err != nil {
		t.Fatal(err)
	}
	if fixture.Validate() == nil {
		t.Fatal("readiness drift was accepted")
	}
}

func readRunnerAttemptBoundaryFixture(t *testing.T) RunnerAttemptBoundaryObservation {
	t.Helper()
	path := os.Getenv("FORGE_RUNNER_ATTEMPT_BOUNDARY_FIXTURE")
	if path == "" {
		t.Skip("FORGE_RUNNER_ATTEMPT_BOUNDARY_FIXTURE is set by the contract script")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	fixture, err := decodeRunnerAttemptBoundaryFixture(encoded)
	if err != nil {
		t.Fatalf("decode Runner Attempt boundary fixture: %v", err)
	}
	return fixture
}

func decodeRunnerAttemptBoundaryFixture(encoded []byte) (RunnerAttemptBoundaryObservation, error) {
	if err := rejectDuplicateFields(encoded); err != nil {
		return RunnerAttemptBoundaryObservation{}, err
	}
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	var fixture RunnerAttemptBoundaryObservation
	if err := decoder.Decode(&fixture); err != nil {
		return RunnerAttemptBoundaryObservation{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return RunnerAttemptBoundaryObservation{}, errInvalidRequest
		}
		return RunnerAttemptBoundaryObservation{}, err
	}
	return fixture, nil
}

func runnerExecutionBoundaryRequestForAttemptBoundary(t *testing.T) RunnerExecutionBoundaryRequest {
	t.Helper()
	return runnerExecutionBoundaryRequest(t)
}
