//go:build linux && !android

package appserver

// This opt-in integration test crosses the real Snaplink JWT boundary with
// the Runtime TUI.  The TUI must consume the owner-bound inventory and
// client-instance projections before posting the planning-only Runner
// dispatch-plan candidate.  The candidate mux is test-only; production
// Runner execution remains closed.

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
)

func TestSnaplinkAuthenticatedClientInstanceRunnerDispatchPlanTUIConvergenceE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_RUNNER_DISPATCH_PLAN_TUI_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_RUNNER_DISPATCH_PLAN_TUI_E2E=1 for Runtime TUI dispatch-plan convergence E2E")
	}
	configuredExecutable := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredExecutable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for Runtime TUI dispatch-plan convergence E2E")
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
				{InstanceID: "client-tui-001", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{"conversation-001"}, ObservedAtMS: 200_500, Status: "active"},
				{InstanceID: "client-web-001", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{"conversation-002"}, ObservedAtMS: 200_500, Status: "idle"},
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

	// Align the read-only v2 inventory with the resource view so the TUI's
	// local inventory/resource guard can prove one snapshot before the POST.
	inventory := fixtureDeviceInventoryReadV2Value(owner)
	inventory.Devices[0].Device.ReservationState = "none"
	inventory.Devices[0].Device.SnapshotObservedAtMS = resourceView.Devices[0].ObservedAtMS
	inventory.Devices[0].Device.AvailableCPUCores = resourceView.Devices[0].AvailableCPUCores
	inventory.Devices[0].Device.AvailableMemoryBytes = resourceView.Devices[0].AvailableMemoryBytes
	inventory.Devices[0].Device.AvailableStorage = resourceView.Devices[0].AvailableStorageBytes
	inventory.Devices[0].Device.GPUs = make([]deviceplacement.GPUDeclarationV2, 0)
	inventorySource := &fixtureDeviceInventoryReadV2Source{value: inventory}
	sessionSource := &fixtureClientInstanceSessionViewSource{value: instances}
	resourceSource := &fixtureClientInstanceResourceViewSource{value: resourceView}

	backend := &fakeConversationBackend{
		listPage: model.OwnedConversationPage{
			Conversations: []model.OwnedConversationEntry{{
				Conversation: model.Conversation{
					ID: "conversation-001", Scope: model.ConversationScope{Kind: "global"},
					Title: "TUI dispatch-plan fixture", CreatedAtMS: 1, UpdatedAtMS: 1,
				},
				AggregateVersion: 1,
			}},
		},
	}
	request := runAttemptLeaseDispatchPreflightRequest(owner, "conversation-001", "run-001")
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	inputPath := filepath.Join(t.TempDir(), "runner-dispatch-plan-tui-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatal(err)
	}

	dispatchPath := "/api/v1/conversations/conversation-001/runs/run-001/runner-dispatch-plan-preview"
	base := newAuthenticatedSessionRoutesWithObservationCandidatesBackend(backend, nil)
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
	testRoutes.Handle(dispatchPath, newRunnerDispatchPlanPreviewRoutes())
	var requestOrder []string
	recordedRoutes := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		requestOrder = append(requestOrder, r.Method+" "+r.URL.EscapedPath())
		switch r.URL.EscapedPath() {
		case clientInstanceSessionViewCandidatePath, clientInstanceResourceViewCandidatePath,
			deviceInventoryReadCandidateV2Path, dispatchPath:
			testRoutes.ServeHTTP(w, r)
		default:
			base.ServeHTTP(w, r)
		}
	})
	server := httptest.NewServer(authenticator.Handler(recordedRoutes))
	t.Cleanup(server.Close)

	visibleStart := len(requestOrder)
	visibleOutput := runForgeRuntimeClientInstanceDispatchPlanTUI(
		t, executable, server.URL, token, inputPath, "client-tui-001",
	)
	for _, want := range []string{
		"offline Runner dispatch-plan preview [forge.runner-dispatch-plan-preview/v1]",
		"conversation=conversation-001 run=run-001",
		"dispatch_performed=false",
	} {
		if !strings.Contains(visibleOutput, want) {
			t.Fatalf("visible TUI output omitted %q: %q", want, visibleOutput)
		}
	}
	visibleRequests := requestOrder[visibleStart:]
	wantVisible := []string{
		http.MethodGet + " " + conversationCollectionPath,
		http.MethodGet + " " + deviceInventoryReadCandidateV2Path,
		http.MethodGet + " " + clientInstanceSessionViewCandidatePath,
		http.MethodGet + " " + clientInstanceResourceViewCandidatePath,
		http.MethodGet + " " + deviceInventoryReadCandidateV2Path,
		http.MethodGet + " " + clientInstanceResourceViewCandidatePath,
		http.MethodPost + " " + dispatchPath,
	}
	assertRequestSuffix(t, visibleRequests, wantVisible)

	// The Web projection hides the selected Conversation. The real TUI must
	// stop locally after the same authenticated reads and emit no candidate
	// POST. This also exercises the selected-session check after filtering.
	hiddenStart := len(requestOrder)
	hiddenOutput := runForgeRuntimeClientInstanceDispatchPlanTUI(
		t, executable, server.URL, token, inputPath, "client-web-001",
	)
	if !strings.Contains(hiddenOutput, "requires the selected session to match conversation conversation-001") &&
		!strings.Contains(hiddenOutput, "no Runner dispatch-plan request was sent") {
		t.Fatalf("hidden TUI projection did not fail closed: %q", hiddenOutput)
	}
	hiddenRequests := requestOrder[hiddenStart:]
	for _, request := range hiddenRequests {
		if request == http.MethodPost+" "+dispatchPath {
			t.Fatalf("hidden TUI projection escaped before-POST guard: %#v", hiddenRequests)
		}
	}
	if sessionSource.calls != 2 || resourceSource.calls != 3 || inventorySource.calls != 3 {
		t.Fatalf("TUI did not re-read owner projections for visible and hidden runs: session=%d resource=%d inventory=%d", sessionSource.calls, resourceSource.calls, inventorySource.calls)
	}

	// The ordinary production constructor remains closed for this candidate.
	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	productionRequest := httptest.NewRequest(http.MethodPost, dispatchPath, strings.NewReader(string(body)))
	productionRequest.Header.Set("Authorization", "Bearer "+token)
	productionRequest.Header.Set("Content-Type", "application/json")
	productionResponse := httptest.NewRecorder()
	production.ServeHTTP(productionResponse, productionRequest)
	if productionResponse.Code != http.StatusNotFound || productionResponse.Body.String() != string(notFoundBody) {
		t.Fatalf("default production TUI dispatch-plan status=%d body=%q", productionResponse.Code, productionResponse.Body.String())
	}
}

func runForgeRuntimeClientInstanceDispatchPlanTUI(
	t *testing.T, executable, apiURL, accessToken, inputPath, instanceID string,
) string {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("client-instance dispatch-plan TUI E2E requires script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader(
		"inventory read-v2\n" +
			"client-instances show-converged\n" +
			"instance " + instanceID + "\n" +
			"runner-dispatch-plan-remote-preview --input " + inputPath + "\n" +
			"quit\n",
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated client-instance dispatch-plan TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("authenticated client-instance dispatch-plan TUI output exceeded the size limit")
	}
	return stdoutBuffer.String()
}

func assertRequestSuffix(t *testing.T, got, want []string) {
	t.Helper()
	if len(got) != len(want) {
		t.Fatalf("request sequence length=%d want=%d: got=%#v want=%#v", len(got), len(want), got, want)
	}
	for index := range want {
		if got[index] != want[index] {
			t.Fatalf("request[%d]=%q want=%q; full=%#v", index, got[index], want[index], got)
		}
	}
}
