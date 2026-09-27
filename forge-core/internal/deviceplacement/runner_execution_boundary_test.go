package deviceplacement

import (
	"bytes"
	"encoding/json"
	"testing"

	"forgeos/forge-core/internal/devicefabricgate"
)

func TestRunnerExecutionBoundaryRequiresBothAdmissionPreviewsAndIndependentGate(t *testing.T) {
	request := runnerExecutionBoundaryRequest(t)
	observation, err := ObserveRunnerExecutionBoundary(request)
	if err != nil {
		t.Fatalf("observe execution boundary: %v", err)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate execution boundary: %v", err)
	}
	if !observation.ExecutionBoundaryReady || !observation.ActivationAllowed ||
		!observation.RunnerAuthorityAccepted || !observation.DispatchAdmissionReady ||
		!observation.TransportAdmissionReady || !observation.EffectStateStartable ||
		!observation.CancellationClear || observation.Authority != (RunnerExecutionBoundaryAuthority{}) {
		t.Fatalf("unexpected ready boundary: %#v", observation)
	}
	encoded, err := json.Marshal(observation)
	if err != nil {
		t.Fatal(err)
	}
	for _, forbidden := range []string{"fencing_token", "argv", "workspace_ref", "payload_body"} {
		if bytes.Contains(encoded, []byte(forbidden)) {
			t.Fatalf("execution boundary leaked %q: %s", forbidden, encoded)
		}
	}
}

func TestRunnerExecutionBoundaryBlocksCancellationAndUncertainEffects(t *testing.T) {
	request := runnerExecutionBoundaryRequest(t)
	request.Controls.CancellationRequested = true
	observation, err := ObserveRunnerExecutionBoundary(request)
	if err != nil {
		t.Fatal(err)
	}
	if observation.ExecutionBoundaryReady || !contains(observation.RejectionReasons, "cancellation_requested") {
		t.Fatalf("cancelled boundary was admitted: %#v", observation)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate cancelled boundary: %v", err)
	}

	request = runnerExecutionBoundaryRequest(t)
	request.Controls.EffectState = "uncertain"
	observation, err = ObserveRunnerExecutionBoundary(request)
	if err != nil {
		t.Fatal(err)
	}
	if observation.ExecutionBoundaryReady || !contains(observation.RejectionReasons, "uncertain_effect_requires_reconciliation") {
		t.Fatalf("uncertain boundary was admitted: %#v", observation)
	}

	transportRequest := transportAdmissionRequest(t)
	transportRequest.AttemptState = "completed"
	transportRequest.Lease.Current = false
	transportRequest.Lease.Active = false
	request = runnerExecutionBoundaryRequestFromTransport(t, transportRequest)
	observation, err = ObserveRunnerExecutionBoundary(request)
	if err != nil {
		t.Fatal(err)
	}
	if observation.ExecutionBoundaryReady || !contains(observation.RejectionReasons, "dispatch_admission_not_ready") ||
		!contains(observation.RejectionReasons, "transport_admission_not_ready") {
		t.Fatalf("admission blockers were lost: %#v", observation)
	}
}

func TestRunnerExecutionBoundaryRejectsConfusedPreviewsAndAuthorityMutation(t *testing.T) {
	request := runnerExecutionBoundaryRequest(t)
	request.Transport.TargetID = "runner-other"
	if _, err := ObserveRunnerExecutionBoundary(request); err == nil {
		t.Fatal("confused transport target was accepted")
	}

	request = runnerExecutionBoundaryRequest(t)
	observation, err := ObserveRunnerExecutionBoundary(request)
	if err != nil {
		t.Fatal(err)
	}
	observation.Authority.ExecutionAuthorized = true
	if err := observation.Validate(); err == nil {
		t.Fatal("authoritative execution boundary was accepted")
	}
	observation, err = ObserveRunnerExecutionBoundary(runnerExecutionBoundaryRequest(t))
	if err != nil {
		t.Fatal(err)
	}
	observation.RejectionReasons = []string{"z", "a"}
	if err := observation.Validate(); err == nil {
		t.Fatal("unsorted boundary reasons were accepted")
	}
}

func runnerExecutionBoundaryRequest(t *testing.T) RunnerExecutionBoundaryRequest {
	t.Helper()
	return runnerExecutionBoundaryRequestFromTransport(t, transportAdmissionRequest(t))
}

func runnerExecutionBoundaryRequestFromTransport(t *testing.T, transportRequest RunnerTransportAdmissionRequest) RunnerExecutionBoundaryRequest {
	t.Helper()
	dispatch, err := ObserveRunnerDispatchAdmission(
		RunnerDispatchAdmissionRequest{
			Owner: transportRequest.Owner, ConversationID: transportRequest.ConversationID,
			RunID: transportRequest.RunID, AttemptID: transportRequest.AttemptID,
			AttemptState: transportRequest.AttemptState, Command: transportRequest.Command,
			EvaluatedAtMS: transportRequest.EvaluatedAtMS,
		}, transportRequest.Lease,
	)
	if err != nil {
		t.Fatal(err)
	}
	transport, err := ObserveRunnerTransportAdmission(transportRequest)
	if err != nil {
		t.Fatal(err)
	}
	return RunnerExecutionBoundaryRequest{
		Activation: acceptedRunnerExecutionActivation(),
		Authority: devicefabricgate.RunnerAuthorityConfig{
			Enabled: true, AuthorityID: "runner-authority-1",
			Decision: devicefabricgate.Decision{
				Status: "accepted", AcceptanceID: "runner-authority-acceptance", AcceptedAtUnixMS: 1,
			},
		},
		Dispatch: dispatch, Transport: transport,
		Controls: RunnerExecutionBoundaryControls{EffectState: "not_started"},
	}
}

func acceptedRunnerExecutionActivation() devicefabricgate.Request {
	accepted := func(id string) devicefabricgate.Decision {
		return devicefabricgate.Decision{Status: "accepted", AcceptanceID: id, AcceptedAtUnixMS: 1}
	}
	return devicefabricgate.Request{
		Mode:    devicefabricgate.ModeExecute,
		ADR0039: accepted("adr-0039-execute"), ADR0113: accepted("adr-0113-execute"),
		ADR0114: accepted("adr-0114-execute"), P4: accepted("p4-execute"),
		Evidence: devicefabricgate.Evidence{
			CoordinatorOwnerIsolation: true, DeviceIdentityProof: true,
			OwnerApprovalAndRevocation: true, HeartbeatCASAndFreshness: true,
			InventoryOwnerScope: true, DisabledDefaultAndRouteClose: true,
			SecurityReview: true, RunnerIsolation: true, LeaseFencing: true,
			CancellationAndUncertainWork: true, VaultArtifactAuthorization: true,
			AuditOutbox: true,
		},
	}
}
