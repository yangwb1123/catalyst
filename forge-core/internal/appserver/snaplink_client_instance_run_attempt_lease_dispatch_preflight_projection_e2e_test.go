package appserver

// This opt-in acceptance test crosses a real Snaplink JWT through the shared
// Flutter Web/App/Mobile Sessions Gate. The Gate must compose the selected
// client-instance session/resource pair and the owner-bound inventory/resource
// image before posting the pure Run/Attempt/lease preflight candidate. Hidden
// Runs stop before POST, and the ordinary production constructor stays closed.

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

func TestSnaplinkAuthenticatedClientInstanceRunAttemptLeaseDispatchPreflightProjectionE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_PROJECTION_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_PROJECTION_E2E=1 for Console Run/Attempt/lease preflight projection E2E")
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
	request := canonicalRunAttemptLeaseDispatchPreflightRequest(t, owner)
	conversationID, runID := request.ConversationID, request.RunID
	visibleSession := preflightProjectionSessionView(t, owner, conversationID)
	visibleResource := preflightProjectionResourceView(t, owner, visibleSession.Instances)
	visibleInventory := preflightProjectionInventory(t, owner, visibleResource)
	sessionSource := &fixtureClientInstanceSessionViewSource{value: visibleSession}
	resourceSource := &fixtureClientInstanceResourceViewSource{value: visibleResource}
	inventorySource := &fixtureDeviceInventoryReadV2Source{value: visibleInventory}
	backend := preflightProjectionConversationBackend(conversationID, runID)

	baseRoutes := newAuthenticatedSessionRoutesWithObservationCandidatesBackend(backend, nil)
	conversationRoutes := newConversationRoutesWithBackend(backend)
	preflightPath := "/api/v1/conversations/" + conversationID + "/runs/" + runID + "/attempt-lease-dispatch-preflight/preview"
	preflightCandidate := newRunAttemptLeaseDispatchPreflightRoutes()
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
		case r.URL.EscapedPath() == deviceInventoryReadCandidateV2Path:
			newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
				Enabled: true, Source: inventorySource,
			}).ServeHTTP(w, r)
		case r.URL.EscapedPath() == preflightPath:
			preflightCandidate.ServeHTTP(w, r)
		case r.URL.EscapedPath() == conversationCollectionPath || strings.HasPrefix(r.URL.EscapedPath(), conversationCollectionPath+"/"):
			conversationRoutes.ServeHTTP(w, r)
		default:
			baseRoutes.ServeHTTP(w, r)
		}
	})
	recorder := &conversationHTTPRecorder{}
	server := httptest.NewServer(authenticator.Handler(recorder.wrap(testRoutes)))
	t.Cleanup(server.Close)

	visibleStart := len(recorder.snapshot())
	runForgeConsoleClientInstanceRunAttemptLeaseDispatchPreflightProjectionE2E(
		t, server.URL, token, owner, request,
		[]string{"web", "app", "mobile"}, true,
	)
	visibleRequests := recorder.snapshot()[visibleStart:]
	assertPreflightProjectionRequests(t, visibleRequests, preflightPath, 3)
	if sessionSource.calls == 0 || resourceSource.calls == 0 || inventorySource.calls == 0 {
		t.Fatalf("visible projection did not read session/resource/inventory: session=%d resource=%d inventory=%d", sessionSource.calls, resourceSource.calls, inventorySource.calls)
	}

	// Replace the owner-bound image after the visible clients have completed.
	// The Web instance now hides the requested Conversation. The Gate must still
	// re-read the pair, but it must not reach inventory authority or preflight.
	hiddenSession := preflightProjectionSessionView(t, owner, "conversation-hidden")
	hiddenResource := preflightProjectionResourceView(t, owner, hiddenSession.Instances)
	if err := hiddenResource.Validate(); err != nil {
		t.Fatal(err)
	}
	sessionSource.value = hiddenSession
	resourceSource.value = hiddenResource
	hiddenStart := len(recorder.snapshot())
	runForgeConsoleClientInstanceRunAttemptLeaseDispatchPreflightProjectionE2E(
		t, server.URL, token, owner, request, []string{"web"}, false,
	)
	hiddenRequests := recorder.snapshot()[hiddenStart:]
	for _, request := range hiddenRequests {
		if request.method == http.MethodPost && request.path == preflightPath {
			t.Fatalf("hidden Run escaped the preflight before-POST guard: %#v", hiddenRequests)
		}
	}
	if sessionSource.calls < 2 || resourceSource.calls < 2 {
		t.Fatalf("hidden projection did not re-read the pair: session=%d resource=%d", sessionSource.calls, resourceSource.calls)
	}

	// The same authenticated production constructor remains default-off for the
	// preflight, pair, resource, and inventory candidate paths.
	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	productionCases := []struct {
		method string
		path   string
		scope  string
		body   string
	}{
		{method: http.MethodGet, path: clientInstanceSessionViewCandidatePath, scope: clientInstanceSessionViewCandidateScope},
		{method: http.MethodGet, path: clientInstanceResourceViewCandidatePath, scope: clientInstanceResourceViewCandidateScope},
		{method: http.MethodGet, path: deviceInventoryReadCandidateV2Path, scope: deviceInventoryReadCandidateScope},
		{method: http.MethodPost, path: preflightPath, scope: runAttemptLeaseDispatchPreflightScope, body: mustMarshalPreflightRequest(t, request)},
	}
	for _, testCase := range productionCases {
		contentType := ""
		if testCase.method == http.MethodPost {
			contentType = "application/json"
		}
		response := requestConversationAPIToken(t, production, token, testCase.method, testCase.path, contentType, "", testCase.body)
		if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
			t.Fatalf("default production %s %s status=%d body=%q", testCase.method, testCase.path, response.Code, response.Body.String())
		}
	}
}

