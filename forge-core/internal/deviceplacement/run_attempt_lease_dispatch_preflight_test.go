package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"path/filepath"
	"testing"
)

func TestRunAttemptLeaseDispatchPreflightJoinsRunAndDispatchPlan(t *testing.T) {
	input := RunAttemptLeaseDispatchPreflightRequest{
		Owner:          runnerDispatchPlanPreviewRequest(t).Intent.Owner,
		ConversationID: "conversation-1",
		RunID:          "run-1",
		RunStatus:      "nonterminal",
		DispatchPlan:   runnerDispatchPlanPreviewRequest(t),
	}
	before, err := json.Marshal(input)
	if err != nil {
		t.Fatal(err)
	}
	observation, err := ObserveRunAttemptLeaseDispatchPreflight(input)
	if err != nil {
		t.Fatalf("observe preflight: %v", err)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate preflight: %v", err)
	}
	if !observation.RunStateAdmissible || !observation.AttemptStateAdmissible || !observation.LeaseActive ||
		observation.DeclarativeReadyCount != 1 || !observation.DeclarativePreflightReady ||
		observation.SelectedTargetID != nil || observation.Authority != (RunAttemptLeaseDispatchPreflightAuthority{}) {
		t.Fatalf("preflight summary = %#v", observation)
	}
	after, err := json.Marshal(input)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(before, after) {
		t.Fatalf("preflight mutated input:\nbefore=%s\nafter=%s", before, after)
	}
	encoded, err := json.Marshal(observation)
	if err != nil {
		t.Fatal(err)
	}
	for _, forbidden := range []string{"fencing_token", "argv", "workspace_ref", "selected_target_id\\\":\\\""} {
		if bytes.Contains(encoded, []byte(forbidden)) {
			t.Fatalf("preflight leaked or selected forbidden field %q: %s", forbidden, encoded)
		}
	}
}

func TestRunAttemptLeaseDispatchPreflightReportsStableReasons(t *testing.T) {
	tests := []struct {
		name   string
		change func(*RunAttemptLeaseDispatchPreflightRequest)
		want   []string
	}{
		{
			name: "terminal run",
			change: func(input *RunAttemptLeaseDispatchPreflightRequest) {
				input.RunStatus = "completed"
			},
			want: []string{"run_state_not_dispatchable"},
		},
		{
			name: "terminal attempt and expired lease",
			change: func(input *RunAttemptLeaseDispatchPreflightRequest) {
				input.DispatchPlan.AttemptState = "completed"
				input.DispatchPlan.Lease.ExpiresAtMS = uint64(input.DispatchPlan.Placement.EvaluatedAtMS)
			},
			want: []string{"attempt_state_not_dispatchable", "lease_inactive_at_evaluated_time", "no_declarative_ready_candidate"},
		},
		{
			name: "no matching candidate",
			change: func(input *RunAttemptLeaseDispatchPreflightRequest) {
				for index := range input.DispatchPlan.Placement.Devices {
					input.DispatchPlan.Placement.Devices[index].ApprovalState = "pending"
				}
			},
			want: []string{"no_declarative_ready_candidate"},
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			plan := runnerDispatchPlanPreviewRequest(t)
			input := RunAttemptLeaseDispatchPreflightRequest{
				Owner: plan.Intent.Owner, ConversationID: plan.Intent.ConversationID,
				RunID: plan.Intent.RunID, RunStatus: "nonterminal", DispatchPlan: plan,
			}
			test.change(&input)
			observation, err := ObserveRunAttemptLeaseDispatchPreflight(input)
			if err != nil {
				t.Fatal(err)
			}
			if observation.DeclarativePreflightReady || !samePreflightStrings(observation.RejectionReasons, test.want) {
				t.Fatalf("reasons=%v ready=%v want=%v", observation.RejectionReasons, observation.DeclarativePreflightReady, test.want)
			}
			if err := observation.Validate(); err != nil {
				t.Fatalf("validate rejection observation: %v", err)
			}
		})
	}
}

