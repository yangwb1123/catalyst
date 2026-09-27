package appserver

import (
	"encoding/json"
	"net/http"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceplacement"
)

func TestSessionRunnerReconciliationProjectionPreviewBindsOwnerAndPath(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	routes := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	body := sessionRunnerReceiptHistoryPreviewBody(t, identity)
	path := "/api/v1/conversations/conversation-001/runs/run-001/runner-reconciliation/preview"
	response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		sessionRunnerReconciliationProjectionScope, "application/json", "", body)
	if response.Code != http.StatusOK {
		t.Fatalf("status=%d body=%q", response.Code, response.Body.String())
	}
	var projection deviceplacement.SessionRunnerReconciliationProjection
	if err := json.Unmarshal(response.Body.Bytes(), &projection); err != nil {
		t.Fatal(err)
	}
	if err := projection.Validate(); err != nil || projection.Source.AttemptCount != 2 ||
		projection.LatestDispositionKind != "uncertain" || projection.AutomaticRetry ||
		projection.SelectedTargetID != nil || projection.Authority != (deviceplacement.SessionRunnerReceiptHistoryAuthority{}) {
		t.Fatalf("invalid projection: validate=%v value=%#v", err, projection)
	}

	wrongPath := "/api/v1/conversations/other/runs/run-001/runner-reconciliation/preview"
	if got := requestConversationAPI(t, routes, identity, http.MethodPost, wrongPath,
		sessionRunnerReconciliationProjectionScope, "application/json", "", body); got.Code != http.StatusBadRequest {
		t.Fatalf("path mismatch status=%d body=%q", got.Code, got.Body.String())
	}
	wrongOwner := strings.Replace(body, `"subject":"account-42"`, `"subject":"other"`, 1)
	if got := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		sessionRunnerReconciliationProjectionScope, "application/json", "", wrongOwner); got.Code != http.StatusForbidden {
		t.Fatalf("owner mismatch status=%d body=%q", got.Code, got.Body.String())
	}
}

func TestSessionRunnerReconciliationProjectionPreviewRejectsDriftAndProductionStaysClosed(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	candidate := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	body := sessionRunnerReceiptHistoryPreviewBody(t, identity)
	path := "/api/v1/conversations/conversation-001/runs/run-001/runner-reconciliation/preview"
	for _, test := range []struct {
		name, body string
	}{
		{name: "unknown root field", body: strings.Replace(body, `{"schema_version":`, `{"unexpected":true,"schema_version":`, 1)},
		{name: "duplicate root field", body: strings.Replace(body, `"attempt_count":2`, `"attempt_count":2,"attempt_count":2`, 1)},
		{name: "non uncertain", body: strings.Replace(body, `"latest_disposition_kind":"uncertain"`, `"latest_disposition_kind":"completed"`, 1)},
		{name: "malformed", body: `{}`},
	} {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(t, candidate, identity, http.MethodPost, path,
				sessionRunnerReconciliationProjectionScope, "application/json", "", test.body)
			if response.Code != http.StatusBadRequest {
				t.Fatalf("status=%d body=%q", response.Code, response.Body.String())
			}
		})
	}
	if got := requestConversationAPI(t, candidate, identity, http.MethodGet, path,
		sessionRunnerReconciliationProjectionScope, "", "", ""); got.Code != http.StatusMethodNotAllowed || got.Header().Get("Allow") != http.MethodPost {
		t.Fatalf("method status=%d allow=%q", got.Code, got.Header().Get("Allow"))
	}
	if got := requestConversationAPI(t, candidate, identity, http.MethodPost, path+"?debug=1",
		sessionRunnerReconciliationProjectionScope, "application/json", "", body); got.Code != http.StatusBadRequest {
		t.Fatalf("query status=%d body=%q", got.Code, got.Body.String())
	}
	if got := requestConversationAPI(t, candidate, identity, http.MethodPost, path,
		"forge:conversations:write", "application/json", "", body); got.Code != http.StatusForbidden {
		t.Fatalf("scope status=%d body=%q", got.Code, got.Body.String())
	}
	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	if got := requestConversationAPI(t, production, identity, http.MethodPost, path,
		sessionRunnerReconciliationProjectionScope, "application/json", "", body); got.Code != http.StatusNotFound || got.Body.String() != string(notFoundBody) {
		t.Fatalf("production status=%d body=%q", got.Code, got.Body.String())
	}
}