func canonicalRunAttemptLeaseDispatchPreflightRequest(t *testing.T, owner deviceplacement.Owner) deviceplacement.RunAttemptLeaseDispatchPreflightRequest {
	t.Helper()
	path := os.Getenv("FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_REQUEST_FIXTURE")
	if path == "" {
		request := runAttemptLeaseDispatchPreflightRequest(owner, "conversation-001", "run-001")
		return request
	}
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read canonical Run/Attempt/lease preflight request fixture: %v", err)
	}
	var request deviceplacement.RunAttemptLeaseDispatchPreflightRequest
	if err := json.Unmarshal(body, &request); err != nil {
		t.Fatalf("decode canonical Run/Attempt/lease preflight request fixture: %v", err)
	}
	request.Owner = owner
	request.DispatchPlan.Placement.Owner = owner
	for index := range request.DispatchPlan.Placement.Devices {
		request.DispatchPlan.Placement.Devices[index].Owner = owner
	}
	request.DispatchPlan.Intent.Owner = owner
	if _, err := deviceplacement.ObserveRunAttemptLeaseDispatchPreflight(request); err != nil {
		t.Fatalf("canonical Run/Attempt/lease preflight request after owner binding: %v", err)
	}
	return request
}

func preflightProjectionSessionView(t *testing.T, owner deviceplacement.Owner, conversationID string) deviceplacement.ClientInstanceSessionViewObservation {
	t.Helper()
	view, err := deviceplacement.ObserveClientInstanceSessionView(deviceplacement.ClientInstanceSessionViewRequest{
		Owner: owner,
		Instances: []deviceplacement.ClientInstanceSessionViewInstance{
			{InstanceID: "client-app-001", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{conversationID}, ObservedAtMS: 200_500, Status: "active"},
			{InstanceID: "client-mobile-001", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{conversationID}, ObservedAtMS: 200_500, Status: "idle"},
			{InstanceID: "client-web-001", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{conversationID}, ObservedAtMS: 200_500, Status: "idle"},
		},
	})
	if err != nil {
		t.Fatalf("observe preflight projection session view: %v", err)
	}
	return view
}

func preflightProjectionResourceView(t *testing.T, owner deviceplacement.Owner, instances []deviceplacement.ClientInstanceSessionViewInstance) deviceplacement.ClientInstanceResourceViewObservation {
	t.Helper()
	view := fixtureClientInstanceResourceView(owner)
	view.Instances = append([]deviceplacement.ClientInstanceSessionViewInstance(nil), instances...)
	if err := view.Validate(); err != nil {
		t.Fatalf("validate preflight projection resource view: %v", err)
	}
	return view
}

func preflightProjectionInventory(t *testing.T, owner deviceplacement.Owner, resource deviceplacement.ClientInstanceResourceViewObservation) deviceplacement.SessionDeviceObservationInventoryV2 {
	t.Helper()
	inventory := fixtureDeviceInventoryReadV2Value(owner)
	if len(inventory.Devices) != 1 || len(resource.Devices) != 1 {
		t.Fatalf("preflight projection requires one fixture device: inventory=%d resource=%d", len(inventory.Devices), len(resource.Devices))
	}
	candidate := &inventory.Devices[0]
	device := &candidate.Device
	resourceDevice := resource.Devices[0]
	candidate.InstanceID = resourceDevice.RunnerInstanceID
	candidate.Revision = resourceDevice.Revision
	candidate.Generation = resourceDevice.Generation
	candidate.HeartbeatSequence = resourceDevice.HeartbeatSequence
	device.DeviceID = resourceDevice.DeviceID
	device.Owner = owner
	device.ApprovalState = resourceDevice.ApprovalState
	device.CordonState = resourceDevice.CordonState
	device.ReservationState = resourceDevice.ReservationState
	device.Liveness = resourceDevice.Liveness
	device.SnapshotObservedAtMS = resourceDevice.ObservedAtMS
	device.OS = resourceDevice.OS
	device.Architecture = resourceDevice.Architecture
	device.AvailableCPUCores = resourceDevice.AvailableCPUCores
	device.AvailableMemoryBytes = resourceDevice.AvailableMemoryBytes
	device.AvailableStorage = resourceDevice.AvailableStorageBytes
	device.GPUs = []deviceplacement.GPUDeclarationV2{}
	if err := deviceplacement.ValidateSessionDeviceObservationInventoryV2(inventory); err != nil {
		t.Fatalf("validate preflight projection inventory: %v", err)
	}
	return inventory
}

