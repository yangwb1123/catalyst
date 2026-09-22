package deviceplacement

import (
	"bytes"
	"encoding/json"
	"os"
	"testing"

	"forgeos/forge-core/internal/executionattempt"
)

func TestRunnerDispatchPlanPreviewBindsPlacementAttemptAndLease(t *testing.T) {
	input := runnerDispatchPlanPreviewRequest(t)
	accepted, err := executionattempt.NewLifecycle().Reduce(executionattempt.Accept)
	if err != nil {
		t.Fatal(err)
	}
	input.AttemptState = string(accepted.State())
	observation, err := ObserveRunnerDispatchPlanPreview(input)
	if err != nil {
		t.Fatalf("observe dispatch plan preview: %v", err)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate dispatch plan preview: %v", err)
	}
	if observation.CandidateCount != 2 || observation.DeclarativeReadyCount != 1 ||
		!observation.AttemptStateAdmissible || !observation.LeaseActive ||
		observation.SelectedTargetID != nil {
		t.Fatalf("plan preview summary = %#v", observation)
	}
	first, second := observation.Candidates[0], observation.Candidates[1]
	if first.TargetID != "runner-1" || !first.MatchesRequirements || !first.LeaseTargetMatch ||
		!first.LeaseActive || !first.DeclarativeReady || first.Reasons == nil || len(first.Reasons) != 0 {
		t.Fatalf("leased candidate = %#v", first)
	}
	if second.TargetID != "runner-2" || !second.MatchesRequirements || second.LeaseTargetMatch ||
		second.DeclarativeReady || !contains(second.Reasons, "lease_target_mismatch") {
		t.Fatalf("unleased candidate = %#v", second)
	}
	if observation.Authority != (RunnerDispatchPlanPreviewAuthority{}) ||
		observation.ReservationCreated || observation.ExecutionAuthorized || observation.DispatchPerformed {
		t.Fatalf("dispatch preview gained authority = %#v", observation)
	}
	encoded, err := json.Marshal(observation)
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Contains(encoded, []byte("fencing_token")) || bytes.Contains(encoded, []byte("argv")) {
		t.Fatalf("dispatch preview leaked command/lease secret fields: %s", encoded)
	}
	if !bytes.Contains(encoded, []byte(`"reasons":[]`)) {
		t.Fatalf("ready candidate reasons must remain an empty JSON array: %s", encoded)
	}
}

func TestRunnerDispatchPlanPreviewKeepsStateAndLeaseGatesDeclarative(t *testing.T) {
	t.Run("terminal attempt", func(t *testing.T) {
		input := runnerDispatchPlanPreviewRequest(t)
		input.AttemptState = "completed"
		observation, err := ObserveRunnerDispatchPlanPreview(input)
		if err != nil {
			t.Fatal(err)
		}
		if observation.AttemptStateAdmissible || observation.DeclarativeReadyCount != 0 {
			t.Fatalf("terminal state preview = %#v", observation)
		}
		for _, candidate := range observation.Candidates {
			if candidate.DeclarativeReady || !contains(candidate.Reasons, "attempt_state_not_dispatchable") {
				t.Fatalf("terminal candidate = %#v", candidate)
			}
		}
	})
	t.Run("expired lease", func(t *testing.T) {
		input := runnerDispatchPlanPreviewRequest(t)
		input.Lease.ExpiresAtMS = uint64(input.Placement.EvaluatedAtMS)
		observation, err := ObserveRunnerDispatchPlanPreview(input)
		if err != nil {
			t.Fatal(err)
		}
		if observation.LeaseActive || observation.DeclarativeReadyCount != 0 {
			t.Fatalf("expired lease preview = %#v", observation)
		}
		if !contains(observation.Candidates[0].Reasons, "lease_inactive_at_evaluated_time") {
			t.Fatalf("expired lease reasons = %#v", observation.Candidates[0].Reasons)
		}
	})
}

