package appserver

import (
	"encoding/json"
	"net/http"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/runtimebridge"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
)

func TestRunnerExecutionIntentPreviewCandidateBindsOwnerAndPath(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	modelOwner := model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
	request := runnerExecutionIntentPreviewRequest(owner, "conversation-001", "run-001")
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-001/runs/run-001/runner-execution-intent/preview"
	response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", string(body))
	if response.Code != http.StatusOK {
		t.Fatalf("Runner execution-intent preview status=%d body=%q", response.Code, response.Body.String())
	}
	var observation deviceplacement.RunnerExecutionIntentObservation
	if err := json.Unmarshal(response.Body.Bytes(), &observation); err != nil {
		t.Fatalf("decode Runner execution-intent preview: %v body=%q", err, response.Body.String())
	}
	if !validRunnerExecutionIntentObservation(observation, modelOwner, "conversation-001", "run-001") ||
		observation.CommandID != "command-001" || observation.TargetID != "runner-1" {
		t.Fatalf("Runner execution-intent preview observation=%#v", observation)
	}
	assertContractHeaders(t, response.Header(), response.Body.Len(), "")

	foreign := requestConversationAPIAs(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "account-foreign", "tenant-slate", "application/json", "", string(body))
	if foreign.Code != http.StatusForbidden {
		t.Fatalf("foreign owner status=%d body=%q", foreign.Code, foreign.Body.String())
	}
	wrongPath := requestConversationAPI(t, routes, identity, http.MethodPost,
		"/api/v1/conversations/other/runs/run-001/runner-execution-intent/preview",
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

func TestRunnerExecutionIntentPreviewCandidateRejectsAmbiguousBodiesAndProductionStaysClosed(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	candidate := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	request := runnerExecutionIntentPreviewRequest(owner, "conversation-001", "run-001")
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-001/runs/run-001/runner-execution-intent/preview"
	for _, test := range []struct {
		name string
		body string
		want int
	}{
		{name: "unknown root field", body: strings.Replace(string(body), `{"owner":`, `{"unexpected":true,"owner":`, 1), want: http.StatusBadRequest},
		{name: "duplicate root field", body: strings.Replace(string(body), `"conversation_id":"conversation-001"`, `"conversation_id":"conversation-001","conversation_id":"conversation-001"`, 1), want: http.StatusBadRequest},
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
		t.Fatalf("production Runner execution-intent status=%d body=%q", response.Code, response.Body.String())
	}
}

func TestRunnerExecutionIntentPreviewBindsOwnerScopedPromptAndRunReferences(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	backend := &fakeConversationBackend{
		promptPage: model.ConversationPromptPage{
			ConversationID: "conversation-001",
			Prompts: []model.ConversationPrompt{{
				ID: "prompt-001", ConversationID: "conversation-001", Role: "user",
				Content: "stored prompt", CreatedAtMS: 200,
			}},
		},
		runPage: runmodel.OwnedRunPage{
			ConversationID: "conversation-001",
			Runs: []runmodel.OwnedRunSummary{{
				RunID: "run-001", PromptID: "prompt-001", CreatedAtMS: 200,
				LatestSequence: 1, Status: "nonterminal",
			}},
		},
	}
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidatesBackend(backend, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	request := runnerExecutionIntentPreviewRequest(owner, "conversation-001", "run-001")
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-001/runs/run-001/runner-execution-intent/preview"
	response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", string(body))
	if response.Code != http.StatusOK {
		t.Fatalf("durable reference binding status=%d body=%q", response.Code, response.Body.String())
	}
	if backend.promptCalls != 1 || backend.runCalls != 1 || backend.promptLimit != runnerExecutionIntentPromptPageLimit || backend.runLimit != runnerExecutionIntentRunPageLimit {
		t.Fatalf("unexpected durable reference reads: prompt_calls=%d run_calls=%d prompt_limit=%d run_limit=%d", backend.promptCalls, backend.runCalls, backend.promptLimit, backend.runLimit)
	}

	backend.promptPage.Prompts[0].ID = "prompt-other"
	missingPrompt := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", string(body))
	if missingPrompt.Code != http.StatusNotFound || !strings.Contains(missingPrompt.Body.String(), `"code":"not_found"`) {
		t.Fatalf("missing durable Prompt status=%d body=%q", missingPrompt.Code, missingPrompt.Body.String())
	}

	backend.promptPage.Prompts[0].ID = "prompt-001"
	backend.err = &runtimebridge.Error{Code: "storage_unavailable"}
	backendFailure := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", string(body))
	if backendFailure.Code != http.StatusServiceUnavailable || !strings.Contains(backendFailure.Body.String(), `"code":"conversation_service_unavailable"`) {
		t.Fatalf("durable reference backend failure status=%d body=%q", backendFailure.Code, backendFailure.Body.String())
	}
}

func runnerExecutionIntentPreviewRequest(owner deviceplacement.Owner, conversationID, runID string) deviceplacement.RunnerExecutionIntentRequest {
	promptID := "prompt-001"
	attemptID := "attempt-001"
	commandID := "command-001"
	targetID := "runner-1"
	idempotencyKey := runID + ":" + attemptID + ":" + commandID
	return deviceplacement.RunnerExecutionIntentRequest{
		Owner: owner, ConversationID: conversationID,
		Prompt: deviceplacement.RunIntentPromptReceipt{
			PromptID: promptID, ConversationID: conversationID, Role: "user", AcceptedAtMS: 200,
			IntentID: "intent-001", InitialEventID: "event-001", InitialEventSequence: 1, InitialEventType: "submitted",
		},
		Run: deviceplacement.RunIntentRunReference{
			RunID: runID, ConversationID: conversationID, PromptID: promptID, CreatedAtMS: 200,
			LatestSequence: 1, Status: "nonterminal",
		},
		Binding: deviceplacement.RunnerExecutionIntentBinding{
			ConversationID: conversationID, PromptID: promptID, RunID: runID, AttemptID: attemptID,
			CommandID: commandID, TargetID: targetID,
			CommandSHA256:  "42ed02a535113450e6f2cc757fb9b4e2cce6143724274191bbae159e9ea8de7a",
			IdempotencyKey: idempotencyKey,
		},
		Command: deviceplacement.RunnerExecutionCommand{
			V: 1, CommandID: commandID,
			LeaseProof:     deviceplacement.RunnerExecutionLeaseProof{AttemptID: attemptID, TargetID: targetID, Epoch: 1, FencingToken: "fence-001"},
			IdempotencyKey: idempotencyKey, WorkspaceRef: "workspace-001",
			Argv: []string{"forge-task", "--prompt-ref", promptID}, TimeoutMS: 5_000, MaxOutputBytes: 65_536,
		},
	}
}
