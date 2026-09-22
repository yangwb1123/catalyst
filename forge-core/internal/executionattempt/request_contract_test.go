package executionattempt

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"testing"
)

type requestFixture struct {
	SchemaVersion  string               `json:"schema_version"`
	EvaluationMode string               `json:"evaluation_mode"`
	Authority      requestAuthority     `json:"authority"`
	Cases          []requestFixtureCase `json:"cases"`
}
type requestAuthority struct {
	DeviceIdentityVerified bool `json:"device_identity_verified"`
	ReferencesResolved     bool `json:"references_resolved"`
	RequestPersisted       bool `json:"request_persisted"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
	AuditPublished         bool `json:"audit_published"`
}
type requestFixtureCase struct {
	Name     string                 `json:"name"`
	Request  AttemptRequestInput    `json:"request"`
	Expected requestFixtureExpected `json:"expected"`
}
type requestFixtureExpected struct {
	Accepted          bool     `json:"accepted"`
	Error             string   `json:"error"`
	InitialState      string   `json:"initial_state"`
	RequestedEffects  []string `json:"requested_effects"`
	ApprovalRecordIDs []string `json:"approval_record_ids"`
}

func TestAttemptRequestContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_ATTEMPT_REQUEST_FIXTURE")
	if path == "" {
		t.Skip("FORGE_ATTEMPT_REQUEST_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var fixture requestFixture
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode Attempt request fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("fixture has trailing JSON: %v", err)
	}
	if fixture.SchemaVersion != "forge.attempt-request/v1" || fixture.EvaluationMode != "pure_attempt_request_only" ||
		fixture.Authority.DeviceIdentityVerified || fixture.Authority.ReferencesResolved || fixture.Authority.RequestPersisted ||
		fixture.Authority.ReservationCreated || fixture.Authority.ExecutionAuthorized || fixture.Authority.DispatchPerformed ||
		fixture.Authority.AuditPublished || len(fixture.Cases) != 18 {
		t.Fatalf("invalid fixture envelope: %#v", fixture)
	}
	for _, testCase := range fixture.Cases {
		t.Run(testCase.Name, func(t *testing.T) {
			request, err := NewRequest(testCase.Request)
			if !testCase.Expected.Accepted {
				if err == nil {
					t.Fatal("request accepted unexpectedly")
				}
				value, ok := err.(*Error)
				if !ok || string(value.Code()) != testCase.Expected.Error {
					t.Fatalf("error=%v, want %q", err, testCase.Expected.Error)
				}
				return
			}
			if err != nil {
				t.Fatalf("request rejected: %v", err)
			}
			if request.InitialState() != testCase.Expected.InitialState {
				t.Fatalf("state=%q", request.InitialState())
			}
			effects := request.RequestedEffects()
			if len(effects) != len(testCase.Expected.RequestedEffects) {
				t.Fatalf("effects=%v", effects)
			}
			for index, value := range effects {
				if value != testCase.Expected.RequestedEffects[index] {
					t.Fatalf("effects=%v", effects)
				}
			}
			approvals := request.ApprovalRefs()
			if len(approvals) != len(testCase.Expected.ApprovalRecordIDs) {
				t.Fatalf("approvals=%v", approvals)
			}
			for index, value := range approvals {
				if value.RecordID != testCase.Expected.ApprovalRecordIDs[index] {
					t.Fatalf("approvals=%v", approvals)
				}
			}
		})
	}
}
