package appserver

// This opt-in acceptance test crosses the real Snaplink JWT boundary for the
// display-only Runner dispatch-plan candidate. Each Flutter invocation is an
// independent Web/App/Mobile client: it reads the shared resource projection
// first, confirms its own session declaration, and then posts one bound
// comparison request. No target is selected, reserved, authorized, or
// dispatched.

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

	"forgeos/forge-core/internal/deviceplacement"
)

func TestSnaplinkAuthenticatedClientInstanceDispatchPlanProjectionE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_DISPATCH_PLAN_PROJECTION_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_DISPATCH_PLAN_PROJECTION_E2E=1 for client-instance dispatch-plan projection E2E")
	}
	identity, authenticator := newMultiPrincipalConversationTestIdentity(t)
	owner := deviceplacement.Owner{
		Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate",
	}
	resourceView := clientInstanceDispatchPlanResourceView(owner)
	resourceSource := &fixtureClientInstanceResourceViewSource{value: resourceView}
	sessionView, err := deviceplacement.ObserveClientInstanceSessionView(
		deviceplacement.ClientInstanceSessionViewRequest{
			Owner:     owner,
			Instances: append([]deviceplacement.ClientInstanceSessionViewInstance(nil), resourceView.Instances...),
		},
	)
	if err != nil {
		t.Fatalf("observe client-instance dispatch session view: %v", err)
	}
	sessionSource := &fixtureClientInstanceSessionViewSource{value: sessionView}
	sessionCandidate := newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
		Enabled: true,
		Source:  sessionSource,
	})
	resourceCandidate := newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
		Enabled: true,
		Source:  resourceSource,
	})
	dispatchCandidate := newRunnerDispatchPlanPreviewRoutes()
	handler := authenticator.Handler(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method == http.MethodGet && r.URL.Path == clientInstanceSessionViewCandidatePath {
			sessionCandidate.ServeHTTP(w, r)
			return
		}
		if r.Method == http.MethodGet && r.URL.Path == clientInstanceResourceViewCandidatePath {
			resourceCandidate.ServeHTTP(w, r)
			return
		}
		dispatchCandidate.ServeHTTP(w, r)
	}))
	server := httptest.NewServer(handler)
	t.Cleanup(server.Close)

	preflight := runAttemptLeaseDispatchPreflightRequest(owner, "conversation-001", "run-001")
	for _, client := range []struct {
		name       string
		instanceID string
	}{
		{name: "web", instanceID: "client-web-001"},
		{name: "app", instanceID: "client-app-001"},
		{name: "mobile", instanceID: "client-mobile-001"},
	} {
		client := client
		t.Run(client.name, func(t *testing.T) {
			token := tokenForIndependentClient(
				identity,
				"forge:conversations:read forge:devices:read",
				"dispatch-plan-projection-"+client.name,
			)
			runForgeConsoleClientInstanceDispatchPlanProjectionE2E(
				t, server.URL, token, owner, client.instanceID, preflight,
			)
		})
	}

	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	resourceRequest := httptest.NewRequest(http.MethodGet, clientInstanceResourceViewCandidatePath, nil)
	resourceRequest.Header.Set("Authorization", "Bearer "+tokenForIndependentClient(
		identity, "forge:devices:read", "dispatch-plan-production-resource",
	))
	resourceResponse := httptest.NewRecorder()
	production.ServeHTTP(resourceResponse, resourceRequest)
	if resourceResponse.Code != http.StatusNotFound {
		t.Fatalf("production client-instance resource route status=%d body=%q", resourceResponse.Code, resourceResponse.Body.String())
	}

	body, err := json.Marshal(preflight.DispatchPlan)
	if err != nil {
		t.Fatal(err)
	}
	dispatchPath := "/api/v1/conversations/conversation-001/runs/run-001/runner-dispatch-plan-preview"
	dispatchRequest := httptest.NewRequest(http.MethodPost, dispatchPath, strings.NewReader(string(body)))
	dispatchRequest.Header.Set("Authorization", "Bearer "+tokenForIndependentClient(
		identity, "forge:conversations:read", "dispatch-plan-production-preview",
	))
	dispatchRequest.Header.Set("Content-Type", "application/json")
	dispatchResponse := httptest.NewRecorder()
	production.ServeHTTP(dispatchResponse, dispatchRequest)
	if dispatchResponse.Code != http.StatusNotFound {
		t.Fatalf("production dispatch-plan route status=%d body=%q", dispatchResponse.Code, dispatchResponse.Body.String())
	}
}

