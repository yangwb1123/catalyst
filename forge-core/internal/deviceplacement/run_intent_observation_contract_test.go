package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"testing"
)

type runIntentObservationFixture struct {
	APIVersion               string                 `json:"api_version"`
	PlacementContractFixture string                 `json:"placement_contract_fixture"`
	Owner                    Owner                  `json:"owner"`
	ConversationID           string                 `json:"conversation_id"`
	Prompt                   RunIntentPromptReceipt `json:"prompt_receipt"`
	Run                      RunIntentRunReference  `json:"run_reference"`
	Expected                 runIntentExpected      `json:"expected"`
}

type runIntentExpected struct {
	EvaluationMode             string                    `json:"evaluation_mode"`
	PromptAccepted             bool                      `json:"prompt_accepted"`
	RunReferenceObserved       bool                      `json:"run_reference_observed"`
	PromptRunBindingValid      bool                      `json:"prompt_run_binding_valid"`
	PlacementObservationBound  bool                      `json:"placement_observation_bound"`
	PreviewOnly                bool                      `json:"preview_only"`
	IntentReplayed             bool                      `json:"intent_replayed"`
	RunStatus                  string                    `json:"run_status"`
	RunLatestSequence          uint64                    `json:"run_latest_sequence"`
	PromptAcceptedAtMS         int64                     `json:"prompt_accepted_at_ms"`
	PlacementEvaluatedAtMS     int64                     `json:"placement_evaluated_at_ms"`
	PlacementDecisionCount     int                       `json:"placement_decision_count"`
	EligibleInstanceCount      int                       `json:"eligible_instance_count"`
	OwnerDeclarationUnverified bool                      `json:"owner_declaration_unverified"`
	DeviceAttributesUnverified bool                      `json:"device_attributes_unverified"`
	SelectedDeviceID           *string                   `json:"selected_device_id"`
	SelectedInstanceID         *string                   `json:"selected_instance_id"`
	Authority                  SessionPlacementAuthority `json:"authority"`
}

func TestRunIntentObservationContractFixture(t *testing.T) {
	runPath := os.Getenv("FORGE_RUN_INTENT_OBSERVATION_CONTRACT_FIXTURE")
	placementPath := os.Getenv("FORGE_SESSION_PLACEMENT_CONTRACT_FIXTURE")
	if runPath == "" || placementPath == "" {
		t.Skip("run intent and session placement fixtures are set by the cross-repository contract test")
	}
	fixture := readRunIntentFixture(t, runPath)
	placement := readSessionPlacementObservation(t, placementPath)
	if fixture.APIVersion != "forgeos.run-intent-observation-contract/v1" ||
		fixture.PlacementContractFixture != "forge-session-placement-observation-v1" {
		t.Fatalf("unexpected Run intent fixture metadata: %#v", fixture)
	}
	observation, err := ObserveRunIntent(RunIntentObservationRequest{
		Owner: fixture.Owner, ConversationID: fixture.ConversationID,
		Prompt: fixture.Prompt, Run: fixture.Run, Placement: placement,
	})
	if err != nil {
		t.Fatalf("observe Run intent: %v", err)
	}
	want := fixture.Expected
	if observation.SchemaVersion != RunIntentObservationSchemaVersion ||
		observation.EvaluationMode != want.EvaluationMode || observation.Owner != fixture.Owner ||
		observation.ConversationID != fixture.ConversationID || observation.PromptID != fixture.Prompt.PromptID ||
		observation.IntentID != fixture.Prompt.IntentID || observation.RunID != fixture.Run.RunID ||
		observation.PromptAccepted != want.PromptAccepted || observation.RunReferenceObserved != want.RunReferenceObserved ||
		observation.PromptRunBindingValid != want.PromptRunBindingValid ||
		observation.PlacementObservationBound != want.PlacementObservationBound ||
		observation.PreviewOnly != want.PreviewOnly || observation.IntentReplayed != want.IntentReplayed ||
		observation.RunStatus != want.RunStatus || observation.RunLatestSequence != want.RunLatestSequence ||
		observation.PromptAcceptedAtMS != want.PromptAcceptedAtMS ||
		observation.PlacementEvaluatedAtMS != want.PlacementEvaluatedAtMS ||
		observation.PlacementDecisionCount != want.PlacementDecisionCount ||
		observation.EligibleInstanceCount != want.EligibleInstanceCount ||
		observation.OwnerDeclarationUnverified != want.OwnerDeclarationUnverified ||
		observation.DeviceAttributesUnverified != want.DeviceAttributesUnverified ||
		observation.SelectedDeviceID != want.SelectedDeviceID ||
		observation.SelectedInstanceID != want.SelectedInstanceID || observation.Authority != want.Authority {
		t.Fatalf("unexpected Run intent observation: %#v, want %#v", observation, want)
	}
}

