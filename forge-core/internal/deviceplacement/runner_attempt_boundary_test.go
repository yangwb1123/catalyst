package deviceplacement

import (
	"bytes"
	"encoding/json"
	"testing"

	"forgeos/forge-core/internal/executionattempt"
)

func TestRunnerAttemptBoundaryAcceptsOnlyForwardDispatchLifecycleEdges(t *testing.T) {
	boundary := observedRunnerExecutionBoundary(t, runnerExecutionBoundaryRequest(t))
	ready, err := ObserveRunnerAttemptBoundary(RunnerAttemptBoundaryRequest{
		Boundary:   boundary,
		Transition: executionattempt.BeginStarting,
	})
	if err != nil {
		t.Fatalf("observe accepted-to-starting boundary: %v", err)
	}
	if err := ready.Validate(); err != nil {
		t.Fatalf("validate accepted-to-starting boundary: %v", err)
	}
	if !ready.ExecutionBoundaryReady || !ready.AttemptTransitionValid ||
		!ready.AttemptTransitionDispatch || !ready.AttemptBoundaryReady ||
		ready.CurrentAttemptState != "accepted" || ready.NextAttemptState != "starting" {
		t.Fatalf("unexpected ready boundary: %#v", ready)
	}

	startingRequest := transportAdmissionRequest(t)
	startingRequest.AttemptState = "starting"
	startingBoundary := observedRunnerExecutionBoundary(t, runnerExecutionBoundaryRequestFromTransport(t, startingRequest))
	running, err := ObserveRunnerAttemptBoundary(RunnerAttemptBoundaryRequest{
		Boundary:   startingBoundary,
		Transition: executionattempt.ObserveRunning,
	})
	if err != nil {
		t.Fatalf("observe starting-to-running boundary: %v", err)
	}
	if err := running.Validate(); err != nil {
		t.Fatalf("validate starting-to-running boundary: %v", err)
	}
	if !running.AttemptBoundaryReady || running.NextAttemptState != "running" {
		t.Fatalf("starting-to-running boundary was not ready: %#v", running)
	}
}

func TestRunnerAttemptBoundaryFailsClosedForInvalidOrTerminalEdges(t *testing.T) {
	base := observedRunnerExecutionBoundary(t, runnerExecutionBoundaryRequest(t))
	invalid, err := ObserveRunnerAttemptBoundary(RunnerAttemptBoundaryRequest{
		Boundary: base, Transition: executionattempt.ObserveRunning,
	})
	if err != nil {
		t.Fatalf("observe invalid lifecycle edge: %v", err)
	}
	if invalid.AttemptBoundaryReady || invalid.AttemptTransitionValid ||
		!contains(invalid.RejectionReasons, "attempt_transition_invalid") {
		t.Fatalf("invalid accepted-to-running edge was admitted: %#v", invalid)
	}
	if err := invalid.Validate(); err != nil {
		t.Fatalf("validate invalid lifecycle edge: %v", err)
	}

	runningRequest := transportAdmissionRequest(t)
	runningRequest.AttemptState = "running"
	runningBoundary := observedRunnerExecutionBoundary(t, runnerExecutionBoundaryRequestFromTransport(t, runningRequest))
	terminal, err := ObserveRunnerAttemptBoundary(RunnerAttemptBoundaryRequest{
		Boundary: runningBoundary, Transition: executionattempt.ObserveCompleted,
	})
	if err != nil {
		t.Fatalf("observe terminal lifecycle edge: %v", err)
	}
	if terminal.AttemptTransitionValid == false || terminal.AttemptBoundaryReady ||
		!contains(terminal.RejectionReasons, "attempt_transition_not_dispatchable") {
		t.Fatalf("terminal edge was treated as dispatchable: %#v", terminal)
	}
	if err := terminal.Validate(); err != nil {
		t.Fatalf("validate terminal lifecycle edge: %v", err)
	}
}

func TestRunnerAttemptBoundaryRetainsExecutionBlockersAndRedactsAuthority(t *testing.T) {
	blocked := runnerExecutionBoundaryRequestFromTransport(t, transportAdmissionRequest(t))
	blocked.Controls.CancellationRequested = true
	boundary := observedRunnerExecutionBoundary(t, blocked)
	value, err := ObserveRunnerAttemptBoundary(RunnerAttemptBoundaryRequest{
		Boundary: boundary, Transition: executionattempt.BeginStarting,
	})
	if err != nil {
		t.Fatalf("observe blocked lifecycle boundary: %v", err)
	}
	if value.AttemptBoundaryReady || !contains(value.RejectionReasons, "execution_boundary_not_ready") {
		t.Fatalf("blocked execution boundary became ready: %#v", value)
	}
	if err := value.Validate(); err != nil {
		t.Fatalf("validate blocked lifecycle boundary: %v", err)
	}
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	for _, forbidden := range []string{"fencing_token", "argv", "workspace_ref", "payload_body"} {
		if bytes.Contains(encoded, []byte(forbidden)) {
			t.Fatalf("attempt boundary leaked %q: %s", forbidden, encoded)
		}
	}
	if value.Authority != (RunnerAttemptBoundaryAuthority{}) {
		t.Fatalf("attempt boundary gained authority: %#v", value.Authority)
	}
}

func TestRunnerAttemptBoundaryRejectsMutatedObservation(t *testing.T) {
	value, err := ObserveRunnerAttemptBoundary(RunnerAttemptBoundaryRequest{
		Boundary:   observedRunnerExecutionBoundary(t, runnerExecutionBoundaryRequest(t)),
		Transition: executionattempt.BeginStarting,
	})
	if err != nil {
		t.Fatal(err)
	}
	value.NextAttemptState = "running"
	if err := value.Validate(); err == nil {
		t.Fatal("mutated next state was accepted")
	}
}

func observedRunnerExecutionBoundary(t *testing.T, request RunnerExecutionBoundaryRequest) RunnerExecutionBoundaryObservation {
	t.Helper()
	value, err := ObserveRunnerExecutionBoundary(request)
	if err != nil {
		t.Fatalf("observe execution boundary fixture: %v", err)
	}
	return value
}
