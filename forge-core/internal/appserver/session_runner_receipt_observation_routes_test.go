package appserver

import (
	"bytes"
	"encoding/json"
	"net/http"
	"testing"

	"forgeos/forge-core/internal/deviceplacement"
)

func TestSessionRunnerReceiptObservationPreviewBindsOwnerAndRun(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}

	conversationID, runID := "conversation-001", "run-001"
	body := sessionRunnerReceiptObservationPreviewBody(
		t, identity.issuer, "account-42", "tenant-slate", conversationID, "prompt-001", runID,
	)
	path := "/api/v1/conversations/" + conversationID + "/runs/" + runID + "/runner-receipt-observation/preview"
	response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", body)
	if response.Code != http.StatusOK {
		t.Fatalf("Runner receipt observation status=%d body=%q", response.Code, response.Body.String())
	}
	wantBytes := append([]byte(body), '\n')
	if !bytes.Equal(response.Body.Bytes(), wantBytes) {
		t.Fatalf("preview did not return the canonical observation unchanged:\nwant=%s\ngot =%s", wantBytes, response.Body.Bytes())
	}
	repeat := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", body)
	if repeat.Code != http.StatusOK || !bytes.Equal(response.Body.Bytes(), repeat.Body.Bytes()) {
		t.Fatalf("repeated preview was not deterministic: first=%q second=%q", response.Body.String(), repeat.Body.String())
	}
	var observation deviceplacement.SessionRunnerReceiptObservation
	if err := json.Unmarshal(response.Body.Bytes(), &observation); err != nil {
		t.Fatalf("decode Runner receipt observation: %v body=%q", err, response.Body.String())
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate Runner receipt observation: %v", err)
	}
	if observation.SchemaVersion != deviceplacement.SessionRunnerReceiptObservationSchemaVersion ||
		observation.ConversationID != conversationID || observation.PromptID != "prompt-001" ||
		observation.RunID != runID || observation.SelectedTargetID != nil ||
		observation.Authority != (deviceplacement.SessionRunnerReceiptAuthority{}) {
		t.Fatalf("unexpected Runner receipt observation: %#v", observation)
	}

	foreign := requestConversationAPIAs(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "account-foreign", "tenant-slate", "application/json", "", body)
	if foreign.Code != http.StatusForbidden {
		t.Fatalf("foreign owner status=%d body=%q", foreign.Code, foreign.Body.String())
	}
	wrongPath := requestConversationAPI(t, routes, identity, http.MethodPost,
		"/api/v1/conversations/other/runs/"+runID+"/runner-receipt-observation/preview",
		"forge:conversations:read", "application/json", "", body)
	if wrongPath.Code != http.StatusBadRequest {
		t.Fatalf("foreign path status=%d body=%q", wrongPath.Code, wrongPath.Body.String())
	}
}

func TestSessionRunnerReceiptObservationPreviewKeepsReadOnlyBoundary(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	body := sessionRunnerReceiptObservationPreviewBody(t, identity.issuer, "account-42", "tenant-slate", "conversation-1", "prompt-1", "run-1")
	path := "/api/v1/conversations/conversation-1/runs/run-1/runner-receipt-observation/preview"
	noScope := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:write", "application/json", "", body)
	if noScope.Code != http.StatusForbidden {
		t.Fatalf("missing read scope status=%d body=%q", noScope.Code, noScope.Body.String())
	}
	method := requestConversationAPI(t, routes, identity, http.MethodGet, path,
		"forge:conversations:read", "", "", "")
	if method.Code != http.StatusMethodNotAllowed || method.Header().Get("Allow") != http.MethodPost {
		t.Fatalf("method status=%d allow=%q body=%q", method.Code, method.Header().Get("Allow"), method.Body.String())
	}
	query := requestConversationAPI(t, routes, identity, http.MethodPost, path+"?limit=1",
		"forge:conversations:read", "application/json", "", body)
	if query.Code != http.StatusBadRequest {
		t.Fatalf("query status=%d body=%q", query.Code, query.Body.String())
	}
	deviceRoute := requestConversationAPI(t, routes, identity, http.MethodGet, "/api/v1/devices",
		"forge:conversations:read", "", "", "")
	if deviceRoute.Code != http.StatusNotFound {
		t.Fatalf("device route status=%d body=%q", deviceRoute.Code, deviceRoute.Body.String())
	}
}

