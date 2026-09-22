package appserver

import (
	"encoding/json"
	"net/http"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceplacement"
)

func TestRunnerDispatchPlanPreviewCandidateBindsOwnerAndPath(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	preflight := runAttemptLeaseDispatchPreflightRequest(owner, "conversation-1", "run-1")
	body, err := json.Marshal(preflight.DispatchPlan)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-1/runs/run-1/runner-dispatch-plan-preview"
	response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", string(body))
	if response.Code != http.StatusOK {
		t.Fatalf("dispatch-plan preview status=%d body=%q", response.Code, response.Body.String())
	}
	var observation deviceplacement.RunnerDispatchPlanPreviewObservation
	if err := json.Unmarshal(response.Body.Bytes(), &observation); err != nil {
		t.Fatalf("decode dispatch-plan preview: %v body=%q", err, response.Body.String())
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate dispatch-plan preview: %v body=%q", err, response.Body.String())
	}
	if observation.Owner != owner || observation.ConversationID != "conversation-1" ||
		observation.RunID != "run-1" || observation.SelectedTargetID != nil ||
		observation.Authority != (deviceplacement.RunnerDispatchPlanPreviewAuthority{}) {
		t.Fatalf("dispatch-plan preview observation=%#v", observation)
	}
	assertContractHeaders(t, response.Header(), response.Body.Len(), "")

	foreign := requestConversationAPIAs(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "account-foreign", "tenant-slate", "application/json", "", string(body))
	if foreign.Code != http.StatusForbidden {
		t.Fatalf("foreign owner status=%d body=%q", foreign.Code, foreign.Body.String())
	}
	wrongPath := requestConversationAPI(t, routes, identity, http.MethodPost,
		"/api/v1/conversations/other/runs/run-1/runner-dispatch-plan-preview",
		"forge:conversations:read", "application/json", "", string(body))
	if wrongPath.Code != http.StatusBadRequest {
		t.Fatalf("wrong path status=%d body=%q", wrongPath.Code, wrongPath.Body.String())
	}
	method := requestConversationAPI(t, routes, identity, http.MethodGet, path,
		"forge:conversations:read", "", "", "")
	if method.Code != http.StatusMethodNotAllowed || method.Header().Get("Allow") != http.MethodPost {
		t.Fatalf("method status=%d allow=%q body=%q", method.Code, method.Header().Get("Allow"), method.Body.String())
	}
	query := requestConversationAPI(t, routes, identity, http.MethodPost, path+"?debug=1",
		"forge:conversations:read", "application/json", "", string(body))
	if query.Code != http.StatusBadRequest {
		t.Fatalf("query status=%d body=%q", query.Code, query.Body.String())
	}
	noScope := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:write", "application/json", "", string(body))
	if noScope.Code != http.StatusForbidden {
		t.Fatalf("scope status=%d body=%q", noScope.Code, noScope.Body.String())
	}
}

func TestRunnerDispatchPlanPreviewCandidateRejectsAmbiguousBodiesAndProductionStaysClosed(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	candidate := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	preflight := runAttemptLeaseDispatchPreflightRequest(owner, "conversation-1", "run-1")
	body, err := json.Marshal(preflight.DispatchPlan)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-1/runs/run-1/runner-dispatch-plan-preview"
	for _, test := range []struct {
		name string
		body string
		want int
	}{
		{name: "unknown root field", body: strings.Replace(string(body), `{"attempt_state":`, `{"unexpected":true,"attempt_state":`, 1), want: http.StatusBadRequest},
		{name: "duplicate root field", body: strings.Replace(string(body), `"attempt_state":"accepted"`, `"attempt_state":"accepted","attempt_state":"accepted"`, 1), want: http.StatusBadRequest},
		{name: "duplicate nested field", body: strings.Replace(string(body), `"attempt_state":"accepted"`, `"attempt_state":"accepted","attempt_state":"accepted"`, 1), want: http.StatusBadRequest},
		{name: "malformed body", body: `{}`, want: http.StatusBadRequest},
	} {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(t, candidate, identity, http.MethodPost, path,
				"forge:conversations:read", "application/json", "", test.body)
			if response.Code != test.want {
				t.Fatalf("status=%d body=%q want=%d", response.Code, response.Body.String(), test.want)
			}
		})
	}

	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	response := requestConversationAPI(t, production, identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", string(body))
	if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
		t.Fatalf("production dispatch-plan status=%d body=%q", response.Code, response.Body.String())
	}
}