func TestRunIntentObservationRejectsConfusedBindings(t *testing.T) {
	owner := Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	placement := SessionPlacementObservation{
		SchemaVersion: SessionPlacementObservationSchemaVersion, EvaluationMode: EvaluationMode,
		Owner: owner, ConversationID: "conversation-1", RunID: "run-1",
		OwnerDeclarationUnverified: true, DeviceAttributesUnverified: true,
		Authority: SessionPlacementAuthority{}, Decisions: []SessionPlacementDecision{{
			DeviceID: "device-1", InstanceID: "runner-1", MatchesRequirements: true,
		}},
	}
	base := RunIntentObservationRequest{
		Owner: owner, ConversationID: "conversation-1",
		Prompt: RunIntentPromptReceipt{
			PromptID: "prompt-1", ConversationID: "conversation-1", Role: "user",
			AcceptedAtMS: 10, IntentID: "intent-1", InitialEventID: "event-1",
			InitialEventSequence: 1, InitialEventType: "submitted",
		},
		Run: RunIntentRunReference{
			RunID: "run-1", ConversationID: "conversation-1", PromptID: "prompt-1",
			CreatedAtMS: 10, LatestSequence: 1, Status: "nonterminal",
		},
		Placement: placement,
	}
	for name, mutate := range map[string]func(*RunIntentObservationRequest){
		"prompt conversation": func(request *RunIntentObservationRequest) {
			request.Prompt.ConversationID = "foreign"
		},
		"run prompt": func(request *RunIntentObservationRequest) {
			request.Run.PromptID = "other-prompt"
		},
		"placement selected": func(request *RunIntentObservationRequest) {
			selected := "device-1"
			request.Placement.SelectedDeviceID = &selected
		},
		"placement authority": func(request *RunIntentObservationRequest) {
			request.Placement.Authority.ExecutionAuthorized = true
		},
	} {
		t.Run(name, func(t *testing.T) {
			request := base
			mutate(&request)
			if _, err := ObserveRunIntent(request); err == nil {
				t.Fatal("confused Run intent binding was accepted")
			}
		})
	}
}

func readRunIntentFixture(t *testing.T, path string) runIntentObservationFixture {
	t.Helper()
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	var fixture runIntentObservationFixture
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode Run intent fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		t.Fatalf("Run intent fixture has trailing JSON: %v", err)
	}
	return fixture
}

func readSessionPlacementObservation(t *testing.T, path string) SessionPlacementObservation {
	t.Helper()
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture sessionPlacementFixture
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode session placement fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		t.Fatalf("session placement fixture has trailing JSON: %v", err)
	}
	request := Request{
		SchemaVersion: RequestSchemaVersion, EvaluatedAtMS: fixture.Placement.EvaluatedAtMS,
		Owner: fixture.Placement.Owner, MaxSnapshotAgeMS: fixture.Placement.MaxSnapshotAgeMS,
		Requirements: fixture.Placement.Requirements,
		Devices:      make([]Device, 0, len(fixture.Placement.Candidates)),
	}
	candidates := make([]SessionPlacementCandidate, 0, len(fixture.Placement.Candidates))
	for _, candidate := range fixture.Placement.Candidates {
		request.Devices = append(request.Devices, candidate.Device)
		candidates = append(candidates, SessionPlacementCandidate{
			InstanceID: candidate.InstanceID, Device: candidate.Device,
		})
	}
	observation, err := ObserveSessionPlacement(SessionPlacementObservationRequest{
		Owner: fixture.Owner, ConversationID: fixture.ConversationID, RunID: fixture.RunID,
		Placement: request, Candidates: candidates,
	})
	if err != nil {
		t.Fatalf("observe session placement: %v", err)
	}
	return observation
}