func TestSessionRunnerReceiptObservationPreviewRejectsNonCanonicalValues(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-1/runs/run-1/runner-receipt-observation/preview"
	base := sessionRunnerReceiptObservationPreviewBody(t, identity.issuer, "account-42", "tenant-slate", "conversation-1", "prompt-1", "run-1")
	tests := []struct {
		name string
		body string
	}{
		{
			name: "unknown top-level field",
			body: mutateSessionRunnerReceiptObservationBody(t, base, func(object map[string]any) {
				object["unexpected"] = true
			}),
		},
		{
			name: "missing null target field",
			body: mutateSessionRunnerReceiptObservationBody(t, base, func(object map[string]any) {
				delete(object, "selected_target_id")
			}),
		},
		{
			name: "selected target",
			body: mutateSessionRunnerReceiptObservationBody(t, base, func(object map[string]any) {
				object["selected_target_id"] = "runner-1"
			}),
		},
		{
			name: "authority mutation",
			body: mutateSessionRunnerReceiptObservationBody(t, base, func(object map[string]any) {
				authority := object["authority"].(map[string]any)
				authority["execution_authorized"] = true
			}),
		},
		{
			name: "nested authority null",
			body: mutateSessionRunnerReceiptObservationBody(t, base, func(object map[string]any) {
				receipt := object["receipt_observation"].(map[string]any)
				receipt["authority"] = nil
			}),
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
				"forge:conversations:read", "application/json", "", test.body)
			if response.Code != http.StatusBadRequest {
				t.Fatalf("non-canonical status=%d body=%q", response.Code, response.Body.String())
			}
		})
	}
}

func sessionRunnerReceiptObservationPreviewBody(
	t *testing.T,
	issuer, subject, tenant, conversationID, promptID, runID string,
) string {
	t.Helper()
	owner := deviceplacement.Owner{Issuer: issuer, Subject: subject, TenantID: tenant}
	observation := deviceplacement.SessionRunnerReceiptObservation{
		SchemaVersion:  deviceplacement.SessionRunnerReceiptObservationSchemaVersion,
		EvaluationMode: deviceplacement.SessionRunnerReceiptObservationEvaluationMode,
		Owner:          owner,
		ConversationID: conversationID,
		PromptID:       promptID,
		RunID:          runID,
		ReceiptObservation: deviceplacement.RunnerTerminalReceiptObservation{
			SchemaVersion:          deviceplacement.RunnerTerminalReceiptSchemaVersion,
			EvaluationMode:         deviceplacement.RunnerTerminalReceiptEvaluationMode,
			CommandID:              "command-001",
			CommandSHA256:          "42ed02a535113450e6f2cc757fb9b4e2cce6143724274191bbae159e9ea8de7a",
			AttemptID:              "attempt-001",
			TargetID:               "runner-1",
			DispositionKind:        "completed",
			ObservedAtMS:           300,
			ReceiptValid:           true,
			PreviewOnly:            true,
			Uncertain:              false,
			ReconciliationRequired: false,
			ManualReviewRequired:   false,
			AutomaticRetry:         false,
			FollowUp:               "none",
			Authority:              deviceplacement.RunnerTerminalReceiptAuthority{},
		},
		PromptRunBindingValid: true,
		ReceiptBindingValid:   true,
		PreviewOnly:           true,
		SelectedTargetID:      nil,
		Authority:             deviceplacement.SessionRunnerReceiptAuthority{},
	}
	encoded, err := json.Marshal(observation)
	if err != nil {
		t.Fatal(err)
	}
	return string(encoded)
}

func mutateSessionRunnerReceiptObservationBody(t *testing.T, body string, mutate func(map[string]any)) string {
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
