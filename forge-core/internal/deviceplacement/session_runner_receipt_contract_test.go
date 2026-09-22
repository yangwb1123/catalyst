package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"testing"
)

func TestSessionRunnerReceiptObservationContractFixture(t *testing.T) {
	fixture := readSessionRunnerReceiptFixture(t)
	if err := fixture.Validate(); err != nil {
		t.Fatalf("invalid session Runner receipt fixture: %v", err)
	}
	observation, err := ObserveSessionRunnerReceipt(sessionRunnerReceiptRequest())
	if err != nil {
		t.Fatalf("observe session Runner receipt: %v", err)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("generated session Runner receipt is invalid: %v", err)
	}
	fixtureJSON, err := json.Marshal(fixture)
	if err != nil {
		t.Fatal(err)
	}
	observationJSON, err := json.Marshal(observation)
	if err != nil {
		t.Fatal(err)
	}
	var expected, actual any
	if err := json.Unmarshal(fixtureJSON, &expected); err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(observationJSON, &actual); err != nil {
		t.Fatal(err)
	}
	if !jsonEqual(expected, actual) {
		t.Fatalf("generated session Runner receipt differs from fixture:\nwant %s\ngot  %s", fixtureJSON, observationJSON)
	}
	if observation.ReceiptObservation.TargetID != "runner-1" || observation.SelectedTargetID != nil {
		t.Fatalf("session receipt unexpectedly selected a target: %#v", observation)
	}
}

func TestSessionRunnerReceiptObservationRejectsConfusedOrAuthoritativeValues(t *testing.T) {
	base := sessionRunnerReceiptRequest()
	for name, mutate := range map[string]func(*SessionRunnerReceiptObservationRequest){
		"foreign prompt": func(request *SessionRunnerReceiptObservationRequest) {
			request.Intent.PromptID = "prompt-foreign"
		},
		"foreign run": func(request *SessionRunnerReceiptObservationRequest) {
			request.Intent.RunID = "run-foreign"
		},
		"foreign receipt command": func(request *SessionRunnerReceiptObservationRequest) {
			request.Receipt.CommandID = "command-foreign"
		},
		"foreign receipt target": func(request *SessionRunnerReceiptObservationRequest) {
			request.Receipt.TargetID = "runner-foreign"
		},
		"receipt digest drift": func(request *SessionRunnerReceiptObservationRequest) {
			request.Receipt.CommandSHA256 = "b" + request.Receipt.CommandSHA256[1:]
		},
		"selected target": func(request *SessionRunnerReceiptObservationRequest) {
			selected := "runner-1"
			request.Intent.SelectedTargetID = &selected
		},
		"receipt authority": func(request *SessionRunnerReceiptObservationRequest) {
			request.Receipt.Authority.ExecutionAuthorized = true
		},
	} {
		t.Run(name, func(t *testing.T) {
			request := base
			mutate(&request)
			if _, err := ObserveSessionRunnerReceipt(request); err == nil {
				t.Fatal("confused or authoritative session receipt was accepted")
			}
		})
	}

	observation, err := ObserveSessionRunnerReceipt(base)
	if err != nil {
		t.Fatal(err)
	}
	observation.Authority.ReceiptPersisted = true
	if err := observation.Validate(); err == nil {
		t.Fatal("authoritative session receipt observation was accepted")
	}
}

func TestSessionRunnerReceiptObservationUncertainRequiresManualReconciliation(t *testing.T) {
	request := sessionRunnerReceiptRequest()
	request.Receipt.DispositionKind = "uncertain"
	request.Receipt.Uncertain = true
	request.Receipt.ReconciliationRequired = true
	request.Receipt.ManualReviewRequired = true
	request.Receipt.FollowUp = "reconciliation_manual"
	observation, err := ObserveSessionRunnerReceipt(request)
	if err != nil {
		t.Fatalf("uncertain session receipt: %v", err)
	}
	if !observation.ReceiptObservation.Uncertain ||
		!observation.ReceiptObservation.ReconciliationRequired ||
		!observation.ReceiptObservation.ManualReviewRequired ||
		observation.ReceiptObservation.AutomaticRetry ||
		observation.ReceiptObservation.FollowUp != "reconciliation_manual" {
		t.Fatalf("uncertain session receipt = %#v", observation)
	}
}

