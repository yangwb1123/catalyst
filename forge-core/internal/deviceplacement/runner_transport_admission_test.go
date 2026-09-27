package deviceplacement

import (
	"bytes"
	"encoding/json"
	"os"
	"testing"

	"forgeos/forge-core/internal/runnertransport"
)

// secret-scan:ignore — deterministic HMAC fixture, never used outside this test.
const transportAdmissionSecret = "runner-transport-admission-secret"

func transportAdmissionObservation(t *testing.T) runnertransport.Observation {
	t.Helper()
	path := runnerDispatchTransportPath("runner-1")
	payload := []byte(`{"attempt_id":"attempt-1","command_id":"command-1","target_id":"runner-1"}`)
	const timestamp = int64(1_700_000_000)
	const nonce = "transport-admission-1"
	signature, err := runnertransport.Sign(transportAdmissionSecret, "POST", path, timestamp, nonce, payload)
	if err != nil {
		t.Fatal(err)
	}
	observation, err := runnertransport.Verify(transportAdmissionSecret, "POST", path, runnertransport.Envelope{
		TS: timestamp, Nonce: nonce, Sig: signature, Payload: json.RawMessage(payload),
	}, timestamp, runnertransport.NewReplayCache(4))
	if err != nil {
		t.Fatal(err)
	}
	return observation
}

func transportAdmissionRequest(t *testing.T) RunnerTransportAdmissionRequest {
	t.Helper()
	local := localRunnerPreviewRequest(t)
	transport := transportAdmissionObservation(t)
	return RunnerTransportAdmissionRequest{
		Owner: local.Intent.Owner, ConversationID: local.Intent.ConversationID,
		RunID: local.Intent.Run.RunID, AttemptID: local.Intent.Binding.AttemptID,
		AttemptState: "accepted", Command: local.Intent.Command,
		Lease: RunnerDispatchAdmissionLease{
			TargetID: "runner-1", Epoch: 1, IssuedAtMS: 100, ExpiresAtMS: 10_100,
			Current: true, Active: true,
		},
		Transport:             transport,
		ExpectedPayloadSHA256: transport.PayloadSHA256,
		EvaluatedAtMS:         300,
	}
}

func TestRunnerTransportAdmissionBindsVerifiedTransportToFencedLease(t *testing.T) {
	request := transportAdmissionRequest(t)
	observation, err := ObserveRunnerTransportAdmission(request)
	if err != nil {
		t.Fatalf("observe transport admission: %v", err)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate transport admission: %v", err)
	}
	if !observation.AdmissionReady || !observation.TransportBindingValid ||
		!observation.CommandBindingValid || !observation.LeaseProofCurrent ||
		!observation.LeaseActive || observation.Authority != (RunnerTransportAdmissionAuthority{}) {
		t.Fatalf("unexpected transport admission: %#v", observation)
	}
	encoded, err := json.Marshal(observation)
	if err != nil {
		t.Fatal(err)
	}
	for _, forbidden := range []string{"fencing_token", "argv", "workspace_ref", "payload_body"} {
		if bytes.Contains(encoded, []byte(forbidden)) {
			t.Fatalf("transport admission leaked %q: %s", forbidden, encoded)
		}
	}
}

func TestRunnerTransportAdmissionRejectsPathAndPayloadConfusion(t *testing.T) {
	request := transportAdmissionRequest(t)
	request.Transport.Path = "/api/v1/runners/runner-other/dispatch"
	observation, err := ObserveRunnerTransportAdmission(request)
	if err != nil {
		t.Fatal(err)
	}
	if observation.AdmissionReady || !contains(observation.RejectionReasons, "transport_binding_invalid") {
		t.Fatalf("confused transport path was admitted: %#v", observation)
	}
	if err := observation.Validate(); err == nil {
		t.Fatal("invalid transport path was accepted by observation validator")
	}

	request = transportAdmissionRequest(t)
	request.ExpectedPayloadSHA256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	observation, err = ObserveRunnerTransportAdmission(request)
	if err != nil {
		t.Fatal(err)
	}
	if observation.AdmissionReady || !contains(observation.RejectionReasons, "transport_binding_invalid") {
		t.Fatalf("confused transport payload was admitted: %#v", observation)
	}
}

func TestRunnerTransportAdmissionRetainsLeaseAndAttemptRejections(t *testing.T) {
	request := transportAdmissionRequest(t)
	request.AttemptState = "completed"
	request.Lease.Current = false
	request.Lease.Active = false
	observation, err := ObserveRunnerTransportAdmission(request)
	if err != nil {
		t.Fatal(err)
	}
	if observation.AdmissionReady ||
		!contains(observation.RejectionReasons, "attempt_state_not_dispatchable") ||
		!contains(observation.RejectionReasons, "lease_proof_not_current") ||
		!contains(observation.RejectionReasons, "lease_inactive_at_evaluated_time") {
		t.Fatalf("lease/Attempt rejection was lost: %#v", observation)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate rejected transport observation: %v", err)
	}
}

func TestRunnerTransportAdmissionValidatorRejectsUnsafeTransportMetadata(t *testing.T) {
	base, err := ObserveRunnerTransportAdmission(transportAdmissionRequest(t))
	if err != nil {
		t.Fatal(err)
	}
	cases := map[string]func(*RunnerTransportAdmissionObservation){
		"invalid nonce": func(value *RunnerTransportAdmissionObservation) {
			value.TransportNonce = "\x00"
		},
		"unsafe epoch": func(value *RunnerTransportAdmissionObservation) {
			value.LeaseEpoch = uint64(MaxSafeIntegerMS) + 1
		},
		"unsafe transport timestamp": func(value *RunnerTransportAdmissionObservation) {
			value.TransportTimestamp = MaxSafeIntegerMS + 1
		},
		"oversized transport payload": func(value *RunnerTransportAdmissionObservation) {
			value.TransportPayloadBytes = runnertransport.MaxPayloadBytes + 1
		},
	}
	for name, mutate := range cases {
		t.Run(name, func(t *testing.T) {
			value := base
			mutate(&value)
			if err := value.Validate(); err == nil {
				t.Fatal("unsafe transport metadata was accepted")
			}
		})
	}
}

func TestRunnerTransportAdmissionCanonicalFixtureValidates(t *testing.T) {
	encoded, err := os.ReadFile("../../../docs/contracts/fixtures/forge-runner-transport-admission-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	var observation RunnerTransportAdmissionObservation
	if err := json.Unmarshal(encoded, &observation); err != nil {
		t.Fatalf("decode transport admission fixture: %v", err)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate transport admission fixture: %v", err)
	}
	if !observation.AdmissionReady || observation.TransportPath != runnerDispatchTransportPath(observation.TargetID) {
		t.Fatalf("fixture drifted: %#v", observation)
	}
}
