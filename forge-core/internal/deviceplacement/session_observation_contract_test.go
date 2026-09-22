package deviceplacement

import (
	"bytes"
	"encoding/json"
	"os"
	"reflect"
	"testing"
)

type sessionPlacementFixture struct {
	APIVersion     string                   `json:"api_version"`
	Owner          Owner                    `json:"owner"`
	ConversationID string                   `json:"conversation_id"`
	RunID          string                   `json:"run_id"`
	Placement      placementParityFixture   `json:"placement"`
	Expected       sessionPlacementExpected `json:"expected"`
}

type sessionPlacementExpected struct {
	EvaluationMode             string                     `json:"evaluation_mode"`
	EvaluatedAtMS              int64                      `json:"evaluated_at_ms"`
	OwnerDeclarationUnverified bool                       `json:"owner_declaration_unverified"`
	DeviceAttributesUnverified bool                       `json:"device_attributes_unverified"`
	Decisions                  []sessionPlacementDecision `json:"decisions"`
	SelectedDeviceID           *string                    `json:"selected_device_id"`
	SelectedInstanceID         *string                    `json:"selected_instance_id"`
	Authority                  SessionPlacementAuthority  `json:"authority"`
}

type sessionPlacementDecision struct {
	DeviceID            string   `json:"device_id"`
	InstanceID          string   `json:"instance_id"`
	MatchesRequirements bool     `json:"matches_requirements"`
	ExclusionReasons    []string `json:"exclusion_reasons"`
}

// TestSessionPlacementObservationContractFixture pins the same pure
// owner/Conversation/Run placement projection consumed by the other clients.
// It never opens Hub state or represents a target assignment.
func TestSessionPlacementObservationContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_SESSION_PLACEMENT_CONTRACT_FIXTURE")
	if path == "" {
		t.Skip("FORGE_SESSION_PLACEMENT_CONTRACT_FIXTURE is set by the cross-repository contract test")
	}
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
	if err := decoder.Decode(&trailing); err == nil {
		t.Fatal("session placement fixture has trailing JSON")
	}
	if fixture.APIVersion != "forgeos.session-placement-observation-contract/v1" ||
		fixture.Placement.SchemaVersion != "forge.device-placement-policy-parity-test/v1" ||
		fixture.Owner != fixture.Placement.Owner {
		t.Fatalf("unexpected session placement binding: %#v", fixture)
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
	if observation.SchemaVersion != SessionPlacementObservationSchemaVersion ||
		observation.EvaluationMode != fixture.Expected.EvaluationMode ||
		observation.Owner != fixture.Owner || observation.ConversationID != fixture.ConversationID ||
		observation.RunID != fixture.RunID || observation.EvaluatedAtMS != fixture.Expected.EvaluatedAtMS ||
		observation.OwnerDeclarationUnverified != fixture.Expected.OwnerDeclarationUnverified ||
		observation.DeviceAttributesUnverified != fixture.Expected.DeviceAttributesUnverified ||
		observation.SelectedDeviceID != nil || observation.SelectedInstanceID != nil ||
		observation.Authority != fixture.Expected.Authority {
		t.Fatalf("unexpected session placement observation: %#v", observation)
	}
	want := make([]sessionPlacementDecision, 0, len(fixture.Expected.Decisions))
	for _, decision := range fixture.Expected.Decisions {
		want = append(want, decision)
	}
	got := make([]sessionPlacementDecision, 0, len(observation.Decisions))
	for _, decision := range observation.Decisions {
		got = append(got, sessionPlacementDecision{
			DeviceID: decision.DeviceID, InstanceID: decision.InstanceID,
			MatchesRequirements: decision.MatchesRequirements,
			ExclusionReasons:    decision.ExclusionReasons,
		})
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("decisions=%#v, want=%#v", got, want)
	}
}

func TestSessionPlacementObservationRejectsMismatchedOwner(t *testing.T) {
	owner := Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	request := SessionPlacementObservationRequest{
		Owner: owner, ConversationID: "conversation-001", RunID: "run-001",
		Placement: Request{SchemaVersion: RequestSchemaVersion, EvaluatedAtMS: 1,
			Owner:            Owner{Issuer: owner.Issuer, Subject: "foreign", TenantID: owner.TenantID},
			MaxSnapshotAgeMS: 1, Requirements: validRequest().Requirements},
	}
	if _, err := ObserveSessionPlacement(request); err == nil {
		t.Fatal("mismatched session owner was accepted")
	}
}
