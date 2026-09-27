package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"testing"
)

func TestSessionRunnerReceiptHistoryContractFixture(t *testing.T) {
	encoded := readSessionRunnerReceiptHistoryFixture(t)
	observation, err := decodeSessionRunnerReceiptHistory(encoded)
	if err != nil {
		t.Fatal(err)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("invalid session Runner receipt history fixture: %v", err)
	}
	generated, err := ObserveSessionRunnerReceiptHistory(SessionRunnerReceiptHistoryRequest{
		Owner: observation.Owner, ConversationID: observation.ConversationID,
		PromptID: observation.PromptID, RunID: observation.RunID, Receipts: observation.Receipts,
	})
	if err != nil {
		t.Fatalf("reduce session Runner receipt history: %v", err)
	}
	wantJSON, err := json.Marshal(observation)
	if err != nil {
		t.Fatal(err)
	}
	gotJSON, err := json.Marshal(generated)
	if err != nil {
		t.Fatal(err)
	}
	var want, got any
	if err := json.Unmarshal(wantJSON, &want); err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(gotJSON, &got); err != nil {
		t.Fatal(err)
	}
	if !jsonEqual(want, got) {
		t.Fatalf("reduced history differs from fixture: want %s got %s", wantJSON, gotJSON)
	}
	if observation.AttemptCount != 2 || observation.LatestDispositionKind != "uncertain" ||
		!observation.ReconciliationRequired || !observation.ManualReviewRequired ||
		observation.AutomaticRetry || observation.FollowUp != "reconciliation_manual" {
		t.Fatalf("history summary lost the manual boundary: %#v", observation)
	}
}

func TestSessionRunnerReceiptHistoryRejectsWireAndLifecycleDrift(t *testing.T) {
	encoded := readSessionRunnerReceiptHistoryFixture(t)
	mutations := map[string][]byte{
		"unknown root field":   append(bytes.TrimSuffix(bytes.TrimSpace(encoded), []byte("}")), []byte(`,"unexpected":true}`)...),
		"duplicate root field": append(bytes.TrimSuffix(bytes.TrimSpace(encoded), []byte("}")), []byte(`,"schema_version":"forge.session-runner-receipt-history/v1"}`)...),
		"trailing value":       append(bytes.TrimSpace(encoded), []byte(" {}")...),
	}
	for name, mutation := range mutations {
		t.Run(name, func(t *testing.T) {
			if _, err := decodeSessionRunnerReceiptHistory(mutation); err == nil {
				t.Fatal("wire drift was accepted")
			}
		})
	}

	base, err := decodeSessionRunnerReceiptHistory(encoded)
	if err != nil {
		t.Fatal(err)
	}
	for name, mutate := range map[string]func(*SessionRunnerReceiptHistoryObservation){
		"duplicate attempt": func(value *SessionRunnerReceiptHistoryObservation) {
			value.Receipts[1].ReceiptObservation.AttemptID = value.Receipts[0].ReceiptObservation.AttemptID
		},
		"out of order": func(value *SessionRunnerReceiptHistoryObservation) {
			value.Receipts[1].ReceiptObservation.ObservedAtMS = 50
		},
		"retry after uncertain": func(value *SessionRunnerReceiptHistoryObservation) {
			value.Receipts[0].ReceiptObservation.DispositionKind = "uncertain"
			value.Receipts[0].ReceiptObservation.Uncertain = true
			value.Receipts[0].ReceiptObservation.ReconciliationRequired = true
			value.Receipts[0].ReceiptObservation.ManualReviewRequired = true
			value.Receipts[0].ReceiptObservation.FollowUp = "reconciliation_manual"
		},
		"summary drift": func(value *SessionRunnerReceiptHistoryObservation) {
			value.AttemptCount++
		},
		"selected target": func(value *SessionRunnerReceiptHistoryObservation) {
			selected := "runner-2"
			value.SelectedTargetID = &selected
		},
		"authority": func(value *SessionRunnerReceiptHistoryObservation) {
			value.Authority.ReceiptPersisted = true
		},
	} {
		t.Run(name, func(t *testing.T) {
			candidate := base
			candidate.Receipts = append([]SessionRunnerReceiptObservation(nil), base.Receipts...)
			mutate(&candidate)
			if err := candidate.Validate(); err == nil {
				t.Fatal("history drift was accepted")
			}
		})
	}
}

func readSessionRunnerReceiptHistoryFixture(t *testing.T) []byte {
	t.Helper()
	path := os.Getenv("FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE")
	if path == "" {
		t.Skip("FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE is set by the contract script")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}

func decodeSessionRunnerReceiptHistory(encoded []byte) (SessionRunnerReceiptHistoryObservation, error) {
	if err := rejectDuplicateFields(encoded); err != nil {
		return SessionRunnerReceiptHistoryObservation{}, err
	}
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	var observation SessionRunnerReceiptHistoryObservation
	if err := decoder.Decode(&observation); err != nil {
		return SessionRunnerReceiptHistoryObservation{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return SessionRunnerReceiptHistoryObservation{}, errInvalidRequest
		}
		return SessionRunnerReceiptHistoryObservation{}, err
	}
	return observation, nil
}
