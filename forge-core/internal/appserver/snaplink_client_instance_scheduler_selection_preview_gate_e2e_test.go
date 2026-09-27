package appserver

// This opt-in acceptance test crosses one real Snaplink JWT through the
// shared Flutter Web/App/Mobile Sessions Gate. Each visible client refreshes
// its selected client-instance pair and owner-bound inventory/resource image
// before the planning-only scheduler-preview candidate. Hidden or drifted
// instances remain candidate-free; production routes stay default-off.

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

func TestSnaplinkAuthenticatedClientInstanceSchedulerSelectionPreviewGateE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_SCHEDULER_PREVIEW_GATE_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_SCHEDULER_PREVIEW_GATE_E2E=1 for Console client-instance scheduler preview Gate E2E")
	}
	authenticator, token, owner := schedulerPreviewGateAuth(t)
	request := schedulerPreviewGateRequest(owner)
	harness := newSchedulerPreviewGateHarness(t, authenticator, owner, request)
	for _, testCase := range []struct {
		name, mode  string
		clientKinds []string
		wantPosts   int
	}{
		{name: "visible-web-app-mobile", mode: "visible", clientKinds: []string{"web", "app", "mobile"}, wantPosts: 3},
		{name: "hidden-web", mode: "hidden", clientKinds: []string{"web"}, wantPosts: 0},
		{name: "resource-drift-web", mode: "drift", clientKinds: []string{"web"}, wantPosts: 0},
	} {
		t.Run(testCase.name, func(t *testing.T) {
			session, resource := schedulerPreviewGateViews(t, owner, testCase.mode)
			harness.sessionSource.value = session
			harness.resourceSource.value = resource
			harness.inventorySource.value = schedulerPreviewGateInventory(t, owner, resource)
			start := len(harness.recorder.snapshot())
			runForgeConsoleClientInstanceSchedulerPreviewGateE2E(
				t, harness.server.URL, token, owner, request, testCase.clientKinds,
				testCase.wantPosts,
			)
			assertSchedulerPreviewGateRequests(
				t, harness.recorder.snapshot()[start:], testCase.wantPosts,
			)
		})
	}
	assertSchedulerPreviewGateProductionClosed(t, harness, token, request)
}

type schedulerPreviewGateHarness struct {
	server          *httptest.Server
	production      http.Handler
	owner           deviceplacement.Owner
	request         schedulerSelectionPreviewRequest
	sessionSource   *fixtureClientInstanceSessionViewSource
	resourceSource  *fixtureClientInstanceResourceViewSource
	inventorySource *fixtureDeviceInventoryReadV2Source
	recorder        *conversationHTTPRecorder
}

func schedulerPreviewGateAuth(t *testing.T) (*authn.Authenticator, string, deviceplacement.Owner) {
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
	return authenticator, token, deviceplacement.Owner{
		Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant,
	}
}

