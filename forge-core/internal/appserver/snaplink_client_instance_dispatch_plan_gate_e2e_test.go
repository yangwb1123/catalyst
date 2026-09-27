package appserver

// This opt-in acceptance test crosses a real Snaplink JWT through the
// Flutter Sessions Gate. The Gate refreshes the selected client-instance
// session/resource pair before posting the display-only Runner dispatch-plan
// candidate. Hidden and drifted declarations fail closed; the production
// constructor remains a 404 for every candidate route.

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
)

func TestSnaplinkAuthenticatedClientInstanceDispatchPlanGateE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_DISPATCH_PLAN_GATE_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_DISPATCH_PLAN_GATE_E2E=1 for Flutter Sessions Gate dispatch-plan E2E")
	}
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)
	authenticator, err := authn.New(authn.Config{
		Issuer: issuer, Audience: snaplinkForgeTestAudience,
		JWKSURL:          issuer + "/.well-known/jwks.json",
		ExpectedTenantID: snaplinkForgeTestTenant, ExpectedSubjectID: snaplinkForgeTestUser,
		JWKSHTTPClient: ssoClient, JWKSRefreshInterval: 24 * time.Hour,
	})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(authenticator.Close)

	owner := deviceplacement.Owner{
		Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant,
	}
	preflight := runAttemptLeaseDispatchPreflightRequest(owner, "conversation-001", "run-001")
	for _, testCase := range []struct {
		name       string
		mode       string
		instanceID string
		expectCard bool
		expectPost bool
	}{
		{name: "visible", mode: "visible", instanceID: "client-cli-001", expectCard: true, expectPost: true},
		{name: "hidden", mode: "hidden", instanceID: "client-web-001", expectCard: false, expectPost: false},
		{name: "drift", mode: "drift", instanceID: "client-cli-001", expectCard: false, expectPost: false},
		{name: "target-drift", mode: "target-drift", instanceID: "client-cli-001", expectCard: false, expectPost: true},
	} {
		testCase := testCase
		t.Run(testCase.name, func(t *testing.T) {
			sessionView := gateDispatchPlanSessionView(owner, testCase.mode)
			resourceView := gateDispatchPlanResourceView(owner, testCase.mode)
			sessionSource := &fixtureClientInstanceSessionViewSource{value: sessionView}
			resourceSource := &fixtureClientInstanceResourceViewSource{value: resourceView}
			backend := gateDispatchPlanConversationBackend()
			baseRoutes := newAuthenticatedSessionRoutesWithObservationCandidatesBackend(backend, nil)
			dispatchCandidate := newRunnerDispatchPlanPreviewRoutes()
			testRoutes := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				switch {
				case r.URL.EscapedPath() == clientInstanceSessionViewCandidatePath:
					newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
						Enabled: true, Source: sessionSource,
					}).ServeHTTP(w, r)
				case r.URL.EscapedPath() == clientInstanceResourceViewCandidatePath:
					newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
						Enabled: true, Source: resourceSource,
					}).ServeHTTP(w, r)
				case strings.HasSuffix(r.URL.EscapedPath(), "/runner-dispatch-plan-preview"):
					dispatchCandidate.ServeHTTP(w, r)
				default:
					baseRoutes.ServeHTTP(w, r)
				}
			})
			requestRecorder := &conversationHTTPRecorder{}
			server := httptest.NewServer(authenticator.Handler(requestRecorder.wrap(testRoutes)))
			t.Cleanup(server.Close)

			runForgeConsoleClientInstanceDispatchPlanGateE2E(
				t, server.URL, token, owner, testCase.instanceID, testCase.expectCard, preflight,
			)
			requests := requestRecorder.snapshot()
			var sessionReads, resourceReads, dispatchPosts []recordedConversationRequest
			for _, request := range requests {
				switch {
				case request.method == http.MethodGet && request.path == clientInstanceSessionViewCandidatePath:
					sessionReads = append(sessionReads, request)
				case request.method == http.MethodGet && request.path == clientInstanceResourceViewCandidatePath:
					resourceReads = append(resourceReads, request)
				case request.method == http.MethodPost && strings.HasSuffix(request.path, "/runner-dispatch-plan-preview"):
					dispatchPosts = append(dispatchPosts, request)
				}
			}
			if len(sessionReads) == 0 || len(resourceReads) == 0 {
				t.Fatalf("Sessions Gate did not read both client-instance projections: %#v", requests)
			}
			if testCase.expectPost {
				if len(dispatchPosts) != 1 {
					t.Fatalf("visible Sessions Gate dispatch POST count=%d requests=%#v", len(dispatchPosts), requests)
				}
				lastSession, lastResource := lastRequestIndex(requests, clientInstanceSessionViewCandidatePath), lastRequestIndex(requests, clientInstanceResourceViewCandidatePath)
				postIndex := lastRequestIndex(requests, dispatchPosts[0].path)
				if lastSession < 0 || lastResource < 0 || postIndex < 0 || lastSession > postIndex || lastResource > postIndex {
					t.Fatalf("dispatch POST did not follow the latest converged pair: %#v", requests)
				}
			} else if len(dispatchPosts) != 0 {
				t.Fatalf("%s declaration escaped the Sessions Gate pair guard: %#v", testCase.mode, requests)
			}
			if testCase.expectCard && len(dispatchPosts) != 1 {
				t.Fatalf("visible Sessions Gate dispatch card had no candidate POST: %#v", requests)
			}
		})
	}

	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	for _, request := range []struct {
		method string
		path   string
		scope  string
	}{
		{method: http.MethodGet, path: clientInstanceSessionViewCandidatePath, scope: "forge:conversations:read"},
		{method: http.MethodGet, path: clientInstanceResourceViewCandidatePath, scope: deviceInventoryReadCandidateScope},
		{method: http.MethodPost, path: "/api/v1/conversations/conversation-001/runs/run-001/runner-dispatch-plan-preview", scope: "forge:conversations:read"},
	} {
		body := ""
		contentType := ""
		if request.method == http.MethodPost {
			body = "{}"
			contentType = "application/json"
		}
		response := requestConversationAPIToken(t, production, token, request.method, request.path, contentType, "", body)
		if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
			t.Fatalf("default production %s %s status=%d body=%q", request.method, request.path, response.Code, response.Body.String())
		}
	}
}

