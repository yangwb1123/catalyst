package appserver

// This opt-in acceptance test crosses one real Snaplink JWT through the
// shared Flutter Web/App/Mobile Sessions Gate. Every visible client refreshes
// its selected client-instance pair and inventory/resource image before the
// metadata-only Runner execution-intent candidate; hidden and drifted views
// stop before inventory or candidate transport.

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
)

type executionIntentGateHarness struct {
	server          *httptest.Server
	production      http.Handler
	token           string
	owner           deviceplacement.Owner
	request         deviceplacement.RunnerExecutionIntentRequest
	intentPath      string
	sessionSource   *fixtureClientInstanceSessionViewSource
	resourceSource  *fixtureClientInstanceResourceViewSource
	inventorySource *fixtureDeviceInventoryReadV2Source
	recorder        *conversationHTTPRecorder
}

func TestSnaplinkAuthenticatedClientInstanceRunnerExecutionIntentGateE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_RUNNER_EXECUTION_INTENT_GATE_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_RUNNER_EXECUTION_INTENT_GATE_E2E=1 for Console execution-intent Gate E2E")
	}
	harness := newExecutionIntentGateHarness(t)
	for _, testCase := range []struct {
		name        string
		mode        string
		clientKinds []string
		expectPosts int
	}{
		{name: "visible-web-app-mobile", mode: "visible", clientKinds: []string{"web", "app", "mobile"}, expectPosts: 3},
		{name: "hidden-web", mode: "hidden", clientKinds: []string{"web"}, expectPosts: 0},
		{name: "resource-drift-web", mode: "drift", clientKinds: []string{"web"}, expectPosts: 0},
	} {
		t.Run(testCase.name, func(t *testing.T) {
			runExecutionIntentGateCase(t, harness, testCase.mode, testCase.clientKinds, testCase.expectPosts)
		})
	}
	assertExecutionIntentGateProductionClosed(t, harness)
}

func newExecutionIntentGateHarness(t *testing.T) *executionIntentGateHarness {
	t.Helper()
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
	sessionSource := &fixtureClientInstanceSessionViewSource{}
	resourceSource := &fixtureClientInstanceResourceViewSource{}
	inventorySource := &fixtureDeviceInventoryReadV2Source{}
	backend := gateDispatchPlanConversationBackend()
	baseRoutes := newAuthenticatedSessionRoutesWithObservationCandidatesBackend(backend, nil)
	intentPath := "/api/v1/conversations/conversation-001/runs/run-001/runner-execution-intent/preview"
	recorder := &conversationHTTPRecorder{}
	server := httptest.NewServer(authenticator.Handler(recorder.wrap(executionIntentGateRoutes(
		baseRoutes, sessionSource, resourceSource, inventorySource, intentPath,
	))))
	t.Cleanup(server.Close)
	return &executionIntentGateHarness{
		server: server, production: authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil)),
		token: token, owner: owner,
		request:    runnerExecutionIntentPreviewRequest(owner, "conversation-001", "run-001"),
		intentPath: intentPath, sessionSource: sessionSource, resourceSource: resourceSource,
		inventorySource: inventorySource, recorder: recorder,
	}
}

func executionIntentGateRoutes(
	baseRoutes http.Handler,
	sessionSource *fixtureClientInstanceSessionViewSource,
	resourceSource *fixtureClientInstanceResourceViewSource,
	inventorySource *fixtureDeviceInventoryReadV2Source,
	intentPath string,
) http.Handler {
	intentCandidate := newRunnerExecutionIntentPreviewRoutes()
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch {
		case r.URL.EscapedPath() == clientInstanceSessionViewCandidatePath:
			newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{Enabled: true, Source: sessionSource}).ServeHTTP(w, r)
		case r.URL.EscapedPath() == clientInstanceResourceViewCandidatePath:
			newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{Enabled: true, Source: resourceSource}).ServeHTTP(w, r)
		case r.URL.EscapedPath() == deviceInventoryReadCandidateV2Path:
			newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{Enabled: true, Source: inventorySource}).ServeHTTP(w, r)
		case r.URL.EscapedPath() == intentPath:
			intentCandidate.ServeHTTP(w, r)
		default:
			baseRoutes.ServeHTTP(w, r)
		}
	})
}

func runExecutionIntentGateCase(t *testing.T, harness *executionIntentGateHarness, mode string, clientKinds []string, expectPosts int) {
	t.Helper()
	sessionView, resourceView := executionIntentGateViews(t, harness.owner, mode)
	harness.sessionSource.value, harness.resourceSource.value = sessionView, resourceView
	harness.inventorySource.value = preflightProjectionInventory(t, harness.owner, resourceView)
	start := len(harness.recorder.snapshot())
	runForgeConsoleClientInstanceRunnerExecutionIntentGateE2E(t, harness.server.URL, harness.token, harness.owner, harness.request, clientKinds, expectPosts)
	assertExecutionIntentGateRequests(t, harness.recorder.snapshot()[start:], harness.intentPath, expectPosts)
}

func assertExecutionIntentGateProductionClosed(t *testing.T, harness *executionIntentGateHarness) {
	t.Helper()
	body, err := json.Marshal(harness.request)
	if err != nil {
		t.Fatal(err)
	}
	for _, testCase := range []struct {
		method, path, scope, body string
	}{
		{http.MethodGet, clientInstanceSessionViewCandidatePath, clientInstanceSessionViewCandidateScope, ""},
		{http.MethodGet, clientInstanceResourceViewCandidatePath, clientInstanceResourceViewCandidateScope, ""},
		{http.MethodGet, deviceInventoryReadCandidateV2Path, deviceInventoryReadCandidateScope, ""},
		{http.MethodPost, harness.intentPath, runnerExecutionIntentPreviewScope, string(body)},
	} {
		contentType := ""
		if testCase.method == http.MethodPost {
			contentType = "application/json"
		}
		response := requestConversationAPIToken(t, harness.production, harness.token, testCase.method, testCase.path, contentType, "", testCase.body)
		if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
			t.Fatalf("default production %s %s status=%d body=%q", testCase.method, testCase.path, response.Code, response.Body.String())
		}
	}
}