func newSchedulerPreviewGateHarness(
	t *testing.T,
	authenticator *authn.Authenticator,
	owner deviceplacement.Owner,
	request schedulerSelectionPreviewRequest,
) *schedulerPreviewGateHarness {
	t.Helper()
	sessionSource := &fixtureClientInstanceSessionViewSource{}
	resourceSource := &fixtureClientInstanceResourceViewSource{}
	inventorySource := &fixtureDeviceInventoryReadV2Source{}
	policySource := &fixtureSchedulerSelectionPolicySource{value: schedulerPreviewGatePolicy(owner)}
	backend := schedulerPreviewGateBackend()
	baseRoutes := newAuthenticatedSessionRoutesWithObservationCandidatesBackend(backend, nil)
	schedulerCandidate := newSchedulerSelectionPreviewRoutes(&schedulerSelectionPreviewConfig{
		Enabled: true, Source: inventorySource, PolicySource: policySource,
		Backend: backend, Now: func(context.Context) (int64, error) { return 300_000, nil },
	})
	routes := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.EscapedPath() {
		case clientInstanceSessionViewCandidatePath:
			newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
				Enabled: true, Source: sessionSource,
			}).ServeHTTP(w, r)
		case clientInstanceResourceViewCandidatePath:
			newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
				Enabled: true, Source: resourceSource,
			}).ServeHTTP(w, r)
		case deviceInventoryReadCandidateV2Path:
			newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
				Enabled: true, Source: inventorySource,
			}).ServeHTTP(w, r)
		case schedulerSelectionPreviewPath:
			schedulerCandidate.ServeHTTP(w, r)
		default:
			baseRoutes.ServeHTTP(w, r)
		}
	})
	recorder := &conversationHTTPRecorder{}
	server := httptest.NewServer(authenticator.Handler(recorder.wrap(routes)))
	t.Cleanup(server.Close)
	return &schedulerPreviewGateHarness{
		server: server, production: authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil)),
		owner: owner, request: request, sessionSource: sessionSource,
		resourceSource: resourceSource, inventorySource: inventorySource, recorder: recorder,
	}
}

func schedulerPreviewGateRequest(owner deviceplacement.Owner) schedulerSelectionPreviewRequest {
	return schedulerSelectionPreviewRequest{
		ConversationID: "conversation-a", RunID: "run-a", AttemptID: "attempt-a",
		Requirements: deviceplacement.Requirements{
			OS: "linux", Architecture: "amd64", MinCPUCores: 1,
			MinMemoryBytes: 1, MinStorageBytes: 1, Runtime: "go",
			GPU: deviceplacement.GPURequirement{}, DataResidencyZones: []string{"us-west"},
			MinimumTrustZone: "untrusted", SandboxFloor: "process", ConcurrencySlots: 1,
		},
	}
}

func schedulerPreviewGateViews(
	t *testing.T, owner deviceplacement.Owner, mode string,
) (deviceplacement.ClientInstanceSessionViewObservation, deviceplacement.ClientInstanceResourceViewObservation) {
	t.Helper()
	instances := schedulerPreviewGateInstances(mode, false)
	session, err := deviceplacement.ObserveClientInstanceSessionView(
		deviceplacement.ClientInstanceSessionViewRequest{Owner: owner, Instances: instances},
	)
	if err != nil {
		t.Fatalf("observe scheduler preview Gate session view: %v", err)
	}
	resource := fixtureClientInstanceResourceView(owner)
	resource.Instances = schedulerPreviewGateInstances(mode, true)
	for index := range resource.Devices {
		resource.Devices[index].ObservedAtMS = 250_000
	}
	if err := resource.Validate(); err != nil {
		t.Fatalf("validate scheduler preview Gate resource view: %v", err)
	}
	return session, resource
}

func schedulerPreviewGateInstances(mode string, resource bool) []deviceplacement.ClientInstanceSessionViewInstance {
	instances := gateDispatchPlanInstances("visible", resource)
	for index := range instances {
		instances[index].SessionIDs = []string{"conversation-a"}
	}
	if mode == "hidden" || (mode == "drift" && resource) {
		for index := range instances {
			if instances[index].InstanceID == "client-web-001" {
				instances[index].SessionIDs = nil
			}
		}
	}
	return instances
}

