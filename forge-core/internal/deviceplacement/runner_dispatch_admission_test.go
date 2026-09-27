package deviceplacement

import (
	"bytes"
	"encoding/json"
	"testing"
)

func TestRunnerDispatchAdmissionBindsCurrentLeaseWithoutAuthority(t *testing.T) {
	local := localRunnerPreviewRequest(t)
	request := RunnerDispatchAdmissionRequest{
		Owner: local.Intent.Owner, ConversationID: local.Intent.ConversationID,
		RunID: local.Intent.Run.RunID, AttemptID: local.Intent.Binding.AttemptID,
		AttemptState: "accepted", Command: local.Intent.Command, EvaluatedAtMS: 300,
	}
	observation, err := ObserveRunnerDispatchAdmission(request, RunnerDispatchAdmissionLease{
		TargetID: local.Intent.Binding.TargetID, Epoch: local.Intent.Command.LeaseProof.Epoch,
		IssuedAtMS: 100, ExpiresAtMS: 1_000, Current: true, Active: true,
	})
	if err != nil {
		t.Fatalf("observe dispatch admission: %v", err)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate dispatch admission: %v", err)
	}
	if !observation.AdmissionReady || !observation.CommandBindingValid ||
		!observation.LeaseProofCurrent || !observation.LeaseActive ||
		observation.Authority != (RunnerDispatchAdmissionAuthority{}) {
		t.Fatalf("unexpected admission observation: %#v", observation)
	}
	encoded, err := json.Marshal(observation)
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Contains(encoded, []byte("fencing_token")) || bytes.Contains(encoded, []byte("argv")) ||
		bytes.Contains(encoded, []byte("workspace_ref")) {
		t.Fatalf("dispatch admission leaked command or fencing material: %s", encoded)
	}
}

func TestRunnerDispatchAdmissionRetainsFailClosedReasons(t *testing.T) {
	local := localRunnerPreviewRequest(t)
	request := RunnerDispatchAdmissionRequest{
		Owner: local.Intent.Owner, ConversationID: local.Intent.ConversationID,
		RunID: local.Intent.Run.RunID, AttemptID: local.Intent.Binding.AttemptID,
		AttemptState: "completed", Command: local.Intent.Command, EvaluatedAtMS: 300,
	}
	observation, err := ObserveRunnerDispatchAdmission(request, RunnerDispatchAdmissionLease{
		TargetID: local.Intent.Binding.TargetID, Epoch: local.Intent.Command.LeaseProof.Epoch,
		IssuedAtMS: 100, ExpiresAtMS: 1_000, Current: false, Active: false,
	})
	if err != nil {
		t.Fatalf("observe rejected admission: %v", err)
	}
	if observation.AdmissionReady || len(observation.RejectionReasons) != 3 ||
		!contains(observation.RejectionReasons, "attempt_state_not_dispatchable") ||
		!contains(observation.RejectionReasons, "lease_proof_not_current") ||
		!contains(observation.RejectionReasons, "lease_inactive_at_evaluated_time") {
		t.Fatalf("unexpected rejection reasons: %#v", observation)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate rejected admission: %v", err)
	}
	observation.RejectionReasons = []string{"z", "a"}
	if err := observation.Validate(); err == nil {
		t.Fatal("unsorted rejection reasons were accepted")
	}
}

func TestRunnerDispatchAdmissionRejectsConfusedCommand(t *testing.T) {
	local := localRunnerPreviewRequest(t)
	request := RunnerDispatchAdmissionRequest{
		Owner: local.Intent.Owner, ConversationID: local.Intent.ConversationID,
		RunID: local.Intent.Run.RunID, AttemptID: local.Intent.Binding.AttemptID,
		AttemptState: "accepted", Command: local.Intent.Command, EvaluatedAtMS: 300,
	}
	request.Command.LeaseProof.AttemptID = "attempt-foreign"
	observation, err := ObserveRunnerDispatchAdmission(request, RunnerDispatchAdmissionLease{
		TargetID: local.Intent.Binding.TargetID, Epoch: local.Intent.Command.LeaseProof.Epoch,
		IssuedAtMS: 100, ExpiresAtMS: 1_000, Current: true, Active: true,
	})
	if err != nil {
		t.Fatal(err)
	}
	if observation.AdmissionReady || !contains(observation.RejectionReasons, "command_binding_invalid") {
		t.Fatalf("foreign command proof was admitted: %#v", observation)
	}
}