func clientInstanceDispatchPlanResourceView(owner deviceplacement.Owner) deviceplacement.ClientInstanceResourceViewObservation {
	instances := []deviceplacement.ClientInstanceSessionViewInstance{
		{InstanceID: "client-app-001", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{"conversation-001"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-cli-001", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{"conversation-001"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-mobile-001", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{"conversation-001"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-tui-001", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{"conversation-001"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-web-001", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{"conversation-001"}, ObservedAtMS: 200_500, Status: "active"},
	}
	devices := []deviceplacement.ClientInstanceResourceViewDevice{
		{
			DeviceID: "runner-1", RunnerInstanceID: "runner-instance-1", Owner: owner,
			Revision: 1, Generation: 1, HeartbeatSequence: 1, ObservedAtMS: 200_500,
			ApprovalState: "approved", CordonState: "clear", ReservationState: "none", Liveness: "online",
			OS: "linux", Architecture: "amd64", CPUCores: 8, AvailableCPUCores: 7,
			MemoryBytes: 16 << 30, AvailableMemoryBytes: 12 << 30, StorageBytes: 100 << 30, AvailableStorageBytes: 80 << 30,
		},
		{
			DeviceID: "runner-2", RunnerInstanceID: "runner-instance-2", Owner: owner,
			Revision: 1, Generation: 1, HeartbeatSequence: 1, ObservedAtMS: 200_500,
			ApprovalState: "approved", CordonState: "clear", ReservationState: "none", Liveness: "online",
			OS: "linux", Architecture: "amd64", CPUCores: 8, AvailableCPUCores: 8,
			MemoryBytes: 16 << 30, AvailableMemoryBytes: 16 << 30, StorageBytes: 100 << 30, AvailableStorageBytes: 100 << 30,
		},
	}
	view, err := deviceplacement.ObserveClientInstanceResourceView(deviceplacement.ClientInstanceResourceViewRequest{
		Owner: owner, Instances: instances, Devices: devices,
	})
	if err != nil {
		panic(err)
	}
	return view
}

func runForgeConsoleClientInstanceDispatchPlanProjectionE2E(
	t *testing.T,
	apiURL, token string,
	owner deviceplacement.Owner,
	instanceID string,
	request deviceplacement.RunAttemptLeaseDispatchPreflightRequest,
) {
	t.Helper()
	consoleRoot := os.Getenv("SNAPLINK_CONSOLE_ROOT")
	if consoleRoot == "" {
		workingDirectory, err := os.Getwd()
		if err != nil {
			t.Fatalf("resolve Console repository: %v", err)
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
		t.Fatalf("Flutter is required for dispatch-plan projection E2E: %v", err)
	}
	inputJSON, err := json.Marshal(struct {
		APIURL         string                                                  `json:"api_url"`
		AccessToken    string                                                  `json:"access_token"`
		Owner          deviceplacement.Owner                                   `json:"owner"`
		InstanceID     string                                                  `json:"instance_id"`
		ConversationID string                                                  `json:"conversation_id"`
		RunID          string                                                  `json:"run_id"`
		Request        deviceplacement.RunAttemptLeaseDispatchPreflightRequest `json:"request"`
	}{
		APIURL: apiURL, AccessToken: token, Owner: owner, InstanceID: instanceID,
		ConversationID: request.ConversationID, RunID: request.RunID, Request: request,
	})
	if err != nil {
		t.Fatalf("encode Flutter dispatch-plan projection input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "client-instance-dispatch-plan-projection-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter dispatch-plan projection input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub", "test/forge_client_instance_dispatch_plan_preview_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_CLIENT_INSTANCE_DISPATCH_PLAN_PROJECTION_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter dispatch-plan projection E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("Flutter dispatch-plan projection output exceeded the size limit")
	}
}