func schedulerPreviewGateInventory(
	t *testing.T,
	owner deviceplacement.Owner,
	resource deviceplacement.ClientInstanceResourceViewObservation,
) deviceplacement.SessionDeviceObservationInventoryV2 {
	t.Helper()
	inventory := fixtureDeviceInventoryReadV2Value(owner)
	inventory.EvaluatedAtMS = 300_000
	if len(inventory.Devices) != 1 || len(resource.Devices) != 1 {
		t.Fatalf("scheduler preview Gate requires one fixture device: inventory=%d resource=%d", len(inventory.Devices), len(resource.Devices))
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
	device.ReservationState = "none"
	device.Liveness = resourceDevice.Liveness
	device.SnapshotObservedAtMS = resourceDevice.ObservedAtMS
	device.LeaseExpiresAtMS = 600_000
	device.OS = resourceDevice.OS
	device.Architecture = resourceDevice.Architecture
	device.AvailableCPUCores = resourceDevice.AvailableCPUCores
	device.AvailableMemoryBytes = resourceDevice.AvailableMemoryBytes
	device.AvailableStorage = resourceDevice.AvailableStorageBytes
	device.GPUs = []deviceplacement.GPUDeclarationV2{}
	if err := deviceplacement.ValidateSessionDeviceObservationInventoryV2(inventory); err != nil {
		t.Fatalf("validate scheduler preview Gate inventory: %v", err)
	}
	return inventory
}

func schedulerPreviewGatePolicy(owner deviceplacement.Owner) deviceplacement.PlacementPolicyRegistry {
	return deviceplacement.PlacementPolicyRegistry{
		SchemaVersion:  deviceplacement.PlacementPolicyRegistrySchemaVersion,
		EvaluationMode: deviceplacement.PlacementPolicyRegistryEvaluationMode,
		Owner:          owner,
		Policies: []deviceplacement.PlacementPolicy{{
			DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1,
			HeartbeatSequence: 1, DataResidencyZones: []string{"us-west"},
			TrustZone: "untrusted", SandboxLevels: []string{"process"},
			ConcurrencyLimit: 1, ActiveConcurrency: 0,
		}},
	}
}

func schedulerPreviewGateBackend() *fakeConversationBackend {
	backend := gateDispatchPlanConversationBackend()
	backend.listPage.Conversations[0].Conversation.ID = "conversation-a"
	backend.listPage.Conversations[0].Conversation.Title = "Shared scheduler preview"
	backend.detail.Conversation.ID = "conversation-a"
	backend.promptPage.ConversationID = "conversation-a"
	backend.runPage.ConversationID = "conversation-a"
	backend.runPage.Runs[0].RunID = "run-a"
	backend.timelinePage.ConversationID = "conversation-a"
	backend.timelinePage.RunID = "run-a"
	return backend
}

func assertSchedulerPreviewGateRequests(
	t *testing.T, requests []recordedConversationRequest, wantPosts int,
) {
	t.Helper()
	posts := 0
	for index, request := range requests {
		if request.method != http.MethodPost || request.path != schedulerSelectionPreviewPath {
			continue
		}
		posts++
		if index < 4 ||
			requests[index-4].path != clientInstanceSessionViewCandidatePath ||
			requests[index-3].path != clientInstanceResourceViewCandidatePath ||
			requests[index-2].path != deviceInventoryReadCandidateV2Path ||
			requests[index-1].path != clientInstanceResourceViewCandidatePath {
			t.Fatalf("scheduler preview POST did not follow session/resource/inventory/resource reads: %#v", requests)
		}
	}
	if posts != wantPosts {
		t.Fatalf("scheduler preview POST count=%d want=%d requests=%#v", posts, wantPosts, requests)
	}
}

func assertSchedulerPreviewGateProductionClosed(
	t *testing.T, harness *schedulerPreviewGateHarness, token string,
	request schedulerSelectionPreviewRequest,
) {
	t.Helper()
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	for _, testCase := range []struct {
		method, path, scope, body string
	}{
		{http.MethodGet, clientInstanceSessionViewCandidatePath, clientInstanceSessionViewCandidateScope, ""},
		{http.MethodGet, clientInstanceResourceViewCandidatePath, clientInstanceResourceViewCandidateScope, ""},
		{http.MethodGet, deviceInventoryReadCandidateV2Path, deviceInventoryReadCandidateScope, ""},
		{http.MethodPost, schedulerSelectionPreviewPath, schedulerSelectionPreviewScope, string(body)},
	} {
		contentType := ""
		if testCase.method == http.MethodPost {
			contentType = "application/json"
		}
		response := requestConversationAPIToken(
			t, harness.production, token, testCase.method, testCase.path,
			contentType, "", testCase.body,
		)
		if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
			t.Fatalf("default production %s %s status=%d body=%q", testCase.method, testCase.path, response.Code, response.Body.String())
		}
	}
}

