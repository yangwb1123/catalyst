//go:build linux && !android

package appserver

// This opt-in integration test joins the authenticated client-instance
// session/resource projections with the planning-only scheduler preview. The
// candidate handlers are mounted on a test mux; the production assembler,
// lease, dispatch, and Runner execution boundaries remain untouched. Optional
// Runtime CLI/TUI sub-checks use the same real Snaplink JWT. Each client that
// explicitly opens the owner-bound inventory-v2/resource pair refreshes it
// before the candidate POST.

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

func TestSnaplinkAuthenticatedClientInstanceSchedulerPreviewConvergenceE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_SCHEDULER_PREVIEW_CONVERGENCE_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_SCHEDULER_PREVIEW_CONVERGENCE_E2E=1 for paired scheduler preview E2E")
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
	instances := []deviceplacement.ClientInstanceSessionViewInstance{
		{InstanceID: "client-cli", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{"conversation-a", "conversation-b"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-tui", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{"conversation-b"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-web", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{"conversation-a"}, ObservedAtMS: 200_500, Status: "idle"},
		{InstanceID: "client-app", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{"conversation-a"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-mobile", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{"conversation-b"}, ObservedAtMS: 200_500, Status: "idle"},
	}
	sessionView, err := deviceplacement.ObserveClientInstanceSessionView(
		deviceplacement.ClientInstanceSessionViewRequest{Owner: owner, Instances: instances},
	)
	if err != nil {
		t.Fatalf("observe paired session view: %v", err)
	}
	resourceView := fixtureClientInstanceResourceView(owner)
	resourceView.Instances = append([]deviceplacement.ClientInstanceSessionViewInstance(nil), sessionView.Instances...)
	if err := resourceView.Validate(); err != nil {
		t.Fatalf("validate paired resource view: %v", err)
	}

	// The scheduler fixture is the same owner-scoped device/Runner pair shown
	// by resourceView. Its policy image supplies the planning attributes that
	// the resource projection intentionally does not expose.
	inventory := fixtureDeviceInventoryReadV2Value(owner)
	inventory.Devices[0].Device.ReservationState = "none"
	inventory.Devices[0].Device.SnapshotObservedAtMS = 250_000
	inventory.Devices[0].Device.LeaseExpiresAtMS = 600_000
	inventorySource := &fixtureDeviceInventoryReadV2Source{value: inventory}
	policySource := &fixtureSchedulerSelectionPolicySource{value: deviceplacement.PlacementPolicyRegistry{
		SchemaVersion:  deviceplacement.PlacementPolicyRegistrySchemaVersion,
		EvaluationMode: deviceplacement.PlacementPolicyRegistryEvaluationMode,
		Owner:          owner,
		Policies: []deviceplacement.PlacementPolicy{{
			DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1,
			HeartbeatSequence: 1, DataResidencyZones: []string{"us-west"},
			TrustZone: "untrusted", SandboxLevels: []string{"process"},
			ConcurrencyLimit: 1, ActiveConcurrency: 0,
		}},
	}}
	backend := &fakeConversationBackend{
		listPage: model.OwnedConversationPage{
			Conversations: []model.OwnedConversationEntry{{
				Conversation: model.Conversation{
					ID: "conversation-a", Scope: model.ConversationScope{Kind: "global"},
					Title: "Scheduler preview fixture", CreatedAtMS: 1, UpdatedAtMS: 1,
				},
				AggregateVersion: 1,
			}},
		},
		runPage: runmodel.OwnedRunPage{
			ConversationID: "conversation-a",
			Runs: []runmodel.OwnedRunSummary{{
				RunID: "run-a", PromptID: "prompt-a", CreatedAtMS: 1,
				LatestSequence: 1, Status: "completed",
			}},
		},
	}
	sessionSource := &fixtureClientInstanceSessionViewSource{value: sessionView}
	resourceSource := &fixtureClientInstanceResourceViewSource{value: resourceView}
	// The Runtime CLI's explicit --instance path performs a second strict
	// inventory/resource convergence read before its planning-only POST. Keep
	// that display image aligned with resourceView while the scheduler route
	// retains the separate fresh, unreserved image above.
	projectionInventory := fixtureDeviceInventoryReadV2Value(owner)
	projectionInventory.Devices[0].Device.ReservationState = "none"
	projectionInventory.Devices[0].Device.SnapshotObservedAtMS = 100_000
	projectionInventory.Devices[0].Device.LeaseExpiresAtMS = 200_000
	projectionInventory.Devices[0].Device.GPUs = []deviceplacement.GPUDeclarationV2{}
	projectionInventorySource := &fixtureDeviceInventoryReadV2Source{value: projectionInventory}

	testRoutes := http.NewServeMux()
	testRoutes.Handle(deviceInventoryReadCandidateV2Path,
		newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
			Enabled: true, Source: projectionInventorySource,
		}))
	testRoutes.Handle(clientInstanceSessionViewCandidatePath,
		newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
			Enabled: true, Source: sessionSource,
		}))
	testRoutes.Handle(clientInstanceResourceViewCandidatePath,
		newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
			Enabled: true, Source: resourceSource,
		}))
	testRoutes.Handle(schedulerSelectionPreviewPath,
		newSchedulerSelectionPreviewRoutes(&schedulerSelectionPreviewConfig{
			Enabled: true, Source: inventorySource, PolicySource: policySource,
			Backend: backend, Now: func(context.Context) (int64, error) { return 300_000, nil },
		}))
	baseRoutes := newAuthenticatedSessionRoutesWithObservationCandidatesBackend(backend, nil)
	var requestOrder []string
	recordedRoutes := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		requestOrder = append(requestOrder, r.Method+" "+r.URL.EscapedPath())
		switch r.URL.EscapedPath() {
		case clientInstanceSessionViewCandidatePath, clientInstanceResourceViewCandidatePath,
			deviceInventoryReadCandidateV2Path, schedulerSelectionPreviewPath:
			testRoutes.ServeHTTP(w, r)
		default:
			baseRoutes.ServeHTTP(w, r)
		}
	})
	server := httptest.NewServer(authenticator.Handler(recordedRoutes))
	t.Cleanup(server.Close)

	client := server.Client()
	sessionResponse := snaplinkConversationRequest(t, client, server.URL, token,
		http.MethodGet, clientInstanceSessionViewCandidatePath, "", "")
	if sessionResponse.StatusCode != http.StatusOK {
		t.Fatalf("paired session view status=%d body=%q", sessionResponse.StatusCode, readConversationClientBody(t, sessionResponse))
	}
	var gotSession deviceplacement.ClientInstanceSessionViewObservation
	sessionBody := readConversationClientBody(t, sessionResponse)
	if err := json.Unmarshal([]byte(sessionBody), &gotSession); err != nil {
		t.Fatalf("decode paired session view: %v body=%q", err, sessionBody)
	}
	if err := gotSession.Validate(); err != nil {
		t.Fatalf("validate paired session view: %v", err)
	}

	resourceResponse := snaplinkConversationRequest(t, client, server.URL, token,
		http.MethodGet, clientInstanceResourceViewCandidatePath, "", "")
	if resourceResponse.StatusCode != http.StatusOK {
		t.Fatalf("paired resource view status=%d body=%q", resourceResponse.StatusCode, readConversationClientBody(t, resourceResponse))
	}
	var gotResource deviceplacement.ClientInstanceResourceViewObservation
	resourceBody := readConversationClientBody(t, resourceResponse)
	if err := json.Unmarshal([]byte(resourceBody), &gotResource); err != nil {
		t.Fatalf("decode paired resource view: %v body=%q", err, resourceBody)
	}
	if err := gotResource.Validate(); err != nil {
		t.Fatalf("validate paired resource view: %v", err)
	}

	requestBody, err := json.Marshal(schedulerSelectionPreviewRequest{
		ConversationID: "conversation-a", RunID: "run-a", AttemptID: "attempt-a",
		Requirements: deviceplacement.Requirements{
			OS: "linux", Architecture: "amd64", MinCPUCores: 1,
			MinMemoryBytes: 1, MinStorageBytes: 1, Runtime: "go",
			GPU: deviceplacement.GPURequirement{}, DataResidencyZones: []string{"us-west"},
			MinimumTrustZone: "untrusted", SandboxFloor: "process", ConcurrencySlots: 1,
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	previewResponse := snaplinkConversationRequest(t, client, server.URL, token,
		http.MethodPost, schedulerSelectionPreviewPath, "", string(requestBody))
	if previewResponse.StatusCode != http.StatusOK {
		t.Fatalf("paired scheduler preview status=%d body=%q", previewResponse.StatusCode, readConversationClientBody(t, previewResponse))
	}
	var preview deviceplacement.SchedulerSelectionPreviewObservation
	previewBody := readConversationClientBody(t, previewResponse)
	if err := json.Unmarshal([]byte(previewBody), &preview); err != nil {
		t.Fatalf("decode paired scheduler preview: %v body=%q", err, previewBody)
	}
	if err := preview.Validate(); err != nil {
		t.Fatalf("validate paired scheduler preview: %v", err)
	}
	if gotSession.Owner != owner || gotResource.Owner != owner || preview.Owner != owner ||
		sessionSource.calls != 1 || resourceSource.calls != 1 || inventorySource.calls != 1 || projectionInventorySource.calls != 0 || policySource.calls != 1 ||
		sessionSource.owner != (model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) ||
		resourceSource.owner != (model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) ||
		inventorySource.owner != (model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) ||
		policySource.owner != (model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) ||
		backend.runOwner != (model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) ||
		len(gotSession.Instances) != 5 || len(gotResource.Instances) != 5 ||
		string(marshalConvergenceJSON(t, gotSession.Instances)) != string(marshalConvergenceJSON(t, gotResource.Instances)) {
		t.Fatalf("owner/pair convergence session=%#v resource=%#v preview=%#v sources=%#v/%#v/%#v/%#v/%#v backend=%#v",
			gotSession, gotResource, preview, sessionSource, resourceSource, projectionInventorySource, inventorySource, policySource, backend)
	}
	if len(gotResource.Devices) != 1 || gotResource.Devices[0].DeviceID != "device-a" ||
		gotResource.Devices[0].RunnerInstanceID != "runner-a" || !gotResource.ReadOnly {
		t.Fatalf("paired resource devices=%#v", gotResource.Devices)
	}
	if !preview.SelectionAvailable || preview.SelectedDeviceID == nil || *preview.SelectedDeviceID != gotResource.Devices[0].DeviceID ||
		preview.SelectedInstanceID == nil || *preview.SelectedInstanceID != gotResource.Devices[0].RunnerInstanceID ||
		preview.SelectionReason != "first_sorted_eligible_candidate" ||
		preview.Authority != (deviceplacement.SchedulerSelectionPreviewAuthority{}) || !preview.PreviewOnly {
		t.Fatalf("paired scheduler preview=%#v", preview)
	}

	// The direct HTTP proof above exercises the pair and planning endpoint. The
	// separately opt-in Runtime CLI/TUI pass proves that the same real JWT can
	// refresh the pair and inventory/resource image, select one instance locally,
	// and reach the planning-only preview. The hidden instance must stop after
	// exactly its two pair reads;
	// no scheduler POST or Runner authority is enabled by this test.
	runRuntimeCLI := os.Getenv("FORGE_CLIENT_INSTANCE_SCHEDULER_PREVIEW_CLI_E2E") == "1"
	runRuntimeTUI := os.Getenv("FORGE_CLIENT_INSTANCE_SCHEDULER_PREVIEW_TUI_E2E") == "1"
	if runRuntimeCLI || runRuntimeTUI {
		configuredExecutable := os.Getenv("FORGE_RUNTIME_BIN")
		if configuredExecutable == "" {
			t.Fatal("set FORGE_RUNTIME_BIN for Runtime scheduler preview E2E")
		}
		executable, err := exec.LookPath(configuredExecutable)
		if err != nil {
			t.Fatalf("FORGE_RUNTIME_BIN must name a built forge-runtime executable available to the test: %v", err)
		}
		executable, err = filepath.Abs(executable)
		if err != nil {
			t.Fatalf("resolve forge-runtime executable path: %v", err)
		}
		inputPath := filepath.Join(t.TempDir(), "scheduler-selection-preview-request.json")
		if err := os.WriteFile(inputPath, requestBody, 0o600); err != nil {
			t.Fatalf("write scheduler preview input for Runtime clients: %v", err)
		}

		if runRuntimeCLI {
			visibleStart := len(requestOrder)
			visibleOutput, visibleStderr, visibleErr := runForgeRuntimeCLI(
				t, executable, server.URL, token, t.TempDir(), "--json", "remote", "placement",
				"scheduler-preview", "--input", inputPath, "--instance", "client-web",
			)
			if visibleErr != nil {
				t.Fatalf("authenticated client-instance scheduler preview Runtime CLI failed: stderr=%q stdout=%q err=%v",
					visibleStderr, visibleOutput, visibleErr)
			}
			var cliPreview deviceplacement.SchedulerSelectionPreviewObservation
			if err := json.Unmarshal([]byte(visibleOutput), &cliPreview); err != nil {
				t.Fatalf("decode client-instance scheduler preview Runtime CLI: %v stdout=%q", err, visibleOutput)
			}
			if err := cliPreview.Validate(); err != nil || cliPreview.Owner != owner ||
				cliPreview.ConversationID != "conversation-a" || cliPreview.RunID != "run-a" ||
				cliPreview.AttemptID != "attempt-a" || !cliPreview.SelectionAvailable ||
				cliPreview.SelectedDeviceID == nil || *cliPreview.SelectedDeviceID != "device-a" ||
				cliPreview.SelectedInstanceID == nil || *cliPreview.SelectedInstanceID != "runner-a" ||
				!cliPreview.PreviewOnly || cliPreview.Authority != (deviceplacement.SchedulerSelectionPreviewAuthority{}) {
				t.Fatalf("client-instance scheduler preview Runtime CLI=%#v err=%v", cliPreview, err)
			}
			assertSchedulerPreviewCLIRequestOrder(t, requestOrder[visibleStart:], true)

			hiddenStart := len(requestOrder)
			_, hiddenStderr, hiddenErr := runForgeRuntimeCLI(
				t, executable, server.URL, token, t.TempDir(), "--json", "remote", "placement",
				"scheduler-preview", "--input", inputPath, "--instance", "client-tui",
			)
			if hiddenErr == nil || !strings.Contains(hiddenStderr, "no scheduler-selection request was sent") {
				t.Fatalf("hidden client-instance scheduler preview Runtime CLI was not rejected before POST: stderr=%q err=%v",
					hiddenStderr, hiddenErr)
			}
			assertSchedulerPreviewCLIRequestOrder(t, requestOrder[hiddenStart:], false)
		}

		if runRuntimeTUI {
			visibleStart := len(requestOrder)
			visibleOutput := runForgeRuntimeClientInstanceSchedulerSelectionPreviewTUI(
				t, executable, server.URL, token, inputPath, "client-web",
			)
			for _, want := range []string{
				"scheduler selection preview [forge.scheduler-selection-preview/v1]",
				"conversation=conversation-a run=run-a attempt=attempt-a",
				"selected=device-a/runner-a reason=first_sorted_eligible_candidate",
				"preview_only=true authority: placement_selected=false reservation_created=false lease_issued=false execution_authorized=false dispatch_performed=false audit_published=false",
			} {
				if !strings.Contains(visibleOutput, want) {
					t.Fatalf("visible client-instance TUI output omitted %q: %q", want, visibleOutput)
				}
			}
			assertSchedulerPreviewClientInstanceRequestOrder(t, requestOrder[visibleStart:], true)

			hiddenStart := len(requestOrder)
			hiddenOutput := runForgeRuntimeClientInstanceSchedulerSelectionPreviewTUI(
				t, executable, server.URL, token, inputPath, "client-tui",
			)
			if !strings.Contains(hiddenOutput, "conversation \"conversation-a\" is not declared for instance \"client-tui\"") ||
				!strings.Contains(hiddenOutput, "No request was sent") {
				t.Fatalf("hidden client-instance TUI did not fail closed: %q", hiddenOutput)
			}
			assertSchedulerPreviewClientInstanceRequestOrder(t, requestOrder[hiddenStart:], false)
		}
	}

	// Device observation access alone cannot invoke the planning endpoint. The
	// scope middleware rejects the request before the Run, inventory, or policy
	// sources are touched.
	underScopedToken := snaplinkForgeLoginWithPolicy(
		t, ssoClient, issuer, "forge-console",
		[]string{"openid", "profile", "forge:conversations:read", deviceInventoryReadCandidateScope},
		[]string{snaplinkForgeTestAudience},
	)
	underScopedPreview := snaplinkConversationRequest(t, client, server.URL, underScopedToken,
		http.MethodPost, schedulerSelectionPreviewPath, "", string(requestBody))
	if underScopedPreview.StatusCode != http.StatusForbidden {
		t.Fatalf("under-scoped scheduler preview status=%d body=%q, want 403", underScopedPreview.StatusCode, readConversationClientBody(t, underScopedPreview))
	}
	expectedProjectionReads := 1
	expectedPlanningReads := 1
	if runRuntimeCLI {
		expectedProjectionReads += 2
		expectedPlanningReads++
	}
	if runRuntimeTUI {
		expectedProjectionReads += 2
		expectedPlanningReads++
	}
	expectedInventoryProjectionReads := 0
	if runRuntimeCLI {
		expectedInventoryProjectionReads++
	}
	if runRuntimeTUI {
		// The visible TUI first explicitly opens the pair, then the planning
		// boundary refreshes it immediately before POST.
		expectedInventoryProjectionReads += 2
	}
	if sessionSource.calls != expectedProjectionReads ||
		resourceSource.calls != expectedProjectionReads+expectedInventoryProjectionReads ||
		projectionInventorySource.calls != expectedInventoryProjectionReads ||
		inventorySource.calls != expectedPlanningReads ||
		policySource.calls != expectedPlanningReads ||
		backend.runCalls != expectedPlanningReads {
		t.Fatalf("under-scoped scheduler preview touched dependencies session=%d resource=%d projection_inventory=%d scheduler_inventory=%d policy=%d runs=%d",
			sessionSource.calls, resourceSource.calls, projectionInventorySource.calls, inventorySource.calls, policySource.calls, backend.runCalls)
	}

	// The ordinary production constructor remains closed for the preview path,
	// even when presented with the full-scope real Snaplink JWT.
	production := httptest.NewServer(authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil)))
	t.Cleanup(production.Close)
	productionPreview := snaplinkConversationRequest(t, production.Client(), production.URL, token,
		http.MethodPost, schedulerSelectionPreviewPath, "", string(requestBody))
	productionBody := readConversationClientBody(t, productionPreview)
	if productionPreview.StatusCode != http.StatusNotFound || productionBody != string(notFoundBody) {
		t.Fatalf("default production scheduler preview status=%d body=%q, want closed 404", productionPreview.StatusCode, productionBody)
	}
}

