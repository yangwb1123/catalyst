package appserver

// This opt-in integration test crosses the real Snaplink JWT boundary with
// the Runtime CLI instance filter.  The CLI joins the owner-bound session and
// resource observations before posting the planning-only Runner dispatch-plan
// candidate.  The candidate mux is test-only; production Runner routes remain
// closed.

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
)

func TestSnaplinkAuthenticatedClientInstanceRunnerDispatchPlanConvergenceE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_RUNNER_DISPATCH_PLAN_CONVERGENCE_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_RUNNER_DISPATCH_PLAN_CONVERGENCE_E2E=1 for Runtime CLI dispatch-plan convergence E2E")
	}
	configuredExecutable := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredExecutable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for Runtime CLI dispatch-plan convergence E2E")
	}
	executable, err := exec.LookPath(configuredExecutable)
	if err != nil {
		t.Fatalf("FORGE_RUNTIME_BIN must name a built forge-runtime executable available to the test: %v", err)
	}
	executable, err = filepath.Abs(executable)
	if err != nil {
		t.Fatalf("resolve forge-runtime executable path: %v", err)
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
	instances, err := deviceplacement.ObserveClientInstanceSessionView(
		deviceplacement.ClientInstanceSessionViewRequest{
			Owner: owner,
			Instances: []deviceplacement.ClientInstanceSessionViewInstance{
				{InstanceID: "client-cli-001", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{"conversation-001", "conversation-002"}, ObservedAtMS: 200_500, Status: "active"},
				{InstanceID: "client-tui-001", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{"conversation-002"}, ObservedAtMS: 200_500, Status: "active"},
				{InstanceID: "client-web-001", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{"conversation-002"}, ObservedAtMS: 200_500, Status: "idle"},
				{InstanceID: "client-app-001", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{"conversation-001"}, ObservedAtMS: 200_500, Status: "active"},
				{InstanceID: "client-mobile-001", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{"conversation-002"}, ObservedAtMS: 200_500, Status: "idle"},
			},
		},
	)
	if err != nil {
		t.Fatalf("observe client-instance session view: %v", err)
	}
	resourceView := fixtureClientInstanceResourceView(owner)
	resourceView.Instances = append([]deviceplacement.ClientInstanceSessionViewInstance(nil), instances.Instances...)
	if err := resourceView.Validate(); err != nil {
		t.Fatalf("validate paired resource view: %v", err)
	}
	// The instance-scoped Runtime CLI refreshes the inventory/resource pair
	// immediately before the candidate POST. Align this lossless inventory
	// image with the resource projection so it can cross that guard.
	inventory := fixtureDeviceInventoryReadV2Value(owner)
	inventory.Devices[0].Device.ReservationState = "none"
	inventory.Devices[0].Device.SnapshotObservedAtMS = resourceView.Devices[0].ObservedAtMS
	inventory.Devices[0].Device.AvailableCPUCores = resourceView.Devices[0].AvailableCPUCores
	inventory.Devices[0].Device.AvailableMemoryBytes = resourceView.Devices[0].AvailableMemoryBytes
	inventory.Devices[0].Device.AvailableStorage = resourceView.Devices[0].AvailableStorageBytes
	inventory.Devices[0].Device.GPUs = []deviceplacement.GPUDeclarationV2{}
	sessionSource := &fixtureClientInstanceSessionViewSource{value: instances}
	resourceSource := &fixtureClientInstanceResourceViewSource{value: resourceView}
	inventorySource := &fixtureDeviceInventoryReadV2Source{value: inventory}
	recorder := &conversationHTTPRecorder{}

	testRoutes := http.NewServeMux()
	testRoutes.Handle(clientInstanceSessionViewCandidatePath,
		newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
			Enabled: true, Source: sessionSource,
		}))
	testRoutes.Handle(clientInstanceResourceViewCandidatePath,
		newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
			Enabled: true, Source: resourceSource,
		}))
	testRoutes.Handle(deviceInventoryReadCandidateV2Path,
		newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
			Enabled: true, Source: inventorySource,
		}))
	dispatchPath := "/api/v1/conversations/conversation-001/runs/run-001/runner-dispatch-plan-preview"
	testRoutes.Handle(dispatchPath, newRunnerDispatchPlanPreviewRoutes())
	server := httptest.NewServer(authenticator.Handler(recorder.wrap(testRoutes)))
	t.Cleanup(server.Close)

	preflight := runAttemptLeaseDispatchPreflightRequest(owner, "conversation-001", "run-001")
	body, err := json.Marshal(preflight)
	if err != nil {
		t.Fatal(err)
	}
	inputPath := filepath.Join(t.TempDir(), "runner-dispatch-plan-instance-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatal(err)
	}

	visibleStart := len(recorder.snapshot())
	output, stderr, err := runForgeRuntimeCLI(
		t, executable, server.URL, token, t.TempDir(), "--json", "remote",
		"runner-dispatch-plan-preview", "--input", inputPath, "--instance", "client-cli-001",
	)
	if err != nil {
		t.Fatalf("authenticated instance-filtered Runner dispatch-plan CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var observation deviceplacement.RunnerDispatchPlanPreviewObservation
	if err := json.Unmarshal([]byte(output), &observation); err != nil {
		t.Fatalf("decode instance-filtered Runner dispatch-plan CLI: %v stdout=%q", err, output)
	}
	want, err := deviceplacement.ObserveRunnerDispatchPlanPreview(preflight.DispatchPlan)
	if err != nil {
		t.Fatalf("observe dispatch-plan fixture: %v", err)
	}
	if !reflect.DeepEqual(observation, want) || observation.Owner != owner || observation.SelectedTargetID != nil ||
		observation.Authority != (deviceplacement.RunnerDispatchPlanPreviewAuthority{}) || !observation.PreviewOnly {
		t.Fatalf("instance-filtered dispatch-plan observation=%#v want=%#v", observation, want)
	}
	visibleRequests := recorder.snapshot()[visibleStart:]
	if len(visibleRequests) != 5 ||
		visibleRequests[0].path != clientInstanceSessionViewCandidatePath ||
		visibleRequests[1].path != clientInstanceResourceViewCandidatePath ||
		visibleRequests[2].path != deviceInventoryReadCandidateV2Path ||
		visibleRequests[3].path != clientInstanceResourceViewCandidatePath ||
		visibleRequests[4].path != dispatchPath ||
		visibleRequests[0].method != http.MethodGet || visibleRequests[1].method != http.MethodGet ||
		visibleRequests[2].method != http.MethodGet || visibleRequests[3].method != http.MethodGet ||
		visibleRequests[4].method != http.MethodPost {
		t.Fatalf("instance-filtered dispatch-plan did not read converged pair before POST: %#v", visibleRequests)
	}
	if sessionSource.calls != 1 || resourceSource.calls != 2 || inventorySource.calls != 1 ||
		sessionSource.owner.Issuer != owner.Issuer || sessionSource.owner.Subject != owner.Subject || sessionSource.owner.TenantID != owner.TenantID ||
		resourceSource.owner.Issuer != owner.Issuer || resourceSource.owner.Subject != owner.Subject || resourceSource.owner.TenantID != owner.TenantID {
		t.Fatalf("instance-filtered dispatch-plan source binding session=%#v resource=%#v inventory=%#v", sessionSource, resourceSource, inventorySource)
	}

	// A Conversation hidden from the selected client instance is rejected
	// after the same pair reads and before the candidate POST.
	hiddenStart := len(recorder.snapshot())
	_, hiddenStderr, hiddenErr := runForgeRuntimeCLI(
		t, executable, server.URL, token, t.TempDir(), "--json", "remote",
		"runner-dispatch-plan-preview", "--input", inputPath, "--instance", "client-web-001",
	)
	if hiddenErr == nil || !strings.Contains(hiddenStderr, "no Runner dispatch-plan request was sent") {
		t.Fatalf("hidden instance-filtered dispatch-plan was not rejected: stderr=%q err=%v", hiddenStderr, hiddenErr)
	}
	hiddenRequests := recorder.snapshot()[hiddenStart:]
	if len(hiddenRequests) != 2 || hiddenRequests[0].path != clientInstanceSessionViewCandidatePath ||
		hiddenRequests[1].path != clientInstanceResourceViewCandidatePath {
		t.Fatalf("hidden instance-filtered dispatch-plan escaped pair guard: %#v", hiddenRequests)
	}

	if sessionSource.calls != 2 || resourceSource.calls != 3 || inventorySource.calls != 1 {
		t.Fatalf("dispatch-plan source calls session=%d resource=%d inventory=%d", sessionSource.calls, resourceSource.calls, inventorySource.calls)
	}
	if strings.Contains(output, "fencing_token") || strings.Contains(output, "argv") || strings.Contains(output, "workspace_ref") {
		t.Fatalf("dispatch-plan preview leaked execution material: %q", output)
	}

	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	productionRequest := httptest.NewRequest(http.MethodPost, dispatchPath, strings.NewReader(string(body)))
	productionRequest.Header.Set("Authorization", "Bearer "+token)
	productionRequest.Header.Set("Content-Type", "application/json")
	productionResponse := httptest.NewRecorder()
	production.ServeHTTP(productionResponse, productionRequest)
	if productionResponse.Code != http.StatusNotFound || productionResponse.Body.String() != string(notFoundBody) {
		t.Fatalf("default production dispatch-plan status=%d body=%q", productionResponse.Code, productionResponse.Body.String())
	}
}