func TestRunnerDispatchPlanPreviewRejectsConfusedInputs(t *testing.T) {
	base := runnerDispatchPlanPreviewRequest(t)
	tests := []struct {
		name   string
		change func(*RunnerDispatchPlanPreviewRequest)
	}{
		{"unknown attempt state", func(input *RunnerDispatchPlanPreviewRequest) { input.AttemptState = "mystery" }},
		{"foreign lease attempt", func(input *RunnerDispatchPlanPreviewRequest) { input.Lease.AttemptID = "attempt-foreign" }},
		{"foreign lease target", func(input *RunnerDispatchPlanPreviewRequest) { input.Lease.TargetID = "runner-foreign" }},
		{"foreign owner", func(input *RunnerDispatchPlanPreviewRequest) { input.Placement.Owner.Subject = "other-user" }},
		{"authoritative intent", func(input *RunnerDispatchPlanPreviewRequest) { input.Intent.Authority.ExecutionAuthorized = true }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			input := base
			input.Placement.Devices = append([]Device(nil), base.Placement.Devices...)
			test.change(&input)
			if _, err := ObserveRunnerDispatchPlanPreview(input); err == nil {
				t.Fatal("confused or authoritative input was accepted")
			}
		})
	}
}

func TestRunnerDispatchPlanPreviewIsDeterministicAndRejectsMutatedObservation(t *testing.T) {
	input := runnerDispatchPlanPreviewRequest(t)
	left, err := ObserveRunnerDispatchPlanPreview(input)
	if err != nil {
		t.Fatal(err)
	}
	input.Placement.Devices = []Device{input.Placement.Devices[1], input.Placement.Devices[0]}
	right, err := ObserveRunnerDispatchPlanPreview(input)
	if err != nil {
		t.Fatal(err)
	}
	leftJSON, _ := json.Marshal(left)
	rightJSON, _ := json.Marshal(right)
	if !bytes.Equal(leftJSON, rightJSON) {
		t.Fatalf("input order changed preview:\n%s\n%s", leftJSON, rightJSON)
	}
	right.Candidates[0].Reasons = []string{"z", "a"}
	if err := right.Validate(); err == nil {
		t.Fatal("unsorted candidate reasons accepted")
	}
	right = left
	right.SelectedTargetID = stringPtr("runner-1")
	if err := right.Validate(); err == nil {
		t.Fatal("selected target accepted in preview")
	}
}

func TestRunnerDispatchPlanPreviewCanonicalFixtureValidates(t *testing.T) {
	fixturePath := "../../../docs/contracts/fixtures/forge-runner-dispatch-plan-preview-v1.json"
	encoded, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatal(err)
	}
	var observation RunnerDispatchPlanPreviewObservation
	if err := json.Unmarshal(encoded, &observation); err != nil {
		t.Fatalf("decode dispatch-plan preview fixture: %v", err)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate dispatch-plan preview fixture: %v", err)
	}
	if observation.SelectedTargetID != nil || observation.DeclarativeReadyCount != 1 {
		t.Fatalf("fixture gained selection or changed readiness: %#v", observation)
	}
}

func runnerDispatchPlanPreviewRequest(t *testing.T) RunnerDispatchPlanPreviewRequest {
	t.Helper()
	local := localRunnerPreviewRequest(t)
	intent, err := ObserveRunnerExecutionIntent(local.Intent)
	if err != nil {
		t.Fatal(err)
	}
	placement := validRequest()
	placement.Devices[0].DeviceID = "runner-1"
	placement.Devices = append(placement.Devices, validDevice("runner-2"))
	local.Grant.IssuedAtMS = uint64(placement.EvaluatedAtMS - 1_000)
	local.Grant.ExpiresAtMS = uint64(placement.EvaluatedAtMS + 5_000)
	return RunnerDispatchPlanPreviewRequest{
		AttemptState: "accepted", Placement: placement, Intent: intent, Lease: local.Grant,
	}
}

func stringPtr(value string) *string { return &value }