func TestRunAttemptLeaseDispatchPreflightRejectsConfusedOrMutatedValues(t *testing.T) {
	basePlan := runnerDispatchPlanPreviewRequest(t)
	base := RunAttemptLeaseDispatchPreflightRequest{
		Owner: basePlan.Intent.Owner, ConversationID: basePlan.Intent.ConversationID,
		RunID: basePlan.Intent.RunID, RunStatus: "nonterminal", DispatchPlan: basePlan,
	}
	for _, test := range []struct {
		name   string
		change func(*RunAttemptLeaseDispatchPreflightRequest)
	}{
		{name: "foreign owner", change: func(input *RunAttemptLeaseDispatchPreflightRequest) { input.Owner.Subject = "other-user" }},
		{name: "foreign conversation", change: func(input *RunAttemptLeaseDispatchPreflightRequest) { input.ConversationID = "conversation-other" }},
		{name: "foreign run", change: func(input *RunAttemptLeaseDispatchPreflightRequest) { input.RunID = "run-other" }},
		{name: "unknown run state", change: func(input *RunAttemptLeaseDispatchPreflightRequest) { input.RunStatus = "unknown" }},
	} {
		t.Run(test.name, func(t *testing.T) {
			input := base
			test.change(&input)
			if _, err := ObserveRunAttemptLeaseDispatchPreflight(input); err == nil {
				t.Fatal("confused preflight input was accepted")
			}
		})
	}

	observation, err := ObserveRunAttemptLeaseDispatchPreflight(base)
	if err != nil {
		t.Fatal(err)
	}
	mutations := []struct {
		name   string
		change func(*RunAttemptLeaseDispatchPreflightObservation)
	}{
		{name: "selected target", change: func(value *RunAttemptLeaseDispatchPreflightObservation) {
			target := "runner-1"
			value.SelectedTargetID = &target
		}},
		{name: "authority", change: func(value *RunAttemptLeaseDispatchPreflightObservation) { value.Authority.DispatchPerformed = true }},
		{name: "missing rejection reason", change: func(value *RunAttemptLeaseDispatchPreflightObservation) { value.LeaseActive = false }},
		{name: "unsafe lease epoch", change: func(value *RunAttemptLeaseDispatchPreflightObservation) {
			value.LeaseEpoch = uint64(MaxSafeIntegerMS) + 1
		}},
	}
	for _, test := range mutations {
		t.Run(test.name, func(t *testing.T) {
			mutated := observation
			test.change(&mutated)
			if err := mutated.Validate(); err == nil {
				t.Fatal("mutated preflight observation was accepted")
			}
		})
	}
}

func TestRunAttemptLeaseDispatchPreflightCanonicalFixture(t *testing.T) {
	fixturePath := os.Getenv("FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE")
	if fixturePath == "" {
		fixturePath = filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "forge-run-attempt-lease-dispatch-preflight-v1.json")
	}
	encoded, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatal(err)
	}
	var observation RunAttemptLeaseDispatchPreflightObservation
	if err := json.Unmarshal(encoded, &observation); err != nil {
		t.Fatal(err)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate fixture: %v", err)
	}
	if !observation.DeclarativePreflightReady || observation.SelectedTargetID != nil {
		t.Fatalf("fixture readiness/selection drifted: %#v", observation)
	}
}

func TestRunAttemptLeaseDispatchPreflightCanonicalRequestProducesCanonicalResponse(t *testing.T) {
	fixtureRoot := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures")
	requestBytes, err := os.ReadFile(filepath.Join(fixtureRoot, "forge-run-attempt-lease-dispatch-preflight-request-v1.json"))
	if err != nil {
		t.Fatal(err)
	}
	if err := rejectDuplicateFields(requestBytes); err != nil {
		t.Fatalf("canonical request has duplicate or malformed JSON: %v", err)
	}
	var request RunAttemptLeaseDispatchPreflightRequest
	decoder := json.NewDecoder(bytes.NewReader(requestBytes))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&request); err != nil {
		t.Fatalf("decode canonical request: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		t.Fatalf("canonical request has trailing JSON: %v", err)
	}

	observation, err := ObserveRunAttemptLeaseDispatchPreflight(request)
	if err != nil {
		t.Fatalf("evaluate canonical request: %v", err)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate evaluated canonical response: %v", err)
	}
	actual, err := json.MarshalIndent(observation, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	actual = append(actual, '\n')
	want, err := os.ReadFile(filepath.Join(fixtureRoot, "forge-run-attempt-lease-dispatch-preflight-v1.json"))
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(actual, want) {
		t.Fatalf("canonical request/response drift:\nactual:\n%s\nwant:\n%s", actual, want)
	}
}
