package appserver

// This opt-in acceptance test crosses one real Snaplink JWT through the
// shared Flutter Web/App/Mobile Sessions Gate. Every visible selected client
// must refresh session/resource and inventory/resource projections before one
// metadata-only Runner execution-boundary POST. Hidden membership and a
// selected resource drift remain candidate-free.

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
	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceplacement"
)

type executionBoundaryGateHarness struct {
	server          *httptest.Server
	production      http.Handler
	token           string
	owner           deviceplacement.Owner
	request         deviceplacement.RunnerExecutionBoundaryPreviewRequest
	boundaryPath    string
	sessionSource   *fixtureClientInstanceSessionViewSource
	resourceSource  *fixtureClientInstanceResourceViewSource
	inventorySource *fixtureDeviceInventoryReadV2Source
	recorder        *conversationHTTPRecorder
}

func TestSnaplinkAuthenticatedClientInstanceRunnerExecutionBoundaryGateE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_RUNNER_EXECUTION_BOUNDARY_GATE_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_RUNNER_EXECUTION_BOUNDARY_GATE_E2E=1 for Console execution-boundary Gate E2E")
	}
	harness := newExecutionBoundaryGateHarness(t)
	for _, testCase := range []struct {
		name        string
		mode        string
		clientKinds []string
		expectPosts int
	}{
		{name: "visible-web-app-mobile", mode: "visible", clientKinds: []string{"web", "app", "mobile"}, expectPosts: 3},
		{name: "hidden-web", mode: "hidden", clientKinds: []string{"web"}, expectPosts: 0},
		{name: "resource-drift-mobile", mode: "drift", clientKinds: []string{"mobile"}, expectPosts: 0},
	} {
		t.Run(testCase.name, func(t *testing.T) {
			runExecutionBoundaryGateCase(t, harness, testCase.mode, testCase.clientKinds, testCase.expectPosts)
		})
	}
	assertExecutionBoundaryGateProductionClosed(t, harness)
}

func newExecutionBoundaryGateHarness(t *testing.T) *executionBoundaryGateHarness {
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
	identityOwner := deviceidentity.Owner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	}
	registryPath, entry := admissionCLILease(t, owner)
	transport := runnerTransportAdmissionE2ERequest(t, identityOwner, entry)
	request := deviceplacement.RunnerExecutionBoundaryPreviewRequest{
		Owner: owner, ConversationID: entry.ConversationID, RunID: entry.RunID,
		AttemptID: entry.AttemptID, AttemptState: "accepted", Command: transport.Command,
		Transport: transport.Transport, ExpectedPayloadSHA256: transport.ExpectedPayloadSHA256,
		Controls: deviceplacement.RunnerExecutionBoundaryControls{EffectState: "not_started"},
	}
	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{
		Status: "accepted", AcceptanceID: "console-execution-boundary-gate-p4-001", AcceptedAtUnixMS: 1,
	}
	activation.Evidence.RunnerIsolation = true
	activation.Evidence.LeaseFencing = true
	activation.Evidence.CancellationAndUncertainWork = true
	activation.Evidence.VaultArtifactAuthorization = true
	activation.Evidence.AuditOutbox = true
	authority := devicefabricgate.RunnerAuthorityConfig{
		Enabled: true, AuthorityID: "console-execution-boundary-gate-runner-authority",
		Decision: devicefabricgate.Decision{
			Status: "accepted", AcceptanceID: "console-execution-boundary-gate-authority-001", AcceptedAtUnixMS: 1,
		},
	}
	sessionSource := &fixtureClientInstanceSessionViewSource{}
	resourceSource := &fixtureClientInstanceResourceViewSource{}
	inventorySource := &fixtureDeviceInventoryReadV2Source{}
	backend := preflightProjectionConversationBackend(entry.ConversationID, entry.RunID)
	baseRoutes := newAuthenticatedSessionRoutesWithObservationCandidatesBackend(backend, nil)
	boundaryPath := runnerExecutionBoundaryPathIDsURL(entry.ConversationID, entry.RunID)
	boundary := newRunnerExecutionBoundaryRoutes(&runnerExecutionBoundaryConfig{
		Enabled: true, RegistryPath: registryPath,
		Now: func(context.Context) (int64, error) {
			return time.Now().UnixMilli(), nil
		},
		Activation: activation, Authority: authority, Backend: backend,
	})
	recorder := &conversationHTTPRecorder{}
	testRoutes := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
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
		case boundaryPath:
			capture := httptest.NewRecorder()
			boundary.ServeHTTP(capture, r)
			t.Logf("execution-boundary candidate status=%d body=%s", capture.Code, capture.Body.String())
			for key, values := range capture.Header() {
				for _, value := range values {
					w.Header().Add(key, value)
				}
			}
			w.WriteHeader(capture.Code)
			_, _ = w.Write(capture.Body.Bytes())
		default:
			baseRoutes.ServeHTTP(w, r)
		}
	})
	server := httptest.NewServer(authenticator.Handler(recorder.wrap(testRoutes)))
	t.Cleanup(server.Close)
	return &executionBoundaryGateHarness{
		server: server, production: authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil)),
		token: token, owner: owner, request: request, boundaryPath: boundaryPath,
		sessionSource: sessionSource, resourceSource: resourceSource,
		inventorySource: inventorySource, recorder: recorder,
	}
}