func runForgeConsoleClientInstanceSchedulerPreviewGateE2E(
	t *testing.T, apiURL, token string, owner deviceplacement.Owner,
	request schedulerSelectionPreviewRequest, clientKinds []string, wantPosts int,
) {
	t.Helper()
	consoleRoot := schedulerPreviewGateConsoleRoot(t)
	flutterExecutable := schedulerPreviewGateFlutterExecutable(t)
	temporaryDirectory := t.TempDir()
	inputPath := writeSchedulerPreviewGateInput(
		t, temporaryDirectory, apiURL, token, owner, request, clientKinds, wantPosts,
	)
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	runFlutterSchedulerPreviewGate(
		t, consoleRoot, flutterExecutable, apiURL, inputPath, flutterHome,
	)
}

func schedulerPreviewGateConsoleRoot(t *testing.T) string {
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
	return consoleRoot
}

func schedulerPreviewGateFlutterExecutable(t *testing.T) string {
	t.Helper()
	flutterBinary := os.Getenv("FLUTTER_BIN")
	if flutterBinary == "" {
		flutterBinary = "flutter"
	}
	flutterExecutable, err := exec.LookPath(flutterBinary)
	if err != nil {
		t.Fatalf("Flutter is required for client-instance scheduler preview Gate E2E: %v", err)
	}
	return flutterExecutable
}

func writeSchedulerPreviewGateInput(
	t *testing.T, temporaryDirectory, apiURL, token string, owner deviceplacement.Owner,
	request schedulerSelectionPreviewRequest, clientKinds []string, wantPosts int,
) string {
	t.Helper()
	inputJSON, err := json.Marshal(struct {
		APIURL                  string                           `json:"api_url"`
		AccessToken             string                           `json:"access_token"`
		Owner                   deviceplacement.Owner            `json:"owner"`
		ClientKinds             []string                         `json:"client_kinds"`
		ExpectedCard            bool                             `json:"expect_scheduler_preview_card"`
		ExpectedDeviceID        string                           `json:"expected_device_id"`
		ExpectedInstanceID      string                           `json:"expected_instance_id"`
		ExpectedSelectionReason string                           `json:"expected_selection_reason"`
		Request                 schedulerSelectionPreviewRequest `json:"request"`
	}{
		APIURL: apiURL, AccessToken: token, Owner: owner, ClientKinds: clientKinds,
		ExpectedCard: wantPosts > 0, ExpectedDeviceID: "device-a", ExpectedInstanceID: "runner-a",
		ExpectedSelectionReason: "first_sorted_eligible_candidate", Request: request,
	})
	if err != nil {
		t.Fatalf("encode Flutter client-instance scheduler preview Gate input: %v", err)
	}
	inputPath := filepath.Join(temporaryDirectory, "client-instance-scheduler-preview-gate-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter scheduler preview Gate input: %v", err)
	}
	return inputPath
}

func runFlutterSchedulerPreviewGate(
	t *testing.T, consoleRoot, flutterExecutable, apiURL, inputPath, flutterHome string,
) {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub", "--dart-define=FORGE_CONVERSATIONS_API_ORIGIN="+apiURL,
		"test/forge_client_instance_scheduler_selection_preview_gate_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_CLIENT_INSTANCE_SCHEDULER_PREVIEW_GATE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout, command.Stderr = &stdoutBuffer, &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter client-instance scheduler preview Gate E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("client-instance scheduler preview Gate Flutter output exceeded the size limit")
	}
}
