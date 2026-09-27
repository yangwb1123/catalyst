package appserver

import (
	"encoding/json"
	"net/http"
	"strings"
	"testing"

	"forgeos/forge-core/internal/auditprojection"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"forgeos/forge-core/internal/runtimebridge/runmodel"
)

func TestRunExecutionEvidencePreviewBindsRunAndReceipt(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	conversationID, runID := "conversation-001", "run-001"
	body := runExecutionEvidencePreviewBody(t, identity.issuer, "account-42", "tenant-slate", conversationID, "prompt-001", runID)
	path := "/api/v1/conversations/" + conversationID + "/runs/" + runID + "/execution-evidence/preview"
	response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", body)
	if response.Code != http.StatusOK {
		t.Fatalf("Run execution evidence status=%d body=%q", response.Code, response.Body.String())
	}
	var evidence deviceplacement.RunExecutionEvidence
	if err := json.Unmarshal(response.Body.Bytes(), &evidence); err != nil {
		t.Fatalf("decode Run execution evidence: %v body=%q", err, response.Body.String())
	}
	if evidence.SchemaVersion != deviceplacement.RunExecutionEvidenceSchemaVersion ||
		evidence.ConversationID != conversationID || evidence.RunID != runID ||
		evidence.PromptID != "prompt-001" || evidence.DispositionKind != "completed" ||
		!evidence.MetadataObserved || evidence.ContentIncluded ||
		evidence.Authority != (deviceplacement.RunExecutionEvidenceAuthority{}) {
		t.Fatalf("unexpected Run execution evidence: %#v", evidence)
	}
	if strings.Contains(response.Body.String(), "issuer") || strings.Contains(response.Body.String(), "fencing_token") || strings.Contains(response.Body.String(), "prompt_content") {
		t.Fatalf("Run execution evidence leaked owner/proof/content: %q", response.Body.String())
	}

	foreign := requestConversationAPIAs(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "account-foreign", "tenant-slate", "application/json", "", body)
	if foreign.Code != http.StatusForbidden {
		t.Fatalf("foreign owner status=%d body=%q", foreign.Code, foreign.Body.String())
	}
	wrongPath := requestConversationAPI(t, routes, identity, http.MethodPost,
		"/api/v1/conversations/other/runs/"+runID+"/execution-evidence/preview",
		"forge:conversations:read", "application/json", "", body)
	if wrongPath.Code != http.StatusBadRequest {
		t.Fatalf("foreign path status=%d body=%q", wrongPath.Code, wrongPath.Body.String())
	}
}

func TestRunExecutionEvidencePreviewKeepsCandidateClosedAndReadOnly(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	path := "/api/v1/conversations/conversation-1/runs/run-1/execution-evidence/preview"
	body := runExecutionEvidencePreviewBody(t, identity.issuer, "account-42", "tenant-slate", "conversation-1", "prompt-1", "run-1")
	closed := requestConversationAPI(t, production, identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", body)
	if closed.Code != http.StatusNotFound {
		t.Fatalf("production candidate status=%d body=%q", closed.Code, closed.Body.String())
	}

	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions)
	if err != nil {
		t.Fatal(err)
	}
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
}

func TestRunExecutionEvidencePreviewRejectsConfusedInputs(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-1/runs/run-1/execution-evidence/preview"
	base := runExecutionEvidencePreviewBody(t, identity.issuer, "account-42", "tenant-slate", "conversation-1", "prompt-1", "run-1")
	tests := []struct {
		name string
		body string
	}{
		{name: "unknown root", body: mutateRunExecutionEvidencePreviewBody(t, base, func(object map[string]any) { object["unexpected"] = true })},
		{name: "receipt binding", body: mutateRunExecutionEvidencePreviewBody(t, base, func(object map[string]any) {
			receipt := object["session_receipt_observed"].(map[string]any)
			receipt["run_id"] = "run-foreign"
		})},
		{name: "run authority", body: mutateRunExecutionEvidencePreviewBody(t, base, func(object map[string]any) {
			run := object["run_observed"].(map[string]any)
			authority := run["authority"].(map[string]any)
			authority["execution_authorized"] = true
		})},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
				"forge:conversations:read", "application/json", "", test.body)
			if response.Code != http.StatusBadRequest {
				t.Fatalf("status=%d body=%q", response.Code, response.Body.String())
			}
		})
	}
}

func runExecutionEvidencePreviewBody(t *testing.T, issuer, subject, tenant, conversationID, promptID, runID string) string {
	t.Helper()
	owner := model.Owner{Issuer: issuer, Subject: subject, TenantID: tenant}
	run, err := auditprojection.ProjectRunObserved(owner, conversationID, runmodel.OwnedRunSummary{
		RunID: runID, PromptID: promptID, CreatedAtMS: 200, LatestSequence: 5, Status: "completed",
	})
	if err != nil {
		t.Fatal(err)
	}
	var receipt deviceplacement.SessionRunnerReceiptObservation
	if err := json.Unmarshal([]byte(sessionRunnerReceiptObservationPreviewBody(t, issuer, subject, tenant, conversationID, promptID, runID)), &receipt); err != nil {
		t.Fatal(err)
	}
	encoded, err := json.Marshal(struct {
		RunObserved            auditprojection.RunObserved                     `json:"run_observed"`
		SessionReceiptObserved deviceplacement.SessionRunnerReceiptObservation `json:"session_receipt_observed"`
	}{RunObserved: run, SessionReceiptObserved: receipt})
	if err != nil {
		t.Fatal(err)
	}
	return string(encoded)
}

func mutateRunExecutionEvidencePreviewBody(t *testing.T, body string, mutate func(map[string]any)) string {
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