func runExecutionBoundaryGateCase(
	t *testing.T,
	harness *executionBoundaryGateHarness,
	mode string,
	clientKinds []string,
	expectPosts int,
) {
	t.Helper()
	session, resource := executionBoundaryGateViews(t, harness.owner, mode)
	harness.sessionSource.value = session
	harness.resourceSource.value = resource
	harness.inventorySource.value = preflightProjectionInventory(t, harness.owner, resource)
	start := len(harness.recorder.snapshot())
	runForgeConsoleClientInstanceRunnerExecutionBoundaryGateE2E(
		t, harness.server.URL, harness.token, harness.owner, harness.request, clientKinds, expectPosts > 0, harness.recorder,
	)
	assertExecutionBoundaryGateRequests(t, harness.recorder.snapshot()[start:], harness.boundaryPath, expectPosts)
}

func executionBoundaryGateViews(
	t *testing.T,
	owner deviceplacement.Owner,
	mode string,
) (deviceplacement.ClientInstanceSessionViewObservation, deviceplacement.ClientInstanceResourceViewObservation) {
	t.Helper()
	sessionInstances := gateDispatchPlanInstances("visible", false)
	resourceInstances := gateDispatchPlanInstances("visible", true)
	if mode == "hidden" {
		for index := range sessionInstances {
			if sessionInstances[index].InstanceID == "client-web-001" {
				sessionInstances[index].SessionIDs = nil
			}
		}
		for index := range resourceInstances {
			if resourceInstances[index].InstanceID == "client-web-001" {
				resourceInstances[index].SessionIDs = nil
			}
		}
	}
	if mode == "drift" {
		for index := range resourceInstances {
			if resourceInstances[index].InstanceID == "client-mobile-001" {
				resourceInstances[index].SessionIDs = nil
			}
		}
	}
	session, err := deviceplacement.ObserveClientInstanceSessionView(
		deviceplacement.ClientInstanceSessionViewRequest{Owner: owner, Instances: sessionInstances},
	)
	if err != nil {
		t.Fatalf("observe execution-boundary Gate session view: %v", err)
	}
	resource := preflightProjectionResourceView(t, owner, resourceInstances)
	return session, resource
}