func runForgeRuntimeClientInstanceSchedulerSelectionPreviewTUI(
	t *testing.T, executable, apiURL, accessToken, inputPath, instanceID string,
) string {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("client-instance scheduler preview TUI E2E requires script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	commands := "client-instances show-converged\n" + "instance " + instanceID + "\n"
	// Keep the hidden-instance proof at the pair boundary. The visible client
	// explicitly opens the inventory/resource observation that is refreshed at
	// the planning boundary.
	if instanceID == "client-web" {
		commands += "inventory show-converged\n"
	}
	commands += "scheduler-selection-preview --input " + inputPath + "\nquit\n"
	command.Stdin = strings.NewReader(commands)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated client-instance scheduler preview TUI failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("authenticated client-instance scheduler preview TUI output exceeded the size limit")
	}
	return stdoutBuffer.String()
}

func assertSchedulerPreviewCLIRequestOrder(t *testing.T, got []string, expectPreview bool) {
	t.Helper()
	want := []string{
		http.MethodGet + " " + clientInstanceSessionViewCandidatePath,
		http.MethodGet + " " + clientInstanceResourceViewCandidatePath,
	}
	if expectPreview {
		want = append(want,
			http.MethodGet+" "+deviceInventoryReadCandidateV2Path,
			http.MethodGet+" "+clientInstanceResourceViewCandidatePath,
		)
		want = append(want, http.MethodPost+" "+schedulerSelectionPreviewPath)
	}
	if len(got) != len(want) {
		t.Fatalf("client-instance scheduler preview CLI request order=%#v, want %#v", got, want)
	}
	for index := range want {
		if got[index] != want[index] {
			t.Fatalf("client-instance scheduler preview CLI request[%d]=%q, want %q (pair must precede preview POST); full=%#v", index, got[index], want[index], got)
		}
	}
}

func assertSchedulerPreviewClientInstanceRequestOrder(t *testing.T, got []string, expectPreview bool) {
	t.Helper()
	want := []string{
		http.MethodGet + " " + conversationCollectionPath,
		http.MethodGet + " " + clientInstanceSessionViewCandidatePath,
		http.MethodGet + " " + clientInstanceResourceViewCandidatePath,
	}
	if expectPreview {
		want = append(want,
			http.MethodGet+" "+deviceInventoryReadCandidateV2Path,
			http.MethodGet+" "+clientInstanceResourceViewCandidatePath,
			http.MethodGet+" "+deviceInventoryReadCandidateV2Path,
			http.MethodGet+" "+clientInstanceResourceViewCandidatePath,
			http.MethodPost+" "+schedulerSelectionPreviewPath,
		)
	}
	if len(got) != len(want) {
		t.Fatalf("client-instance scheduler preview TUI request order=%#v, want %#v", got, want)
	}
	for index := range want {
		if got[index] != want[index] {
			t.Fatalf("client-instance scheduler preview TUI request[%d]=%q, want %q (pair must precede preview POST); full=%#v", index, got[index], want[index], got)
		}
	}
}