func gateDispatchPlanSessionView(owner deviceplacement.Owner, mode string) deviceplacement.ClientInstanceSessionViewObservation {
	instances := gateDispatchPlanInstances(mode, false)
	view, err := deviceplacement.ObserveClientInstanceSessionView(deviceplacement.ClientInstanceSessionViewRequest{
		Owner: owner, Instances: instances,
	})
	if err != nil {
		panic(err)
	}
	return view
}

func gateDispatchPlanResourceView(owner deviceplacement.Owner, mode string) deviceplacement.ClientInstanceResourceViewObservation {
	view := clientInstanceDispatchPlanResourceView(owner)
	view.Instances = gateDispatchPlanInstances(mode, true)
	if mode == "target-drift" {
		// Keep the owner/session declaration converged while removing the
		// target named by the dispatch-plan response from the resource image.
		view.Devices = append([]deviceplacement.ClientInstanceResourceViewDevice(nil), view.Devices[1:]...)
	}
	if err := view.Validate(); err != nil {
		panic(err)
	}
	return view
}

func gateDispatchPlanInstances(mode string, resource bool) []deviceplacement.ClientInstanceSessionViewInstance {
	instances := []deviceplacement.ClientInstanceSessionViewInstance{
		{InstanceID: "client-app-001", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{"conversation-001"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-cli-001", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{"conversation-001"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-mobile-001", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{"conversation-001"}, ObservedAtMS: 200_500, Status: "idle"},
		{InstanceID: "client-tui-001", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{"conversation-001"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-web-001", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{"conversation-001"}, ObservedAtMS: 200_500, Status: "idle"},
	}
	if mode == "hidden" {
		for index := range instances {
			if instances[index].InstanceID == "client-web-001" {
				instances[index].SessionIDs = nil
			}
		}
	}
	if mode == "drift" && resource {
		for index := range instances {
			if instances[index].InstanceID == "client-cli-001" {
				instances[index].SessionIDs = nil
			}
		}
	}
	return instances
}

func gateDispatchPlanConversationBackend() *fakeConversationBackend {
	conversation := model.Conversation{
		ID: "conversation-001", Scope: model.ConversationScope{Kind: "global"},
		Title: "Shared dispatch plan", CreatedAtMS: 1, UpdatedAtMS: 2,
	}
	return &fakeConversationBackend{
		listPage: model.OwnedConversationPage{
			Conversations: []model.OwnedConversationEntry{{Conversation: conversation, AggregateVersion: 1}},
		},
		detail: model.OwnedConversationEntry{Conversation: conversation, AggregateVersion: 1},
		promptPage: model.ConversationPromptPage{
			ConversationID: conversation.ID,
			Prompts:        []model.ConversationPrompt{},
		},
		runPage: runmodel.OwnedRunPage{
			ConversationID: conversation.ID,
			Runs:           []runmodel.OwnedRunSummary{{RunID: "run-001", PromptID: "prompt-001", CreatedAtMS: 10, LatestSequence: 1, Status: "nonterminal"}},
		},
		timelinePage: runmodel.OwnedRunTimelinePage{
			ConversationID: conversation.ID, RunID: "run-001", AfterSequence: 0, ScannedThroughSequence: 0,
			Events: []runmodel.OwnedRunEventSummary{},
		},
	}
}

func lastRequestIndex(requests []recordedConversationRequest, path string) int {
	for index := len(requests) - 1; index >= 0; index-- {
		if requests[index].path == path || strings.HasSuffix(requests[index].path, "/runner-dispatch-plan-preview") && path == requests[index].path {
			return index
		}
	}
	return -1
}

func runForgeConsoleClientInstanceDispatchPlanGateE2E(
	t *testing.T,
	apiURL, token string,
	owner deviceplacement.Owner,
	instanceID string,
	expectCard bool,
	request deviceplacement.RunAttemptLeaseDispatchPreflightRequest,
) {
	t.Helper()
	consoleRoot := os.Getenv("SNAPLINK_CONSOLE_ROOT")
	if consoleRoot == "" {
		workingDirectory, err := os.Getwd()
		if err != nil {
			t.Fatal(err)
		}
		repoRoot := filepath.Clean(filepath.Join(workingDirectory, "..", "..", ".."))
		consoleRoot = filepath.Join(filepath.Dir(repoRoot), "workspace", "demo", "snaplink-console")
	}
	if _, err := os.Stat(filepath.Join(consoleRoot, "pubspec.yaml")); err != nil {
		t.Fatalf("SNAPLINK_CONSOLE_ROOT must name the Flutter Console repository: %v", err)
	}
	flutterBinary := os.Getenv("FLUTTER_BIN")
	if flutterBinary == "" {
		flutterBinary = "flutter"
	}
	flutterExecutable, err := exec.LookPath(flutterBinary)
	if err != nil {
		t.Fatalf("Flutter is required for Sessions Gate dispatch-plan E2E: %v", err)
	}
	inputJSON, err := json.Marshal(struct {
		APIURL      string                                                  `json:"api_url"`
		AccessToken string                                                  `json:"access_token"`
		Owner       deviceplacement.Owner                                   `json:"owner"`
		InstanceID  string                                                  `json:"instance_id"`
		ExpectCard  bool                                                    `json:"expect_dispatch_card"`
		Request     deviceplacement.RunAttemptLeaseDispatchPreflightRequest `json:"request"`
	}{
		APIURL: apiURL, AccessToken: token, Owner: owner, InstanceID: instanceID,
		ExpectCard: expectCard, Request: request,
	})
	if err != nil {
		t.Fatalf("encode Sessions Gate dispatch input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "client-instance-dispatch-plan-gate-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Sessions Gate dispatch input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub",
		"--dart-define=FORGE_CONVERSATIONS_API_ORIGIN="+apiURL,
		"test/forge_client_instance_dispatch_plan_gate_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_CLIENT_INSTANCE_DISPATCH_PLAN_GATE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter Sessions Gate dispatch-plan E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("Sessions Gate dispatch-plan Flutter output exceeded the size limit")
	}
}