func assertExecutionBoundaryGateRequests(
	t *testing.T,
	requests []recordedConversationRequest,
	boundaryPath string,
	wantPosts int,
) {
	t.Helper()
	relevant := make([]recordedConversationRequest, 0, len(requests))
	for _, request := range requests {
		if request.path == clientInstanceSessionViewCandidatePath ||
			request.path == clientInstanceResourceViewCandidatePath ||
			request.path == deviceInventoryReadCandidateV2Path ||
			request.path == boundaryPath {
			relevant = append(relevant, request)
		}
	}
	posts := 0
	for index, request := range relevant {
		if request.method != http.MethodPost || request.path != boundaryPath {
			continue
		}
		posts++
		if index < 4 ||
			relevant[index-4].method != http.MethodGet ||
			relevant[index-4].path != clientInstanceSessionViewCandidatePath ||
			relevant[index-3].method != http.MethodGet ||
			relevant[index-3].path != clientInstanceResourceViewCandidatePath ||
			relevant[index-2].method != http.MethodGet ||
			relevant[index-2].path != deviceInventoryReadCandidateV2Path ||
			relevant[index-1].method != http.MethodGet ||
			relevant[index-1].path != clientInstanceResourceViewCandidatePath {
			t.Fatalf("execution-boundary POST did not follow session/resource/inventory/resource reads: %#v", relevant)
		}
	}
	if posts != wantPosts {
		t.Fatalf("execution-boundary POST count=%d want=%d relevant=%#v all=%#v", posts, wantPosts, relevant, requests)
	}
}

func assertExecutionBoundaryGateProductionClosed(t *testing.T, harness *executionBoundaryGateHarness) {
	t.Helper()
	body, err := json.Marshal(harness.request)
	if err != nil {
		t.Fatal(err)
	}
	for _, request := range []struct {
		method, path, scope, body string
	}{
		{http.MethodGet, clientInstanceSessionViewCandidatePath, clientInstanceSessionViewCandidateScope, ""},
		{http.MethodGet, clientInstanceResourceViewCandidatePath, clientInstanceResourceViewCandidateScope, ""},
		{http.MethodGet, deviceInventoryReadCandidateV2Path, deviceInventoryReadCandidateScope, ""},
		{http.MethodPost, harness.boundaryPath, runnerTransportAdmissionScope, string(body)},
	} {
		contentType := ""
		if request.method == http.MethodPost {
			contentType = "application/json"
		}
		response := requestConversationAPIToken(t, harness.production, harness.token, request.method, request.path, contentType, "", request.body)
		if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
			t.Fatalf("default production %s %s status=%d body=%q", request.method, request.path, response.Code, response.Body.String())
		}
	}
}

func runForgeConsoleClientInstanceRunnerExecutionBoundaryGateE2E(
	t *testing.T,
	apiURL, token string,
	owner deviceplacement.Owner,
	request deviceplacement.RunnerExecutionBoundaryPreviewRequest,
	clientKinds []string,
	expectCard bool,
	recorder *conversationHTTPRecorder,
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
		t.Fatalf("Flutter is required for execution-boundary Gate E2E: %v", err)
	}
	inputJSON, err := json.Marshal(struct {
		APIURL      string                                                `json:"api_url"`
		AccessToken string                                                `json:"access_token"`
		Owner       deviceplacement.Owner                                 `json:"owner"`
		ClientKinds []string                                              `json:"client_kinds"`
		ExpectCard  bool                                                  `json:"expect_execution_boundary_card"`
		Request     deviceplacement.RunnerExecutionBoundaryPreviewRequest `json:"request"`
	}{
		APIURL: apiURL, AccessToken: token, Owner: owner,
		ClientKinds: clientKinds, ExpectCard: expectCard, Request: request,
	})
	if err != nil {
		t.Fatalf("encode execution-boundary Gate input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "client-instance-runner-execution-boundary-gate-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private execution-boundary Gate input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub", "--dart-define=FORGE_CONVERSATIONS_API_ORIGIN="+apiURL,
		"test/forge_client_instance_runner_execution_boundary_gate_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_CLIENT_INSTANCE_RUNNER_EXECUTION_BOUNDARY_GATE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout, command.Stderr = &stdoutBuffer, &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter execution-boundary Gate E2E failed: stdout=%q stderr=%q err=%v requests=%#v", stdoutBuffer.String(), stderrBuffer.String(), err, recorder.snapshot())
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("execution-boundary Gate Flutter output exceeded the size limit")
	}
}
