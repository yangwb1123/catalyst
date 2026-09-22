package appserver

import (
	"encoding/json"
	"net/http"
	"strings"
	"testing"

	"forgeos/forge-core/internal/auditprojection"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
)

func TestRunObservedCandidateProjectsOneOwnerRun(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	backend := &fakeConversationBackend{
		runObservation: mustRunObserved(t, owner, "conversation-1", runmodel.OwnedRunSummary{
			RunID: "run-1", PromptID: "prompt-1", CreatedAtMS: 200,
			LatestSequence: 5, Status: "completed",
		}),
		runPage: runmodel.OwnedRunPage{
			ConversationID: "conversation-1",
			Runs: []runmodel.OwnedRunSummary{{
				RunID: "run-1", PromptID: "prompt-1", CreatedAtMS: 200,
				LatestSequence: 5, Status: "completed",
			}},
		},
	}
	routes := authenticator.Handler(
		newAuthenticatedSessionRoutesWithObservationCandidatesBackend(backend, nil),
	)
	path := "/api/v1/conversations/conversation-1/runs/run-1/observation"
	response := requestConversationAPI(t, routes, identity, http.MethodGet, path,
		"forge:conversations:read", "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("Run observation status=%d body=%q", response.Code, response.Body.String())
	}
	var value auditprojection.RunObserved
	if err := json.Unmarshal(response.Body.Bytes(), &value); err != nil {
		t.Fatalf("decode Run observation: %v body=%q", err, response.Body.String())
	}
	want, err := auditprojection.ProjectRunObserved(owner, "conversation-1", backend.runPage.Runs[0])
	if err != nil || value != want {
		t.Fatalf("Run observation=%#v want=%#v err=%v", value, want, err)
	}
	if backend.runCalls != 1 || backend.runOwner != owner || backend.runID != "conversation-1" {
		t.Fatalf("Run observation backend calls=%d owner=%#v conversation=%q", backend.runCalls, backend.runOwner, backend.runID)
	}
	assertContractHeaders(t, response.Header(), response.Body.Len(), "")
}

func TestRunObservedCandidateRejectsUnsafeBackendProjection(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	backend := &fakeConversationBackend{
		runObservation: auditprojection.RunObserved{
			APIVersion:       auditprojection.ForgeRunObservedV1,
			OwnerRef:         auditprojection.ObservedOwnerReference(owner),
			ConversationID:   "conversation-1",
			RunID:            "run-1",
			PromptID:         "prompt-1",
			CreatedAtMS:      maxSafeJSONInteger + 1,
			LatestSequence:   1,
			Status:           "completed",
			MetadataObserved: true,
		},
	}
	routes := authenticator.Handler(newRunObservedRoutes(backend))
	response := requestConversationAPI(t, routes, identity, http.MethodGet,
		"/api/v1/conversations/conversation-1/runs/run-1/observation",
		"forge:conversations:read", "", "", "")
	if response.Code != http.StatusBadGateway || backend.runCalls != 1 ||
		!strings.Contains(response.Body.String(), `"code":"conversation_service_error"`) {
		t.Fatalf("unsafe Run observation status=%d calls=%d body=%q", response.Code, backend.runCalls, response.Body.String())
	}
}

func TestRunObservedCandidateKeepsForeignAndMalformedRequestsClosed(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	backend := &fakeConversationBackend{runPage: runmodel.OwnedRunPage{
		ConversationID: "conversation-1", Runs: []runmodel.OwnedRunSummary{},
	}}
	routes := authenticator.Handler(
		newAuthenticatedSessionRoutesWithObservationCandidatesBackend(backend, nil),
	)
	path := "/api/v1/conversations/conversation-1/runs/run-1/observation"
	foreign := requestConversationAPIAs(t, routes, identity, http.MethodGet, path,
		"forge:conversations:read", "account-foreign", "tenant-slate", "", "", "")
	if foreign.Code != http.StatusForbidden {
		t.Fatalf("foreign Run observation status=%d body=%q", foreign.Code, foreign.Body.String())
	}
	for _, test := range []struct {
		name, method, requestPath, body, contentType string
		wantStatus                                   int
	}{
		{name: "method", method: http.MethodPost, requestPath: path, body: `{}`, contentType: "application/json", wantStatus: http.StatusMethodNotAllowed},
		{name: "query", method: http.MethodGet, requestPath: path + "?debug=1", wantStatus: http.StatusBadRequest},
		{name: "body", method: http.MethodGet, requestPath: path, body: `{}`, contentType: "application/json", wantStatus: http.StatusBadRequest},
		{name: "unknown run", method: http.MethodGet, requestPath: path, wantStatus: http.StatusNotFound},
	} {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(t, routes, identity, test.method, test.requestPath,
				"forge:conversations:read", test.contentType, "", test.body)
			if response.Code != test.wantStatus {
				t.Fatalf("status=%d want=%d body=%q", response.Code, test.wantStatus, response.Body.String())
			}
			if test.name == "method" && response.Header().Get("Allow") != http.MethodGet {
				t.Fatalf("Allow=%q", response.Header().Get("Allow"))
			}
		})
	}
}

func TestRunObservedCandidateRemainsClosedByProductionSessionConstructor(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	routes := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	response := requestConversationAPI(t, routes, identity, http.MethodGet,
		"/api/v1/conversations/conversation-1/runs/run-1/observation",
		"forge:conversations:read", "", "", "")
	if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
		t.Fatalf("production Run observation status=%d body=%q", response.Code, response.Body.String())
	}
}

func mustRunObserved(t *testing.T, owner model.Owner, conversationID string, summary runmodel.OwnedRunSummary) auditprojection.RunObserved {
	t.Helper()
	observed, err := auditprojection.ProjectRunObserved(owner, conversationID, summary)
	if err != nil {
		t.Fatal(err)
	}
	return observed
}
