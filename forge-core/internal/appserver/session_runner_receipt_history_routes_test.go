package appserver

import (
	"encoding/json"
	"net/http"
	"os"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceplacement"
)

func TestSessionRunnerReceiptHistoryPreviewBindsOwnerAndPath(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	body := sessionRunnerReceiptHistoryPreviewBody(t, identity)
	path := "/api/v1/conversations/conversation-001/runs/run-001/runner-receipt-history/preview"
	response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", body)
	if response.Code != http.StatusOK {
		t.Fatalf("history preview status=%d body=%q", response.Code, response.Body.String())
	}
	var observation deviceplacement.SessionRunnerReceiptHistoryObservation
	if err := json.Unmarshal(response.Body.Bytes(), &observation); err != nil {
		t.Fatalf("decode history preview: %v body=%q", err, response.Body.String())
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate history preview: %v", err)
	}
	if observation.AttemptCount != 2 || observation.LatestDispositionKind != "uncertain" ||
		!observation.ReconciliationRequired || !observation.ManualReviewRequired ||
		observation.AutomaticRetry || observation.SelectedTargetID != nil {
		t.Fatalf("unexpected history preview: %#v", observation)
	}

	foreign := requestConversationAPIAs(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "account-foreign", "tenant-slate", "application/json", "", body)
	if foreign.Code != http.StatusForbidden {
		t.Fatalf("foreign owner status=%d body=%q", foreign.Code, foreign.Body.String())
	}
	wrongPath := requestConversationAPI(t, routes, identity, http.MethodPost,
		"/api/v1/conversations/other/runs/run-001/runner-receipt-history/preview",
		"forge:conversations:read", "application/json", "", body)
	if wrongPath.Code != http.StatusBadRequest {
		t.Fatalf("wrong path status=%d body=%q", wrongPath.Code, wrongPath.Body.String())
	}
}

func TestSessionRunnerReceiptHistoryPreviewRejectsDriftAndProductionStaysClosed(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	candidate := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	body := sessionRunnerReceiptHistoryPreviewBody(t, identity)
	path := "/api/v1/conversations/conversation-001/runs/run-001/runner-receipt-history/preview"
	mutations := map[string]string{
		"unknown root": strings.TrimSuffix(body, "}") + `,"unexpected":true}`,
		"unknown nested": mutateHistoryBody(t, body, func(object map[string]any) {
			receipts := object["receipts"].([]any)
			receipts[0].(map[string]any)["unexpected"] = true
		}),
		"summary drift": mutateHistoryBody(t, body, func(object map[string]any) {
			object["attempt_count"] = 1
		}),
		"selected target": mutateHistoryBody(t, body, func(object map[string]any) {
			object["selected_target_id"] = "runner-2"
		}),
	}
	for name, value := range mutations {
		t.Run(name, func(t *testing.T) {
			response := requestConversationAPI(t, candidate, identity, http.MethodPost, path,
				"forge:conversations:read", "application/json", "", value)
			if response.Code != http.StatusBadRequest {
				t.Fatalf("drift status=%d body=%q", response.Code, response.Body.String())
			}
		})
	}

	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	response := requestConversationAPI(t, production, identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", body)
	if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
		t.Fatalf("production history status=%d body=%q", response.Code, response.Body.String())
	}
	method := requestConversationAPI(t, candidate, identity, http.MethodGet, path,
		"forge:conversations:read", "", "", "")
	if method.Code != http.StatusMethodNotAllowed || method.Header().Get("Allow") != http.MethodPost {
		t.Fatalf("method status=%d allow=%q body=%q", method.Code, method.Header().Get("Allow"), method.Body.String())
	}
}

func sessionRunnerReceiptHistoryPreviewBody(t *testing.T, identity *conversationTestIdentity) string {
	t.Helper()
	path := os.Getenv("FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE")
	if path == "" {
		t.Skip("FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE is set by the contract script")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var value deviceplacement.SessionRunnerReceiptHistoryObservation
	if err := json.Unmarshal(encoded, &value); err != nil {
		t.Fatal(err)
	}
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	value.Owner = owner
	for index := range value.Receipts {
		value.Receipts[index].Owner = owner
	}
	derived, err := deviceplacement.ObserveSessionRunnerReceiptHistory(deviceplacement.SessionRunnerReceiptHistoryRequest{
		Owner: owner, ConversationID: value.ConversationID, PromptID: value.PromptID,
		RunID: value.RunID, Receipts: value.Receipts,
	})
	if err != nil {
		t.Fatal(err)
	}
	result, err := json.Marshal(derived)
	if err != nil {
		t.Fatal(err)
	}
	return string(result)
}

func mutateHistoryBody(t *testing.T, body string, mutate func(map[string]any)) string {
	t.Helper()
	var object map[string]any
	if err := json.Unmarshal([]byte(body), &object); err != nil {
		t.Fatal(err)
	}
	mutate(object)
	encoded, err := json.Marshal(object)
	if err != nil {
		t.Fatal(err)
	}
	return string(encoded)
}