func TestSessionRunnerReceiptObservationFixtureRejectsUnknownFields(t *testing.T) {
	path := os.Getenv("FORGE_SESSION_RUNNER_RECEIPT_CONTRACT_FIXTURE")
	if path == "" {
		t.Skip("FORGE_SESSION_RUNNER_RECEIPT_CONTRACT_FIXTURE is set by the cross-repository contract test")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	encoded = append(bytes.TrimSpace(encoded), []byte(`{"unexpected":true}`)...)
	if _, err := decodeSessionRunnerReceiptFixture(encoded); err == nil {
		t.Fatal("unknown session receipt field was accepted")
	}
}

func sessionRunnerReceiptRequest() SessionRunnerReceiptObservationRequest {
	return SessionRunnerReceiptObservationRequest{
		Owner:          Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"},
		ConversationID: "conversation-001",
		PromptID:       "prompt-001",
		RunID:          "run-001",
		Intent: RunnerExecutionIntentObservation{
			SchemaVersion: RunnerExecutionIntentSchemaVersion, EvaluationMode: RunnerExecutionIntentEvaluationMode,
			Owner:          Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"},
			ConversationID: "conversation-001", PromptID: "prompt-001", RunID: "run-001",
			AttemptID: "attempt-001", CommandID: "command-001", TargetID: "runner-1",
			CommandSHA256:  "42ed02a535113450e6f2cc757fb9b4e2cce6143724274191bbae159e9ea8de7a",
			IdempotencyKey: "run-001:attempt-001:command-001", PromptRunBindingValid: true,
			RunnerCommandBindingValid: true, PreviewOnly: true, SelectedTargetID: nil,
			Authority: RunnerExecutionIntentAuthority{},
		},
		Receipt: RunnerTerminalReceiptObservation{
			SchemaVersion: RunnerTerminalReceiptSchemaVersion, EvaluationMode: RunnerTerminalReceiptEvaluationMode,
			CommandID: "command-001", CommandSHA256: "42ed02a535113450e6f2cc757fb9b4e2cce6143724274191bbae159e9ea8de7a",
			AttemptID: "attempt-001", TargetID: "runner-1", DispositionKind: "completed", ObservedAtMS: 300,
			ReceiptValid: true, PreviewOnly: true, Uncertain: false, ReconciliationRequired: false,
			ManualReviewRequired: false, AutomaticRetry: false, FollowUp: "none", Authority: RunnerTerminalReceiptAuthority{},
		},
	}
}

func readSessionRunnerReceiptFixture(t *testing.T) SessionRunnerReceiptObservation {
	t.Helper()
	path := os.Getenv("FORGE_SESSION_RUNNER_RECEIPT_CONTRACT_FIXTURE")
	if path == "" {
		t.Skip("FORGE_SESSION_RUNNER_RECEIPT_CONTRACT_FIXTURE is set by the cross-repository contract test")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	fixture, err := decodeSessionRunnerReceiptFixture(encoded)
	if err != nil {
		t.Fatalf("decode session Runner receipt fixture: %v", err)
	}
	return fixture
}

func decodeSessionRunnerReceiptFixture(encoded []byte) (SessionRunnerReceiptObservation, error) {
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	var fixture SessionRunnerReceiptObservation
	if err := decoder.Decode(&fixture); err != nil {
		return SessionRunnerReceiptObservation{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return SessionRunnerReceiptObservation{}, errInvalidRequest
		}
		return SessionRunnerReceiptObservation{}, err
	}
	return fixture, nil
}

func jsonEqual(left, right any) bool {
	leftJSON, leftErr := json.Marshal(left)
	rightJSON, rightErr := json.Marshal(right)
	return leftErr == nil && rightErr == nil && bytes.Equal(leftJSON, rightJSON)
}