func executionIntentGateViews(
	t *testing.T,
	owner deviceplacement.Owner,
	mode string,
) (deviceplacement.ClientInstanceSessionViewObservation, deviceplacement.ClientInstanceResourceViewObservation) {
	t.Helper()
	sessionInstances := gateDispatchPlanInstances(mode, false)
	resourceInstances := gateDispatchPlanInstances(mode, true)
	session, err := deviceplacement.ObserveClientInstanceSessionView(
		deviceplacement.ClientInstanceSessionViewRequest{Owner: owner, Instances: sessionInstances},
	)
	if err != nil {
		t.Fatalf("observe execution-intent Gate session view: %v", err)
	}
	resource := preflightProjectionResourceView(t, owner, resourceInstances)
	return session, resource
}

func assertExecutionIntentGateRequests(
	t *testing.T,
	requests []recordedConversationRequest,
	intentPath string,
	wantPosts int,
) {
	t.Helper()
	relevant := make([]recordedConversationRequest, 0, len(requests))
	for _, request := range requests {
		if request.path == clientInstanceSessionViewCandidatePath ||
			request.path == clientInstanceResourceViewCandidatePath ||
			request.path == deviceInventoryReadCandidateV2Path ||
			request.path == intentPath {
			relevant = append(relevant, request)
		}
	}
	posts := 0
	for index, request := range relevant {
		if request.method != http.MethodPost || request.path != intentPath {
			continue
		}
		posts++
		if index < 4 || relevant[index-4].path != clientInstanceSessionViewCandidatePath ||
			relevant[index-3].path != clientInstanceResourceViewCandidatePath ||
			relevant[index-2].path != deviceInventoryReadCandidateV2Path ||
			relevant[index-1].path != clientInstanceResourceViewCandidatePath {
			t.Fatalf("execution-intent POST did not follow session/resource/inventory/resource reads: %#v", relevant)
		}
	}
	if posts != wantPosts {
		t.Fatalf("execution-intent POST count=%d want=%d relevant=%#v all=%#v", posts, wantPosts, relevant, requests)
	}
}

func runForgeConsoleClientInstanceRunnerExecutionIntentGateE2E(
	t *testing.T,
	apiURL, token string,
	owner deviceplacement.Owner,
	request deviceplacement.RunnerExecutionIntentRequest,
	clientKinds []string,
	expectPosts int,
) {
	t.Helper()
	consoleRoot := executionIntentGateConsoleRoot(t)
	flutterExecutable := executionIntentGateFlutter(t)
	inputPath, flutterHome := writeExecutionIntentGateInput(t, apiURL, token, owner, request, clientKinds, expectPosts)
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub", "--dart-define=FORGE_CONVERSATIONS_API_ORIGIN="+apiURL,
		"test/forge_client_instance_execution_intent_gate_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_CLIENT_INSTANCE_RUNNER_EXECUTION_INTENT_GATE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout, command.Stderr = &stdoutBuffer, &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter execution-intent Gate E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("execution-intent Gate Flutter output exceeded the size limit")
	}
}

func executionIntentGateConsoleRoot(t *testing.T) string {
	t.Helper()
	root := os.Getenv("SNAPLINK_CONSOLE_ROOT")
	if root == "" {
		workingDirectory, err := os.Getwd()
		if err != nil {
			t.Fatal(err)
		}
		repoRoot := filepath.Clean(filepath.Join(workingDirectory, "..", "..", ".."))
		root = filepath.Join(filepath.Dir(repoRoot), "workspace", "demo", "snaplink-console")
	}
	if _, err := os.Stat(filepath.Join(root, "pubspec.yaml")); err != nil {
		t.Fatalf("SNAPLINK_CONSOLE_ROOT must name the Flutter Console repository: %v", err)
	}
	return root
}

func executionIntentGateFlutter(t *testing.T) string {
	t.Helper()
	binary := os.Getenv("FLUTTER_BIN")
	if binary == "" {
		binary = "flutter"
	}
	executable, err := exec.LookPath(binary)
	if err != nil {
		t.Fatalf("Flutter is required for execution-intent Gate E2E: %v", err)
	}
	return executable
}

func writeExecutionIntentGateInput(
	t *testing.T,
	apiURL, token string,
	owner deviceplacement.Owner,
	request deviceplacement.RunnerExecutionIntentRequest,
	clientKinds []string,
	expectPosts int,
) (string, string) {
	t.Helper()
	inputJSON, err := json.Marshal(struct {
		APIURL      string                                       `json:"api_url"`
		AccessToken string                                       `json:"access_token"`
		Owner       deviceplacement.Owner                        `json:"owner"`
		ClientKinds []string                                     `json:"client_kinds"`
		ExpectCard  bool                                         `json:"expect_execution_intent_card"`
		Request     deviceplacement.RunnerExecutionIntentRequest `json:"request"`
	}{
		APIURL: apiURL, AccessToken: token, Owner: owner, ClientKinds: clientKinds,
		ExpectCard: expectPosts > 0, Request: request,
	})
	if err != nil {
		t.Fatalf("encode execution-intent Gate input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "client-instance-execution-intent-gate-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private execution-intent Gate input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	return inputPath, flutterHome
}