func preflightProjectionConversationBackend(conversationID, runID string) *fakeConversationBackend {
	conversation := model.Conversation{
		ID: conversationID, Scope: model.ConversationScope{Kind: "global"},
		Title: "Run Attempt lease preflight", CreatedAtMS: 1, UpdatedAtMS: 2,
	}
	return &fakeConversationBackend{
		listPage: model.OwnedConversationPage{
			Conversations: []model.OwnedConversationEntry{{Conversation: conversation, AggregateVersion: 1}},
		},
		detail: model.OwnedConversationEntry{Conversation: conversation, AggregateVersion: 1},
		promptPage: model.ConversationPromptPage{
			ConversationID: conversationID,
			Prompts:        []model.ConversationPrompt{},
		},
		runPage: runmodel.OwnedRunPage{
			ConversationID: conversationID,
			Runs:           []runmodel.OwnedRunSummary{{RunID: runID, PromptID: "prompt-001", CreatedAtMS: 10, LatestSequence: 1, Status: "nonterminal"}},
		},
		timelinePage: runmodel.OwnedRunTimelinePage{
			ConversationID: conversationID, RunID: runID, AfterSequence: 0, ScannedThroughSequence: 0,
			Events: []runmodel.OwnedRunEventSummary{},
		},
	}
}

func assertPreflightProjectionRequests(t *testing.T, requests []recordedConversationRequest, preflightPath string, wantPosts int) {
	t.Helper()
	posts := 0
	for _, request := range requests {
		if request.method == http.MethodPost && request.path == preflightPath {
			posts++
		}
	}
	if posts != wantPosts {
		t.Fatalf("preflight POST count=%d want=%d requests=%#v", posts, wantPosts, requests)
	}
	if wantPosts == 0 {
		return
	}
	for index, request := range requests {
		if request.method != http.MethodPost || request.path != preflightPath {
			continue
		}
		var sessionIndex, inventoryIndex, resourceIndex int = -1, -1, -1
		for prior := 0; prior < index; prior++ {
			switch {
			case requests[prior].method == http.MethodGet && requests[prior].path == clientInstanceSessionViewCandidatePath:
				sessionIndex = prior
			case requests[prior].method == http.MethodGet && requests[prior].path == deviceInventoryReadCandidateV2Path:
				inventoryIndex = prior
			case requests[prior].method == http.MethodGet && requests[prior].path == clientInstanceResourceViewCandidatePath:
				resourceIndex = prior
			}
		}
		if sessionIndex < 0 || resourceIndex < 0 || inventoryIndex < 0 ||
			sessionIndex > index || resourceIndex > index || inventoryIndex > index {
			t.Fatalf("preflight POST did not follow pair/inventory/resource reads: %#v", requests)
		}
	}
}

func runForgeConsoleClientInstanceRunAttemptLeaseDispatchPreflightProjectionE2E(
	t *testing.T,
	apiURL, token string,
	owner deviceplacement.Owner,
	request deviceplacement.RunAttemptLeaseDispatchPreflightRequest,
	clientKinds []string,
	expectPreflight bool,
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
		t.Fatalf("Flutter is required for preflight projection E2E: %v", err)
	}
	inputJSON, err := json.Marshal(struct {
		APIURL          string                                                  `json:"api_url"`
		AccessToken     string                                                  `json:"access_token"`
		Owner           deviceplacement.Owner                                   `json:"owner"`
		Request         deviceplacement.RunAttemptLeaseDispatchPreflightRequest `json:"request"`
		ClientKinds     []string                                                `json:"client_kinds"`
		ExpectPreflight bool                                                    `json:"expect_preflight"`
	}{
		APIURL: apiURL, AccessToken: token, Owner: owner, Request: request,
		ClientKinds: clientKinds, ExpectPreflight: expectPreflight,
	})
	if err != nil {
		t.Fatalf("encode preflight projection E2E input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "client-instance-preflight-projection-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private preflight projection E2E input: %v", err)
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
		"test/forge_client_instance_run_attempt_lease_dispatch_preflight_projection_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_CLIENT_INSTANCE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_PROJECTION_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter preflight projection E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("Flutter preflight projection E2E output exceeded the size limit")
	}
}

func mustMarshalPreflightRequest(t *testing.T, request deviceplacement.RunAttemptLeaseDispatchPreflightRequest) string {
	t.Helper()
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	return string(body)
}
