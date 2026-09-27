package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"testing"
)

func TestSessionRunnerReconciliationProjectionContractFixture(t *testing.T) {
	want, err := decodeSessionRunnerReconciliationProjection(readSessionRunnerReconciliationProjectionFixture(t))
	if err != nil || want.Validate() != nil {
		t.Fatalf("invalid reconciliation projection fixture: decode=%v validate=%v", err, want.Validate())
	}
	history, err := decodeSessionRunnerReceiptHistory(readSessionRunnerReceiptHistoryFixture(t))
	if err != nil {
		t.Fatal(err)
	}
	got, err := ProjectSessionRunnerReconciliation(history)
	if err != nil {
		t.Fatalf("project reconciliation: %v", err)
	}
	assertSameSessionRunnerReconciliationJSON(t, want, got)
}

func TestSessionRunnerReconciliationProjectionRejectsWireAndSummaryDrift(t *testing.T) {
	encoded := readSessionRunnerReconciliationProjectionFixture(t)
	for name, mutation := range map[string][]byte{
		"unknown root field":   append(bytes.TrimSuffix(bytes.TrimSpace(encoded), []byte("}")), []byte(`,"unexpected":true}`)...),
		"duplicate root field": append(bytes.TrimSuffix(bytes.TrimSpace(encoded), []byte("}")), []byte(`,"schema_version":"forge.session-runner-reconciliation-projection/v1"}`)...),
		"trailing value":       append(bytes.TrimSpace(encoded), []byte(" {}")...),
	} {
		t.Run(name, func(t *testing.T) {
			if _, err := decodeSessionRunnerReconciliationProjection(mutation); err == nil {
				t.Fatal("wire drift was accepted")
			}
		})
	}
	base, err := decodeSessionRunnerReconciliationProjection(encoded)
	if err != nil {
		t.Fatal(err)
	}
	for name, mutate := range map[string]func(*SessionRunnerReconciliationProjection){
		"source binding":  func(value *SessionRunnerReconciliationProjection) { value.Source.RunID = "other-run" },
		"latest summary":  func(value *SessionRunnerReconciliationProjection) { value.LatestAttemptID = "other-attempt" },
		"automatic retry": func(value *SessionRunnerReconciliationProjection) { value.AutomaticRetry = true },
		"selected target": func(value *SessionRunnerReconciliationProjection) {
			selected := "runner-2"
			value.SelectedTargetID = &selected
		},
		"authority": func(value *SessionRunnerReconciliationProjection) { value.Authority.AuditPublished = true },
	} {
		t.Run(name, func(t *testing.T) {
			candidate := base
			mutate(&candidate)
			if candidate.Validate() == nil {
				t.Fatal("projection drift was accepted")
			}
		})
	}
}

func TestSessionRunnerReconciliationProjectionRequiresUncertainTerminalHistory(t *testing.T) {
	history, err := decodeSessionRunnerReceiptHistory(readSessionRunnerReceiptHistoryFixture(t))
	if err != nil {
		t.Fatal(err)
	}
	latest := &history.Receipts[len(history.Receipts)-1].ReceiptObservation
	latest.DispositionKind = "completed"
	latest.Uncertain = false
	latest.ReconciliationRequired = false
	latest.ManualReviewRequired = false
	latest.FollowUp = "none"
	history, err = ObserveSessionRunnerReceiptHistory(SessionRunnerReceiptHistoryRequest{
		Owner: history.Owner, ConversationID: history.ConversationID, PromptID: history.PromptID,
		RunID: history.RunID, Receipts: history.Receipts,
	})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := ProjectSessionRunnerReconciliation(history); err == nil {
		t.Fatal("completed history produced a manual reconciliation projection")
	}
}

func readSessionRunnerReconciliationProjectionFixture(t *testing.T) []byte {
	t.Helper()
	path := os.Getenv("FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE")
	if path == "" {
		t.Skip("FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE is set by the contract script")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}

func decodeSessionRunnerReconciliationProjection(encoded []byte) (SessionRunnerReconciliationProjection, error) {
	if err := rejectDuplicateFields(encoded); err != nil {
		return SessionRunnerReconciliationProjection{}, err
	}
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	var projection SessionRunnerReconciliationProjection
	if err := decoder.Decode(&projection); err != nil {
		return SessionRunnerReconciliationProjection{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return SessionRunnerReconciliationProjection{}, errInvalidRequest
		}
		return SessionRunnerReconciliationProjection{}, err
	}
	return projection, nil
}

func assertSameSessionRunnerReconciliationJSON(t *testing.T, want, got SessionRunnerReconciliationProjection) {
	t.Helper()
	wantJSON, err := json.Marshal(want)
	if err != nil {
		t.Fatal(err)
	}
	gotJSON, err := json.Marshal(got)
	if err != nil {
		t.Fatal(err)
	}
	var wantValue, gotValue any
	if json.Unmarshal(wantJSON, &wantValue) != nil || json.Unmarshal(gotJSON, &gotValue) != nil || !jsonEqual(wantValue, gotValue) {
		t.Fatalf("projection differs from fixture: want %s got %s", wantJSON, gotJSON)
	}
}
