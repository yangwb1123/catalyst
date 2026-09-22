package appserver

import (
	"encoding/json"
	"net/http"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/executionlease"
	"forgeos/forge-core/internal/executionreconcile"
)

func TestExecutionReconciliationPreviewCandidateBindsOwnerAndPath(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	routes := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	input := executionReconciliationInput(t, owner, "conversation-1", "run-1")
	body, err := json.Marshal(input)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-1/runs/run-1/execution-reconciliation/preview"
	response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", string(body))
	if response.Code != http.StatusOK {
		t.Fatalf("execution reconciliation status=%d body=%q", response.Code, response.Body.String())
	}
	var observation executionreconcile.Observation
	if err := json.Unmarshal(response.Body.Bytes(), &observation); err != nil {
		t.Fatalf("decode execution reconciliation: %v body=%q", err, response.Body.String())
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate execution reconciliation: %v body=%q", err, response.Body.String())
	}
	if observation.Owner != owner || observation.ConversationID != "conversation-1" ||
		observation.RunID != "run-1" || observation.NextObservation != "await_terminal" ||
		observation.AutomaticRetry || observation.Authority != (executionreconcile.Authority{}) {
		t.Fatalf("execution reconciliation observation=%#v", observation)
	}
	assertContractHeaders(t, response.Header(), response.Body.Len(), "")

	foreign := requestConversationAPIAs(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "account-foreign", "tenant-slate", "application/json", "", string(body))
	if foreign.Code != http.StatusForbidden {
		t.Fatalf("foreign owner status=%d body=%q", foreign.Code, foreign.Body.String())
	}
	wrongPath := requestConversationAPI(t, routes, identity, http.MethodPost,
		"/api/v1/conversations/other/runs/run-1/execution-reconciliation/preview",
		"forge:conversations:read", "application/json", "", string(body))
	if wrongPath.Code != http.StatusBadRequest {
		t.Fatalf("wrong path status=%d body=%q", wrongPath.Code, wrongPath.Body.String())
	}
	if method := requestConversationAPI(t, routes, identity, http.MethodGet, path,
		"forge:conversations:read", "", "", ""); method.Code != http.StatusMethodNotAllowed || method.Header().Get("Allow") != http.MethodPost {
		t.Fatalf("method status=%d allow=%q body=%q", method.Code, method.Header().Get("Allow"), method.Body.String())
	}
	if query := requestConversationAPI(t, routes, identity, http.MethodPost, path+"?debug=1",
		"forge:conversations:read", "application/json", "", string(body)); query.Code != http.StatusBadRequest {
		t.Fatalf("query status=%d body=%q", query.Code, query.Body.String())
	}
	if noScope := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:write", "application/json", "", string(body)); noScope.Code != http.StatusForbidden {
		t.Fatalf("scope status=%d body=%q", noScope.Code, noScope.Body.String())
	}
}

func TestExecutionReconciliationPreviewCandidateRejectsUnsafeInputAndProductionStaysClosed(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	candidate := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	input := executionReconciliationInput(t, owner, "conversation-1", "run-1")
	body, err := json.Marshal(input)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-1/runs/run-1/execution-reconciliation/preview"
	for _, test := range []struct {
		name string
		body string
		want int
	}{
		{name: "unknown root field", body: strings.Replace(string(body), `{"owner":`, `{"unexpected":true,"owner":`, 1), want: http.StatusBadRequest},
		{name: "duplicate root field", body: strings.Replace(string(body), `"run_status":"nonterminal"`, `"run_status":"nonterminal","run_status":"nonterminal"`, 1), want: http.StatusBadRequest},
		{name: "unsafe epoch", body: strings.Replace(string(body), `"epoch":1`, `"epoch":9007199254740992`, 1), want: http.StatusBadRequest},
		{name: "missing terminal key", body: strings.Replace(string(body), `,"terminal":null`, "", 1), want: http.StatusBadRequest},
		{name: "malformed body", body: `{}`, want: http.StatusBadRequest},
	} {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(t, candidate, identity, http.MethodPost, path,
				"forge:conversations:read", "application/json", "", test.body)
			if response.Code != test.want {
				t.Fatalf("status=%d want=%d body=%q", response.Code, test.want, response.Body.String())
			}
		})
	}

	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	response := requestConversationAPI(t, production, identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", string(body))
	if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
		t.Fatalf("production execution reconciliation status=%d body=%q", response.Code, response.Body.String())
	}
}

func executionReconciliationInput(t *testing.T, owner deviceplacement.Owner, conversationID, runID string) executionreconcile.Input {
	t.Helper()
	lease, err := executionlease.Issue("attempt-1", "runner-1", 1, "fence-1", 100, 10_000)
	if err != nil {
		t.Fatal(err)
	}
	return executionreconcile.Input{
		Owner: owner, ConversationID: conversationID, RunID: runID,
		AttemptID: "attempt-1", CommandID: "command-1", TargetID: "runner-1",
		RunStatus: "nonterminal", AttemptState: "running", Lease: lease,
		ObservedAtMS: 200,
	}
}
