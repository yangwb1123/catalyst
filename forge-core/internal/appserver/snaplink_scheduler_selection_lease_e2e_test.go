//go:build linux && !android

package appserver

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/executionlease"
	"forgeos/forge-core/internal/runtimebridge"
	runtimebridgemodel "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
)

// TestRunAcceptedExecuteSchedulerLeaseClaimsAcrossClients crosses the real
// Snaplink JWT and Forge Run boundaries. One client claims a fenced lease,
// independent Console clients replay the exact idempotent receipt, and the
// Runtime CLI/TUI can consume the same route when the real binary is enabled.
// The Core, Console, and Runtime CLI clients deliberately reuse one
// idempotency key so a semantically identical request encoded by different
// clients proves an exact fenced replay. The TUI then uses a fresh key to
// claim the next eligible target.
func TestRunAcceptedExecuteSchedulerLeaseClaimsAcrossClients(t *testing.T) {
	if os.Getenv("FORGE_RUNTIME_BIN") == "" {
		t.Skip("scheduler lease E2E requires a configured Forge Runtime binary for durable Run binding")
	}
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)

	root := privateTestParent(t)
	stateDir := filepath.Join(root, "app-state-scheduler-lease-e2e")
	runtimeStateDir := filepath.Join(root, "runtime-state-scheduler-lease-e2e")
	if err := os.Mkdir(runtimeStateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	runtimeExecutable := filepath.Join(root, "forge-runtime-scheduler-lease-e2e")
	configuredRuntime := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredRuntime != "" {
		runtimeExecutable = configuredRuntime
		initializeRuntimeHubForIntegration(t, runtimeExecutable, runtimeStateDir)
	} else if err := os.WriteFile(runtimeExecutable, []byte("test runtime"), 0o700); err != nil {
		t.Fatal(err)
	}

	owner := deviceidentity.Owner{
		Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant,
	}
	conversationID := "conversation-lease-e2e"
	runID := "run-lease-e2e"
	if configuredRuntime != "" {
		projectPath, projectID, _ := seedRuntimeConversationScopes(t, runtimeExecutable, runtimeStateDir)
		if err := os.WriteFile(filepath.Join(projectPath, "README.md"), []byte("deterministic scheduler lease fixture\n"), 0o600); err != nil {
			t.Fatal(err)
		}
		bridgeStateDir := filepath.Join(root, "bridge-state-scheduler-lease-e2e")
		if err := os.Mkdir(bridgeStateDir, 0o700); err != nil {
			t.Fatal(err)
		}
		bridge, err := runtimebridge.New(runtimebridge.Config{
			Executable: runtimeExecutable, AppServerStateDir: bridgeStateDir,
			RuntimeStateDir: runtimeStateDir, Timeout: 5 * time.Second,
		})
		if err != nil {
			t.Fatal(err)
		}
		conversation, err := bridge.CreateOwnedConversation(
			context.Background(),
			runtimebridgemodel.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			runtimebridgemodel.ConversationScope{Kind: "project", ID: projectID},
			"Scheduler lease E2E", "scheduler-lease-e2e-create",
		)
		if err != nil {
			t.Fatalf("create Runtime conversation for scheduler lease Gate E2E: %v", err)
		}
		conversationID = conversation.ID
		prompt, _, replayed, err := bridge.AppendOwnedPrompt(
			context.Background(),
			runtimebridgemodel.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, "reserve a target for this scheduler lease preview", "scheduler-lease-e2e-prompt", 1,
		)
		if err != nil || replayed || prompt.ID == "" {
			t.Fatalf("append Runtime Prompt for scheduler lease E2E: prompt=%#v replayed=%v err=%v", prompt, replayed, err)
		}
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		command := exec.CommandContext(ctx, runtimeExecutable,
			"--state-dir", runtimeStateDir, "--json", "--idempotency-key", "scheduler-lease-e2e-start",
			"-C", projectPath, "run", "start", conversationID, prompt.ID, "--read", "README.md")
		command.Env = []string{}
		output, err := command.CombinedOutput()
		cancel()
		if err != nil || len(output) == 0 {
			t.Fatalf("seed deterministic scheduler lease Run: %v: %s", err, output)
		}
		page, err := bridge.OwnedConversationRuns(
			context.Background(),
			runtimebridgemodel.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, nil, 25,
		)
		if err != nil || len(page.Runs) != 1 || page.Runs[0].PromptID != prompt.ID {
			t.Fatalf("read seeded scheduler lease Run page=%#v err=%v", page, err)
		}
		runID = page.Runs[0].RunID
	}
	now := time.Now().UnixMilli()
	first := schedulerLeaseFreshLifecycleState(t, owner, "device-a", "runner-a", 1, now)
	second := schedulerLeaseFreshLifecycleState(t, owner, "device-b", "runner-b", 1, now)
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{second, first})
	policyPath := writePlacementPolicySourceFile(t, owner, []deviceplacement.PlacementPolicy{
		schedulerLeasePolicy(first),
		schedulerLeasePolicy(second),
	})
	clientInstancePath := writeClientInstanceSessionViewSourceFile(t, owner)
	leasePath := filepath.Join(filepath.Dir(registryPath), "leases.json")
	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{
		Status: "accepted", AcceptanceID: "p4-scheduler-lease-e2e-001", AcceptedAtUnixMS: 1,
	}
	activation.Evidence.RunnerIsolation = true
	activation.Evidence.LeaseFencing = true
	activation.Evidence.CancellationAndUncertainWork = true
	activation.Evidence.VaultArtifactAuthorization = true
	activation.Evidence.AuditOutbox = true
	config := Config{
		ListenAddress:                        "127.0.0.1:0",
		StateDir:                             stateDir,
		Build:                                BuildInfo{Version: "test"},
		RuntimeExecutable:                    runtimeExecutable,
		RuntimeStateDir:                      runtimeStateDir,
		SnaplinkIssuer:                       issuer,
		SnaplinkAudience:                     snaplinkForgeTestAudience,
		SnaplinkJWKSURL:                      issuer + "/.well-known/jwks.json",
		ExpectedTenantID:                     snaplinkForgeTestTenant,
		ExpectedSubjectID:                    snaplinkForgeTestUser,
		JWKSHTTPClient:                       ssoClient,
		JWKSRefreshInterval:                  24 * time.Hour,
		DeviceFabricActivation:               ptrDeviceFabricRequest(activation),
		DeviceInventoryLifecycleRegistryFile: registryPath,
		DeviceClientInstanceSessionViewFile:  clientInstancePath,
		DeviceExecutionLeaseRegistryFile:     leasePath,
		DeviceExecutionPolicyRegistryFile:    policyPath,
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	readyChannel := make(chan Ready, 1)
	result := make(chan error, 1)
	go func() {
		result <- Run(ctx, config, func(ready Ready) error {
			readyChannel <- ready
			return nil
		})
	}()
	var ready Ready
	select {
	case ready = <-readyChannel:
	case err := <-result:
		t.Fatalf("scheduler lease E2E server failed before readiness: %v", err)
	case <-time.After(5 * time.Second):
		t.Fatal("scheduler lease E2E server did not announce readiness")
	}
	t.Cleanup(func() {
		cancel()
		if err := waitResult(t, result); err != nil {
			t.Error(err)
		}
	})

	requirements := deviceplacement.Requirements{
		OS: "linux", Architecture: "amd64", MinCPUCores: 1,
		MinMemoryBytes: 1, MinStorageBytes: 1, Runtime: "oci",
		GPU: deviceplacement.GPURequirement{}, DataResidencyZones: []string{"us-west"},
		MinimumTrustZone: "standard", SandboxFloor: "container", ConcurrencySlots: 1,
	}
	leaseRequest := schedulerSelectionLeaseRequest{
		ConversationID: conversationID, RunID: runID, AttemptID: "attempt-lease-e2e",
		Requirements: requirements, TTLMS: 30_000,
	}
	previewRequest := schedulerSelectionPreviewRequest{
		ConversationID: conversationID, RunID: runID, AttemptID: leaseRequest.AttemptID,
		Requirements: requirements,
	}
	body, err := json.Marshal(leaseRequest)
	if err != nil {
		t.Fatal(err)
	}
	previewBody, err := json.Marshal(previewRequest)
	if err != nil {
		t.Fatal(err)
	}
	client := &http.Client{Timeout: 5 * time.Second}
	previewReq, err := http.NewRequest(
		http.MethodPost, ready.Listen+schedulerSelectionPreviewPath, strings.NewReader(string(previewBody)),
	)
	if err != nil {
		t.Fatal(err)
	}
	previewReq.Header.Set("Authorization", "Bearer "+token)
	previewReq.Header.Set("Content-Type", "application/json")
	previewResponse, err := client.Do(previewReq)
	if err != nil {
		t.Fatal(err)
	}
	previewPayload, err := io.ReadAll(previewResponse.Body)
	_ = previewResponse.Body.Close()
	if err != nil {
		t.Fatal(err)
	}
	if previewResponse.StatusCode != http.StatusOK {
		t.Fatalf("policy-complete scheduler preview status=%d body=%q", previewResponse.StatusCode, previewPayload)
	}
	var preview deviceplacement.SchedulerSelectionPreviewObservation
	if err := json.Unmarshal(previewPayload, &preview); err != nil {
		t.Fatalf("decode policy-complete scheduler preview: %v body=%q", err, previewPayload)
	}
	if err := preview.Validate(); err != nil || !preview.SelectionAvailable ||
		preview.SelectedDeviceID == nil || *preview.SelectedDeviceID != "device-a" ||
		preview.SelectedInstanceID == nil || *preview.SelectedInstanceID != "runner-a" ||
		preview.SelectionReason != "first_sorted_eligible_candidate" ||
		preview.Authority != (deviceplacement.SchedulerSelectionPreviewAuthority{}) {
		t.Fatalf("policy-complete scheduler preview=%#v err=%v", preview, err)
	}
	policyPreviewExpectation := schedulerSelectionPreviewExpectation{
		SelectionAvailable: true,
		SelectionReason:    "first_sorted_eligible_candidate",
		DeviceID:           "device-a",
		InstanceID:         "runner-a",
	}
	if configuredRuntime != "" {
		runtimeOwner := runtimebridgemodel.Owner{
			Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant,
		}
		runForgeRuntimeSchedulerSelectionRemoteCLIWithRequest(
			t, configuredRuntime, ready.Listen, token, runtimeOwner, previewRequest,
			policyPreviewExpectation,
		)
		runForgeRuntimeSchedulerSelectionRemoteTUIWithRequest(
			t, configuredRuntime, ready.Listen, token, runtimeOwner, previewRequest,
			policyPreviewExpectation,
		)
	}
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		runForgeConsoleSchedulerSelectionPreviewE2EWithToken(
			t, ready.Listen, token,
			runtimebridgemodel.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant},
			requirements, conversationID, runID, policyPreviewExpectation,
		)
		if configuredRuntime != "" {
			runForgeConsoleSchedulerSelectionPreviewGateE2EWithToken(
				t, ready.Listen, token,
				runtimebridgemodel.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant},
				requirements, conversationID, runID,
				policyPreviewExpectation,
			)
		}
	}
	request := func(idempotencyKey string) (int, []byte) {
		t.Helper()
		req, err := http.NewRequest(http.MethodPost, ready.Listen+schedulerSelectionLeasePath, strings.NewReader(string(body)))
		if err != nil {
			t.Fatal(err)
		}
		req.Header.Set("Authorization", "Bearer "+token)
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("Idempotency-Key", idempotencyKey)
		response, err := client.Do(req)
		if err != nil {
			t.Fatal(err)
		}
		defer response.Body.Close()
		payload, err := io.ReadAll(response.Body)
		if err != nil {
			t.Fatal(err)
		}
		return response.StatusCode, payload
	}

	const idempotencyKey = "scheduler-lease-e2e-key-00000001"
	status, payload := request(idempotencyKey)
	if status != http.StatusOK {
		t.Fatalf("production scheduler lease claim status=%d body=%q", status, payload)
	}
	var firstLease schedulerSelectionLeaseResponse
	if err := json.Unmarshal(payload, &firstLease); err != nil {
		t.Fatalf("decode production scheduler lease: %v body=%q", err, payload)
	}
	assertSchedulerLeaseE2EReceipt(t, firstLease, owner, leaseRequest, "device-a", "runner-a", false)

	status, payload = request(idempotencyKey)
	if status != http.StatusOK {
		t.Fatalf("production scheduler lease replay status=%d body=%q", status, payload)
	}
	var replay schedulerSelectionLeaseResponse
	if err := json.Unmarshal(payload, &replay); err != nil {
		t.Fatalf("decode production scheduler lease replay: %v", err)
	}
	assertSchedulerLeaseE2EReceipt(t, replay, owner, leaseRequest, "device-a", "runner-a", true)
	if replay.Grant != firstLease.Grant {
		t.Fatalf("scheduler lease replay changed grant: first=%#v replay=%#v", firstLease.Grant, replay.Grant)
	}

	var runtimeRenewal schedulerSelectionLeaseResponse
	if configuredRuntime != "" {
		runForgeRuntimeSchedulerLeaseRemoteCLI(t, configuredRuntime, ready.Listen, token, body, owner, idempotencyKey, "device-a", "runner-a", true)
		runtimeRenewal = runForgeRuntimeSchedulerLeaseRenewalRemoteCLI(t, configuredRuntime, ready.Listen, token, firstLease, owner)
		runForgeRuntimeSchedulerLeaseRemoteTUI(t, configuredRuntime, ready.Listen, token, body, owner, "device-b", "runner-b")
	}
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		runForgeConsoleSchedulerSelectionLeaseE2EWithToken(t, ready.Listen, token, owner, leaseRequest, idempotencyKey, "device-a", "runner-a")
		if configuredRuntime != "" {
			runForgeConsoleSchedulerSelectionLeaseGateE2EWithToken(t, ready.Listen, token, owner, leaseRequest, idempotencyKey, "device-a", "runner-a")
			if runtimeRenewal.Grant.Epoch == 0 {
				t.Fatalf("runtime scheduler lease renewal did not produce a valid grant")
			}
			// Exercise one fresh cross-client renewal through Core, then let the
			// Console API clients replay that exact receipt. The Gate receives the
			// resulting proof and performs the final fenced release.
			consoleRenewalRequest := schedulerSelectionLeaseRenewalRequest{
				ConversationID: runtimeRenewal.ConversationID, RunID: runtimeRenewal.RunID,
				AttemptID: runtimeRenewal.AttemptID, TargetID: runtimeRenewal.Grant.TargetID,
				Epoch: runtimeRenewal.Grant.Epoch, FencingToken: runtimeRenewal.Grant.FencingToken,
				TTLMS: 30_000,
			}
			consoleRenewal := postSchedulerSelectionLeaseRenewal(
				t, ready.Listen, token, consoleRenewalRequest, "scheduler-lease-console-renew-key-00001",
			)
			runForgeConsoleSchedulerSelectionLeaseRenewalE2EWithToken(
				t, ready.Listen, token, owner, consoleRenewalRequest,
				"scheduler-lease-console-renew-key-00001", consoleRenewal,
			)
			consoleReleaseRequest := schedulerSelectionLeaseReleaseRequest{
				ConversationID: consoleRenewal.ConversationID, RunID: consoleRenewal.RunID,
				AttemptID: consoleRenewal.AttemptID, TargetID: consoleRenewal.Grant.TargetID,
				Epoch: consoleRenewal.Grant.Epoch, FencingToken: consoleRenewal.Grant.FencingToken,
			}
			runForgeConsoleSchedulerSelectionLeaseReleaseGateE2EWithToken(
				t, ready.Listen, token, consoleReleaseRequest,
				"scheduler-lease-console-release-key-00001", consoleRenewal,
			)
		}
	}
}

// TestSnaplinkAuthenticatedClientInstanceSchedulerLeaseProjectionE2EWhenConfigured
// crosses a real Snaplink JWT with the Runtime CLI/TUI and shared Flutter
// Web/App/Mobile instance-aware lease lifecycle. The Console path composes the
// session/resource and inventory/resource readers before claim. All candidate
// handlers are deliberately mounted on a test mux: the ordinary production
// constructor below must continue to return 404, and a hidden client instance
// must be rejected before the lease POST is observed by the candidate route.
func TestSnaplinkAuthenticatedClientInstanceSchedulerLeaseProjectionE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_SCHEDULER_LEASE_PROJECTION_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_SCHEDULER_LEASE_PROJECTION_E2E=1 for instance-aware scheduler lease E2E")
	}
	configuredExecutable := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredExecutable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for instance-aware scheduler lease E2E")
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
	instances := []deviceplacement.ClientInstanceSessionViewInstance{
		{InstanceID: "client-cli-001", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{"conversation-lease-instance-a"}, ObservedAtMS: 200_500, Status: "active"},
		// Keep the TUI declaration on the same owner-scoped Conversation as
		// the CLI so the test can exercise a second authenticated client
		// against the released target without adding another backend fixture.
		{InstanceID: "client-tui-001", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{"conversation-lease-instance-a", "conversation-lease-instance-b"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-web-001", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{"conversation-lease-instance-b"}, ObservedAtMS: 200_500, Status: "idle"},
		{InstanceID: "client-app-001", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{"conversation-lease-instance-a"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-mobile-001", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{"conversation-lease-instance-b"}, ObservedAtMS: 200_500, Status: "idle"},
	}
	sessionView, err := deviceplacement.ObserveClientInstanceSessionView(
		deviceplacement.ClientInstanceSessionViewRequest{Owner: owner, Instances: instances},
	)
	if err != nil {
		t.Fatalf("observe five-instance session view: %v", err)
	}
	resourceView := fixtureClientInstanceResourceView(owner)
	resourceView.Instances = append([]deviceplacement.ClientInstanceSessionViewInstance(nil), sessionView.Instances...)
	resourceView.Devices[0].ObservedAtMS = 250_000
	secondResource := resourceView.Devices[0]
	secondResource.DeviceID = "device-b"
	secondResource.RunnerInstanceID = "runner-b"
	secondResource.Revision = 2
	secondResource.Generation = 2
	secondResource.HeartbeatSequence = 2
	secondResource.ObservedAtMS = 250_000
	resourceView.Devices = append(resourceView.Devices, secondResource)
	if err := resourceView.Validate(); err != nil {
		t.Fatalf("validate five-instance resource view: %v", err)
	}
	sessionSource := &fixtureClientInstanceSessionViewSource{value: sessionView}
	resourceSource := &fixtureClientInstanceResourceViewSource{value: resourceView}

	// The lease candidate consumes the same owner-scoped resource image as the
	// paired client-instance projection.  The policy image supplies the
	// planning attributes intentionally absent from the read-only resource view.
	inventory := fixtureDeviceInventoryReadV2Value(owner)
	inventory.Devices[0].Device.ReservationState = "none"
	inventory.Devices[0].Device.SnapshotObservedAtMS = 250_000
	inventory.Devices[0].Device.LeaseExpiresAtMS = 600_000
	inventory.Devices[0].Device.GPUs = []deviceplacement.GPUDeclarationV2{}
	secondInventory := inventory.Devices[0]
	secondInventory.InstanceID = "runner-b"
	secondInventory.Revision = 2
	secondInventory.Generation = 2
	secondInventory.HeartbeatSequence = 2
	secondInventory.Device.DeviceID = "device-b"
	secondInventory.Device.Runtimes = []string{"python"}
	inventory.Devices = append(inventory.Devices, secondInventory)
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
		}, {
			DeviceID: "device-b", InstanceID: "runner-b", Revision: 2, Generation: 2,
			HeartbeatSequence: 2, DataResidencyZones: []string{"us-west"},
			TrustZone: "untrusted", SandboxLevels: []string{"process"},
			ConcurrencyLimit: 1, ActiveConcurrency: 0,
		}},
	}}
	conversation := runtimebridgemodel.Conversation{
		ID: "conversation-lease-instance-a", Scope: runtimebridgemodel.ConversationScope{Kind: "global"},
		Title: "Scheduler lease instance projection", CreatedAtMS: 1, UpdatedAtMS: 2,
	}
	backend := &fakeConversationBackend{
		listPage: runtimebridgemodel.OwnedConversationPage{
			Conversations: []runtimebridgemodel.OwnedConversationEntry{{Conversation: conversation, AggregateVersion: 1}},
		},
		detail: runtimebridgemodel.OwnedConversationEntry{Conversation: conversation, AggregateVersion: 1},
		promptPage: runtimebridgemodel.ConversationPromptPage{
			ConversationID: conversation.ID, Prompts: []runtimebridgemodel.ConversationPrompt{},
		},
		runPage: runmodel.OwnedRunPage{
			ConversationID: "conversation-lease-instance-a",
			Runs: []runmodel.OwnedRunSummary{{
				RunID: "run-lease-instance-a", PromptID: "prompt-lease-instance-a", CreatedAtMS: 1,
				LatestSequence: 1, Status: "completed",
			}},
		},
		timelinePage: runmodel.OwnedRunTimelinePage{
			ConversationID: conversation.ID, RunID: "run-lease-instance-a",
			AfterSequence: 0, ScannedThroughSequence: 0, Events: []runmodel.OwnedRunEventSummary{},
		},
	}
	leaseRegistryDir := t.TempDir()
	if err := os.Chmod(leaseRegistryDir, 0o700); err != nil {
		t.Fatal(err)
	}
	leaseRegistryPath := filepath.Join(leaseRegistryDir, "scheduler-lease-instance-projection.json")
	leaseClock := int64(300_000)
	leaseConfig := &schedulerSelectionLeaseConfig{
		Enabled: true, Source: inventorySource, PolicySource: policySource,
		Now:          func(context.Context) (int64, error) { leaseClock++; return leaseClock, nil },
		RegistryPath: leaseRegistryPath, Backend: backend,
	}
	testRoutes := http.NewServeMux()
	// The interactive TUI performs its ordinary owner-scoped session list on
	// startup before opening the explicit client-instance projection. Keep that
	// read available while excluding it from the pair ordering assertions below.
	conversationRoutes := newConversationRoutesWithBackend(backend)
	testRoutes.Handle(conversationCollectionPath, conversationRoutes)
	testRoutes.Handle(conversationCollectionPath+"/", conversationRoutes)
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
	testRoutes.Handle(schedulerSelectionLeasePath, newSchedulerSelectionLeaseRoutes(leaseConfig))
	testRoutes.Handle(schedulerSelectionLeaseRenewalPath, newSchedulerSelectionLeaseRenewalRoutes(leaseConfig))
	testRoutes.Handle(schedulerSelectionLeaseReleasePath, newSchedulerSelectionLeaseReleaseRoutes(leaseConfig))
	var requestOrder []string
	recordedRoutes := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.EscapedPath() != conversationCollectionPath {
			requestOrder = append(requestOrder, r.Method+" "+r.URL.EscapedPath())
		}
		testRoutes.ServeHTTP(w, r)
	})
	server := httptest.NewServer(authenticator.Handler(recordedRoutes))
	t.Cleanup(server.Close)

	requirements := deviceplacement.Requirements{
		OS: "linux", Architecture: "amd64", MinCPUCores: 1,
		MinMemoryBytes: 1, MinStorageBytes: 1, Runtime: "go",
		GPU: deviceplacement.GPURequirement{}, DataResidencyZones: []string{"us-west"},
		MinimumTrustZone: "untrusted", SandboxFloor: "process", ConcurrencySlots: 1,
	}
	claimRequest := schedulerSelectionLeaseRequest{
		ConversationID: "conversation-lease-instance-a", RunID: "run-lease-instance-a",
		AttemptID: "attempt-lease-instance", Requirements: requirements, TTLMS: 30_000,
	}
	claimBody, err := json.Marshal(claimRequest)
	if err != nil {
		t.Fatal(err)
	}
	claimInput := filepath.Join(t.TempDir(), "scheduler-lease-instance-claim.json")
	if err := os.WriteFile(claimInput, claimBody, 0o600); err != nil {
		t.Fatal(err)
	}
	claimStart := len(requestOrder)
	claimOutput, claimStderr, err := runForgeRuntimeCLI(
		t, executable, server.URL, token, t.TempDir(),
		"--json", "--idempotency-key", "scheduler-lease-instance-claim-00001",
		"remote", "placement", "scheduler-lease", "--input", claimInput, "--instance", "client-cli-001",
	)
	if err != nil {
		t.Fatalf("instance-aware scheduler lease claim failed: stderr=%q stdout=%q err=%v", claimStderr, claimOutput, err)
	}
	var claimed schedulerSelectionLeaseResponse
	if err := json.Unmarshal([]byte(claimOutput), &claimed); err != nil {
		t.Fatalf("decode instance-aware scheduler lease claim: %v stdout=%q", err, claimOutput)
	}
	assertSchedulerLeaseE2EReceipt(t, claimed, deviceidentity.Owner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	}, claimRequest, "device-a", "runner-a", false)
	assertSchedulerLeaseProjectionRequestOrder(t, requestOrder[claimStart:], schedulerSelectionLeasePath)

	// The shared Flutter Sessions Gate serves Web, desktop App, and Mobile. Give
	// each selected instance the same visible Conversation for this bounded
	// replay, then require both independently composed observation pairs before
	// the existing fenced lease can be replayed. The source image is restored
	// before the CLI/TUI hidden-instance assertions below.
	consoleSessionView, err := deviceplacement.ObserveClientInstanceSessionView(
		deviceplacement.ClientInstanceSessionViewRequest{
			Owner: owner,
			Instances: []deviceplacement.ClientInstanceSessionViewInstance{
				{InstanceID: "client-cli-001", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{"conversation-lease-instance-a"}, ObservedAtMS: 200_500, Status: "active"},
				{InstanceID: "client-tui-001", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{"conversation-lease-instance-a"}, ObservedAtMS: 200_500, Status: "active"},
				{InstanceID: "client-web-001", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{"conversation-lease-instance-a"}, ObservedAtMS: 200_500, Status: "idle"},
				{InstanceID: "client-app-001", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{"conversation-lease-instance-a"}, ObservedAtMS: 200_500, Status: "active"},
				{InstanceID: "client-mobile-001", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{"conversation-lease-instance-a"}, ObservedAtMS: 200_500, Status: "idle"},
			},
		},
	)
	if err != nil {
		t.Fatalf("observe Console scheduler lease session view: %v", err)
	}
	consoleResourceView := resourceView
	consoleResourceView.Instances = append([]deviceplacement.ClientInstanceSessionViewInstance(nil), consoleSessionView.Instances...)
	if err := consoleResourceView.Validate(); err != nil {
		t.Fatalf("validate Console scheduler lease resource view: %v", err)
	}
	originalSessionView := sessionSource.value
	originalResourceView := resourceSource.value
	sessionSource.value = consoleSessionView
	resourceSource.value = consoleResourceView
	consoleStart := len(requestOrder)
	t.Cleanup(func() {
		if t.Failed() {
			t.Logf("Console scheduler lease projection requests: %#v backend list=%d detail=%d prompts=%d runs=%d timeline=%d",
				requestOrder[consoleStart:], backend.listCalls, backend.detailCalls, backend.promptCalls, backend.runCalls, backend.timelineCalls)
		}
	})
	runForgeConsoleClientInstanceSchedulerLeaseProjectionE2EWithToken(
		t, server.URL, token, owner, claimRequest,
		"scheduler-lease-instance-claim-00001", "device-a", "runner-a",
		"conversation-lease-instance-b",
	)
	assertConsoleClientInstanceSchedulerLeaseProjectionRequests(t, requestOrder[consoleStart:])
	sessionSource.value = originalSessionView
	resourceSource.value = originalResourceView

	// The Web instance does not declare the claimed Conversation.  The CLI
	// must read both pair members, then fail locally without contacting lease.
	hiddenClaimStart := len(requestOrder)
	_, hiddenClaimStderr, hiddenClaimErr := runForgeRuntimeCLI(
		t, executable, server.URL, token, t.TempDir(),
		"--json", "--idempotency-key", "scheduler-lease-instance-hidden-00001",
		"remote", "placement", "scheduler-lease", "--input", claimInput, "--instance", "client-web-001",
	)
	if hiddenClaimErr == nil || !strings.Contains(hiddenClaimStderr, "no scheduler lease request was sent") {
		t.Fatalf("hidden instance scheduler lease claim escaped pair guard: stderr=%q err=%v", hiddenClaimStderr, hiddenClaimErr)
	}
	assertSchedulerLeaseProjectionReadOnlyPair(t, requestOrder[hiddenClaimStart:])

	renewalRequest := schedulerSelectionLeaseRenewalRequest{
		ConversationID: claimed.ConversationID, RunID: claimed.RunID, AttemptID: claimed.AttemptID,
		TargetID: claimed.Grant.TargetID, Epoch: claimed.Grant.Epoch,
		FencingToken: claimed.Grant.FencingToken, TTLMS: 30_000,
	}
	renewalBody, err := json.Marshal(renewalRequest)
	if err != nil {
		t.Fatal(err)
	}
	renewalInput := filepath.Join(t.TempDir(), "scheduler-lease-instance-renewal.json")
	if err := os.WriteFile(renewalInput, renewalBody, 0o600); err != nil {
		t.Fatal(err)
	}
	renewalStart := len(requestOrder)
	renewalOutput, renewalStderr, err := runForgeRuntimeCLI(
		t, executable, server.URL, token, t.TempDir(),
		"--json", "--idempotency-key", "scheduler-lease-instance-renew-00001",
		"remote", "placement", "scheduler-lease-renew", "--input", renewalInput, "--instance", "client-cli-001",
	)
	if err != nil {
		t.Fatalf("instance-aware scheduler lease renewal failed: stderr=%q stdout=%q err=%v", renewalStderr, renewalOutput, err)
	}
	var renewed schedulerSelectionLeaseResponse
	if err := json.Unmarshal([]byte(renewalOutput), &renewed); err != nil {
		t.Fatalf("decode instance-aware scheduler lease renewal: %v stdout=%q", err, renewalOutput)
	}
	if renewed.Owner != claimed.Owner || renewed.ConversationID != claimed.ConversationID ||
		renewed.RunID != claimed.RunID || renewed.AttemptID != claimed.AttemptID ||
		renewed.DeviceID != claimed.DeviceID || renewed.InstanceID != claimed.InstanceID ||
		renewed.Replayed || renewed.Grant.Epoch != claimed.Grant.Epoch+1 ||
		renewed.Authority.ExecutionAuthorized || renewed.Authority.DispatchPerformed || renewed.Authority.AuditPublished {
		t.Fatalf("instance-aware scheduler lease renewal=%#v claimed=%#v", renewed, claimed)
	}
	assertSchedulerLeaseProjectionRequestOrder(t, requestOrder[renewalStart:], schedulerSelectionLeaseRenewalPath)

	hiddenRenewalStart := len(requestOrder)
	_, hiddenRenewalStderr, hiddenRenewalErr := runForgeRuntimeCLI(
		t, executable, server.URL, token, t.TempDir(),
		"--json", "--idempotency-key", "scheduler-lease-instance-hidden-renew-00001",
		"remote", "placement", "scheduler-lease-renew", "--input", renewalInput, "--instance", "client-web-001",
	)
	if hiddenRenewalErr == nil || !strings.Contains(hiddenRenewalStderr, "no scheduler lease renewal request was sent") {
		t.Fatalf("hidden instance scheduler lease renewal escaped pair guard: stderr=%q err=%v", hiddenRenewalStderr, hiddenRenewalErr)
	}
	assertSchedulerLeaseProjectionReadOnlyPair(t, requestOrder[hiddenRenewalStart:])

	releaseRequest := schedulerSelectionLeaseReleaseRequest{
		ConversationID: renewed.ConversationID, RunID: renewed.RunID, AttemptID: renewed.AttemptID,
		TargetID: renewed.Grant.TargetID, Epoch: renewed.Grant.Epoch,
		FencingToken: renewed.Grant.FencingToken,
	}
	releaseBody, err := json.Marshal(releaseRequest)
	if err != nil {
		t.Fatal(err)
	}
	releaseInput := filepath.Join(t.TempDir(), "scheduler-lease-instance-release.json")
	if err := os.WriteFile(releaseInput, releaseBody, 0o600); err != nil {
		t.Fatal(err)
	}
	releaseStart := len(requestOrder)
	releaseOutput, releaseStderr, err := runForgeRuntimeCLI(
		t, executable, server.URL, token, t.TempDir(),
		"--json", "--idempotency-key", "scheduler-lease-instance-release-00001",
		"remote", "placement", "scheduler-lease-release", "--input", releaseInput, "--instance", "client-cli-001",
	)
	if err != nil {
		t.Fatalf("instance-aware scheduler lease release failed: stderr=%q stdout=%q err=%v", releaseStderr, releaseOutput, err)
	}
	var released schedulerSelectionLeaseReleaseResponse
	if err := json.Unmarshal([]byte(releaseOutput), &released); err != nil {
		t.Fatalf("decode instance-aware scheduler lease release: %v stdout=%q", err, releaseOutput)
	}
	assertSchedulerLeaseReleaseE2EReceipt(t, released, deviceidentity.Owner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	}, releaseRequest, "device-a", "runner-a", false)
	assertSchedulerLeaseProjectionRequestOrder(t, requestOrder[releaseStart:], schedulerSelectionLeaseReleasePath)

	hiddenReleaseStart := len(requestOrder)
	_, hiddenReleaseStderr, hiddenReleaseErr := runForgeRuntimeCLI(
		t, executable, server.URL, token, t.TempDir(),
		"--json", "--idempotency-key", "scheduler-lease-instance-hidden-release-00001",
		"remote", "placement", "scheduler-lease-release", "--input", releaseInput, "--instance", "client-web-001",
	)
	if hiddenReleaseErr == nil || !strings.Contains(hiddenReleaseStderr, "no scheduler lease release request was sent") {
		t.Fatalf("hidden instance scheduler lease release escaped pair guard: stderr=%v err=%v", hiddenReleaseStderr, hiddenReleaseErr)
	}
	assertSchedulerLeaseProjectionReadOnlyPair(t, requestOrder[hiddenReleaseStart:])

	// The authenticated TUI consumes the same owner-scoped pair through its
	// interactive client-instance projection.  This claim runs after the CLI
	// release, so it proves the TUI path can acquire a fresh fenced lease while
	// retaining the same production execution boundary.
	tuiClaimRequest := schedulerSelectionLeaseRequest{
		ConversationID: "conversation-lease-instance-a", RunID: "run-lease-instance-a",
		AttemptID: "attempt-lease-instance-tui", Requirements: requirements, TTLMS: 30_000,
	}
	tuiClaimRequest.Requirements.Runtime = "python"
	tuiClaimBody, err := json.Marshal(tuiClaimRequest)
	if err != nil {
		t.Fatal(err)
	}
	tuiClaimInput := filepath.Join(t.TempDir(), "scheduler-lease-instance-tui-claim.json")
	if err := os.WriteFile(tuiClaimInput, tuiClaimBody, 0o600); err != nil {
		t.Fatal(err)
	}
	tuiClaimStart := len(requestOrder)
	runForgeRuntimeSchedulerLeaseRemoteTUIWithClientInstance(
		t, executable, server.URL, token, tuiClaimInput, "client-tui-001", "device-b", "runner-b",
		true,
	)
	assertSchedulerLeaseProjectionRequestOrder(t, requestOrder[tuiClaimStart:], schedulerSelectionLeasePath)

	// A foreign instance still reads the same pair, then stops before the
	// lease POST.  The TUI must expose the local display-filter rejection and
	// never turn that observation into an authorization attempt.
	tuiHiddenStart := len(requestOrder)
	foreignTUIOutput := runForgeRuntimeSchedulerLeaseRemoteTUIWithClientInstance(
		t, executable, server.URL, token, tuiClaimInput, "client-web-001", "device-b", "runner-b", false,
	)
	if !strings.Contains(foreignTUIOutput, "not declared") ||
		!strings.Contains(foreignTUIOutput, "No request was sent.") {
		t.Fatalf("hidden instance scheduler lease TUI was not rejected locally: %q", foreignTUIOutput)
	}
	assertSchedulerLeaseProjectionReadOnlyPair(t, requestOrder[tuiHiddenStart:])
	expectedSessionReads := countSchedulerLeaseProjectionRequest(requestOrder, http.MethodGet+" "+clientInstanceSessionViewCandidatePath)
	expectedResourceReads := countSchedulerLeaseProjectionRequest(requestOrder, http.MethodGet+" "+clientInstanceResourceViewCandidatePath)
	expectedRunReads := countSchedulerLeaseProjectionRequest(requestOrder, http.MethodPost+" "+schedulerSelectionLeasePath) +
		countSchedulerLeaseProjectionRequest(requestOrder, http.MethodPost+" "+schedulerSelectionLeaseRenewalPath) +
		countSchedulerLeaseProjectionRunCollectionReads(requestOrder)
	if sessionSource.calls != expectedSessionReads || resourceSource.calls != expectedResourceReads || backend.runCalls != expectedRunReads ||
		sessionSource.calls < 12 || resourceSource.calls < 15 || backend.runCalls < 6 ||
		sessionSource.owner != (runtimebridgemodel.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) ||
		resourceSource.owner != (runtimebridgemodel.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) {
		t.Fatalf("instance-aware lease dependency convergence session=%d resource=%d backend_runs=%d session_source=%#v resource_source=%#v",
			sessionSource.calls, resourceSource.calls, backend.runCalls, sessionSource, resourceSource)
	}

	// The ordinary constructor is still closed even with the same real JWT.
	production := httptest.NewServer(authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil)))
	t.Cleanup(production.Close)
	productionCases := []struct {
		path string
		body string
		key  string
	}{
		{path: schedulerSelectionLeasePath, body: string(claimBody), key: "production-lease-closed-00001"},
		{path: schedulerSelectionLeaseRenewalPath, body: string(renewalBody), key: "production-renew-closed-00001"},
		{path: schedulerSelectionLeaseReleasePath, body: string(mustJSON(t, schedulerSelectionLeaseReleaseRequest{
			ConversationID: renewed.ConversationID, RunID: renewed.RunID, AttemptID: renewed.AttemptID,
			TargetID: renewed.Grant.TargetID, Epoch: renewed.Grant.Epoch, FencingToken: renewed.Grant.FencingToken,
		})), key: "production-release-closed-00001"},
	}
	for _, candidate := range productionCases {
		response := snaplinkConversationRequest(t, production.Client(), production.URL, token,
			http.MethodPost, candidate.path, candidate.key, candidate.body)
		body := readConversationClientBody(t, response)
		if response.StatusCode != http.StatusNotFound || body != string(notFoundBody) {
			t.Fatalf("default production scheduler lease path=%s status=%d body=%q, want closed 404", candidate.path, response.StatusCode, body)
		}
	}
	for _, path := range []string{
		clientInstanceSessionViewCandidatePath,
		clientInstanceResourceViewCandidatePath,
		deviceInventoryReadCandidateV2Path,
	} {
		response := snaplinkConversationRequest(t, production.Client(), production.URL, token,
			http.MethodGet, path, "", "")
		body := readConversationClientBody(t, response)
		if response.StatusCode != http.StatusNotFound || body != string(notFoundBody) {
			t.Fatalf("default production scheduler lease dependency path=%s status=%d body=%q, want closed 404", path, response.StatusCode, body)
		}
	}
}

func assertSchedulerLeaseProjectionRequestOrder(t *testing.T, got []string, leasePath string) {
	t.Helper()
	want := []string{
		http.MethodGet + " " + clientInstanceSessionViewCandidatePath,
		http.MethodGet + " " + clientInstanceResourceViewCandidatePath,
		http.MethodPost + " " + leasePath,
	}
	if len(got) != len(want) {
		t.Fatalf("scheduler lease projection request order=%#v, want %#v", got, want)
	}
	for index := range want {
		if got[index] != want[index] {
			t.Fatalf("scheduler lease projection request[%d]=%q, want %q (pair must precede lease POST)", index, got[index], want[index])
		}
	}
}

func assertSchedulerLeaseProjectionReadOnlyPair(t *testing.T, got []string) {
	t.Helper()
	want := []string{
		http.MethodGet + " " + clientInstanceSessionViewCandidatePath,
		http.MethodGet + " " + clientInstanceResourceViewCandidatePath,
	}
	if len(got) != len(want) || got[0] != want[0] || got[1] != want[1] {
		t.Fatalf("hidden scheduler lease projection requests=%#v, want only pair reads %#v", got, want)
	}
}

func runForgeConsoleClientInstanceSchedulerLeaseProjectionE2EWithToken(
	t *testing.T,
	apiURL, token string,
	owner deviceplacement.Owner,
	request schedulerSelectionLeaseRequest,
	idempotencyKey, expectedDeviceID, expectedInstanceID, hiddenConversationID string,
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
		t.Fatalf("Flutter is required for client-instance scheduler lease projection E2E: %v", err)
	}
	inputJSON, err := json.Marshal(struct {
		APIURL               string                         `json:"api_url"`
		AccessToken          string                         `json:"access_token"`
		Owner                deviceplacement.Owner          `json:"owner"`
		Request              schedulerSelectionLeaseRequest `json:"request"`
		IdempotencyKey       string                         `json:"idempotency_key"`
		ExpectedDeviceID     string                         `json:"expected_device_id"`
		ExpectedInstanceID   string                         `json:"expected_instance_id"`
		HiddenConversationID string                         `json:"hidden_conversation_id"`
	}{
		APIURL: apiURL, AccessToken: token, Owner: owner, Request: request,
		IdempotencyKey: idempotencyKey, ExpectedDeviceID: expectedDeviceID,
		ExpectedInstanceID: expectedInstanceID, HiddenConversationID: hiddenConversationID,
	})
	if err != nil {
		t.Fatalf("encode Flutter client-instance scheduler lease input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "client-instance-scheduler-lease-projection-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter client-instance scheduler lease input: %v", err)
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
		"test/forge_client_instance_scheduler_lease_projection_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_CLIENT_INSTANCE_SCHEDULER_LEASE_PROJECTION_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter client-instance scheduler lease projection E2E failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("Flutter client-instance scheduler lease projection output exceeded the size limit")
	}
}

func assertConsoleClientInstanceSchedulerLeaseProjectionRequests(t *testing.T, got []string) {
	t.Helper()
	leaseRequest := http.MethodPost + " " + schedulerSelectionLeasePath
	segmentStart := 0
	leaseCount := 0
	for index, request := range got {
		if request != leaseRequest {
			continue
		}
		leaseCount++
		segment := got[segmentStart:index]
		for _, required := range []string{
			http.MethodGet + " " + clientInstanceSessionViewCandidatePath,
			http.MethodGet + " " + clientInstanceResourceViewCandidatePath,
			http.MethodGet + " " + deviceInventoryReadCandidateV2Path,
		} {
			if countSchedulerLeaseProjectionRequest(segment, required) == 0 {
				t.Fatalf("Console scheduler lease claim %d omitted prerequisite %q before POST: %#v", leaseCount, required, segment)
			}
		}
		segmentStart = index + 1
	}
	if leaseCount != 3 {
		t.Fatalf("Console Web/App/Mobile scheduler lease POST count=%d, want 3; requests=%#v", leaseCount, got)
	}
	hiddenTail := got[segmentStart:]
	if countSchedulerLeaseProjectionRequest(hiddenTail, http.MethodGet+" "+clientInstanceSessionViewCandidatePath) == 0 ||
		countSchedulerLeaseProjectionRequest(hiddenTail, http.MethodGet+" "+clientInstanceResourceViewCandidatePath) == 0 ||
		countSchedulerLeaseProjectionRequest(hiddenTail, leaseRequest) != 0 {
		t.Fatalf("hidden Console instance did not stop after the read-only pair: %#v", hiddenTail)
	}
}

func countSchedulerLeaseProjectionRequest(requests []string, want string) int {
	count := 0
	for _, request := range requests {
		if request == want {
			count++
		}
	}
	return count
}

func countSchedulerLeaseProjectionRunCollectionReads(requests []string) int {
	count := 0
	for _, request := range requests {
		if strings.HasPrefix(request, http.MethodGet+" "+conversationCollectionPath+"/") &&
			strings.HasSuffix(request, "/runs") {
			count++
		}
	}
	return count
}

func mustJSON(t *testing.T, value any) []byte {
	t.Helper()
	body, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return body
}

func schedulerLeaseFreshLifecycleState(
	t *testing.T,
	owner deviceidentity.Owner,
	deviceID, instanceID string,
	sequence uint64,
	now int64,
) deviceinventory.PersistedEnrollmentHeartbeatLifecycleState {
	t.Helper()
	state := lifecycleRegistrySourceState(t, owner, deviceID, instanceID, sequence)
	observed := uint64(now - 1_000)
	expires := uint64(now + 59_000)
	state.Heartbeat.Instance.ServerObservedAtMS = observed
	state.Heartbeat.Instance.CapabilityLeaseExpiresAtMS = expires
	state.Inventory.Runner.ServerObservedAtMS = observed
	state.Inventory.Runner.CapabilityLeaseExpiresAtMS = expires
	return state
}

func schedulerLeasePolicy(
	state deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
) deviceplacement.PlacementPolicy {
	return deviceplacement.PlacementPolicy{
		DeviceID: state.Device.DeviceID, InstanceID: state.Heartbeat.Instance.InstanceID,
		Revision: state.Revision, Generation: state.Heartbeat.Instance.Generation,
		HeartbeatSequence:  state.Heartbeat.Instance.HeartbeatSequence,
		DataResidencyZones: []string{"us-west"}, TrustZone: "standard",
		SandboxLevels: []string{"container"}, ConcurrencyLimit: 4, ActiveConcurrency: 0,
	}
}

func assertSchedulerLeaseE2EReceipt(
	t *testing.T,
	value schedulerSelectionLeaseResponse,
	owner deviceidentity.Owner,
	request schedulerSelectionLeaseRequest,
	deviceID, instanceID string,
	replayed bool,
) {
	t.Helper()
	if value.SchemaVersion != executionlease.RegistrySchemaVersion ||
		value.EvaluationMode != executionlease.RegistryEvaluationMode ||
		value.Owner != (deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) ||
		value.ConversationID != request.ConversationID || value.RunID != request.RunID ||
		value.AttemptID != request.AttemptID || value.DeviceID != deviceID || value.InstanceID != instanceID ||
		value.Grant.AttemptID != request.AttemptID || value.Grant.TargetID != instanceID ||
		value.Replayed != replayed || !value.Authority.PlacementSelected ||
		!value.Authority.ReservationCreated || !value.Authority.LeaseIssued ||
		value.Authority.ExecutionAuthorized || value.Authority.DispatchPerformed || value.Authority.AuditPublished {
		t.Fatalf("scheduler lease receipt=%#v", value)
	}
	if err := value.Grant.Validate(); err != nil {
		t.Fatalf("scheduler lease grant invalid: %v", err)
	}
}

func assertSchedulerLeaseReleaseE2EReceipt(
	t *testing.T,
	value schedulerSelectionLeaseReleaseResponse,
	owner deviceidentity.Owner,
	request schedulerSelectionLeaseReleaseRequest,
	expectedDeviceID, expectedInstanceID string,
	replayed bool,
) {
	t.Helper()
	if value.SchemaVersion != executionlease.RegistrySchemaVersion ||
		value.EvaluationMode != executionlease.RegistryReleaseEvaluationMode ||
		value.Owner != (deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) ||
		value.ConversationID != request.ConversationID || value.RunID != request.RunID ||
		value.AttemptID != request.AttemptID || value.DeviceID != expectedDeviceID ||
		value.InstanceID != expectedInstanceID || value.Epoch != request.Epoch ||
		value.Replayed != replayed || value.ReleasedAtMS == 0 ||
		value.Authority != (schedulerSelectionLeaseAuthority{}) {
		t.Fatalf("scheduler lease release receipt=%#v request=%#v", value, request)
	}
}

func runForgeRuntimeSchedulerLeaseRemoteCLI(
	t *testing.T,
	executable, apiURL, accessToken string,
	body []byte,
	owner deviceidentity.Owner,
	idempotencyKey string,
	expectedDeviceID, expectedInstanceID string,
	expectedReplayed bool,
) {
	t.Helper()
	inputPath := filepath.Join(t.TempDir(), "scheduler-selection-lease-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatalf("write scheduler lease request for Runtime CLI: %v", err)
	}
	output, stderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "--idempotency-key", idempotencyKey,
		"remote", "placement", "scheduler-lease", "--input", inputPath,
	)
	if err != nil {
		t.Fatalf("authenticated scheduler lease Rust CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var decoded schedulerSelectionLeaseResponse
	if err := json.Unmarshal([]byte(output), &decoded); err != nil {
		t.Fatalf("decode authenticated scheduler lease Rust CLI: %v stdout=%q", err, output)
	}
	request := schedulerSelectionLeaseRequest{}
	if err := json.Unmarshal(body, &request); err != nil {
		t.Fatal(err)
	}
	assertSchedulerLeaseE2EReceipt(t, decoded, owner, request, expectedDeviceID, expectedInstanceID, expectedReplayed)
}

func runForgeRuntimeSchedulerLeaseRemoteTUI(
	t *testing.T,
	executable, apiURL, accessToken string,
	body []byte,
	owner deviceidentity.Owner,
	expectedDeviceID, expectedInstanceID string,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("scheduler lease TUI E2E requires script: %v", err)
	}
	inputPath := filepath.Join(t.TempDir(), "scheduler-selection-lease-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatalf("write scheduler lease request for Runtime TUI: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("scheduler-selection-lease --input " + inputPath + "\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated scheduler lease Rust TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("authenticated scheduler lease Rust TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"scheduler lease [forge.execution-lease-registry/v1]",
		"target=" + expectedDeviceID + "/" + expectedInstanceID,
		"lease_issued=true",
		"fencing token withheld",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("authenticated scheduler lease Rust TUI output omitted %q: %q", want, output)
		}
	}
	if strings.Contains(output, "fence-") || strings.Contains(output, "token-") {
		t.Fatalf("authenticated scheduler lease Rust TUI leaked fencing material: %q", output)
	}
	_ = owner // The human TUI intentionally omits owner validation details.
}

// runForgeRuntimeSchedulerLeaseRemoteTUIWithClientInstance drives the
// authenticated interactive TUI through its owner-scoped session/resource
// pair.  The caller decides whether the selected instance should be allowed
// to post the lease; hidden-instance cases still return the human rejection
// text so the request-order assertion can prove that no POST escaped.
func runForgeRuntimeSchedulerLeaseRemoteTUIWithClientInstance(
	t *testing.T,
	executable, apiURL, accessToken, inputPath, instanceID, expectedDeviceID, expectedInstanceID string,
	expectReceipt bool,
) string {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("client-instance scheduler lease TUI E2E requires script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader(
		"client-instances session-view\n" +
			"client-instances resource-view\n" +
			"instance " + instanceID + "\n" +
			"scheduler-selection-lease --input " + inputPath + "\nquit\n",
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("client-instance scheduler lease Rust TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("client-instance scheduler lease Rust TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	if expectReceipt {
		for _, want := range []string{
			"Client-instance filter set to \"" + instanceID + "\"",
			"scheduler lease [forge.execution-lease-registry/v1]",
			"target=" + expectedDeviceID + "/" + expectedInstanceID,
			"lease_issued=true",
			"fencing token withheld",
		} {
			if !strings.Contains(output, want) {
				t.Fatalf("client-instance scheduler lease Rust TUI output omitted %q: %q", want, output)
			}
		}
		if strings.Contains(output, "fence-") || strings.Contains(output, "token-") {
			t.Fatalf("client-instance scheduler lease Rust TUI leaked fencing material: %q", output)
		}
	}
	return output
}

func runForgeRuntimeSchedulerLeaseRenewalRemoteCLI(
	t *testing.T,
	executable, apiURL, accessToken string,
	prior schedulerSelectionLeaseResponse,
	owner deviceidentity.Owner,
) schedulerSelectionLeaseResponse {
	t.Helper()
	renewal := schedulerSelectionLeaseRenewalRequest{
		ConversationID: prior.ConversationID, RunID: prior.RunID, AttemptID: prior.AttemptID,
		TargetID: prior.Grant.TargetID, Epoch: prior.Grant.Epoch,
		FencingToken: prior.Grant.FencingToken, TTLMS: 30_000,
	}
	body, err := json.Marshal(renewal)
	if err != nil {
		t.Fatal(err)
	}
	inputPath := filepath.Join(t.TempDir(), "scheduler-selection-lease-renewal-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatalf("write scheduler lease renewal request for Runtime CLI: %v", err)
	}
	const key = "scheduler-lease-renew-e2e-key-00000001"
	run := func() schedulerSelectionLeaseResponse {
		output, stderr, err := runForgeRuntimeCLI(
			t, executable, apiURL, accessToken, t.TempDir(),
			"--json", "--idempotency-key", key,
			"remote", "placement", "scheduler-lease-renew", "--input", inputPath,
		)
		if err != nil {
			t.Fatalf("authenticated scheduler lease renewal Rust CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
		}
		var decoded schedulerSelectionLeaseResponse
		if err := json.Unmarshal([]byte(output), &decoded); err != nil {
			t.Fatalf("decode authenticated scheduler lease renewal Rust CLI: %v stdout=%q", err, output)
		}
		return decoded
	}
	first := run()
	if first.Owner != prior.Owner || first.ConversationID != prior.ConversationID ||
		first.RunID != prior.RunID || first.AttemptID != prior.AttemptID ||
		first.DeviceID != prior.DeviceID || first.InstanceID != prior.InstanceID ||
		first.Replayed || first.Grant.Epoch != prior.Grant.Epoch+1 ||
		first.Grant.FencingToken == prior.Grant.FencingToken ||
		first.Authority.ExecutionAuthorized || first.Authority.DispatchPerformed || first.Authority.AuditPublished {
		t.Fatalf("renewal Rust CLI receipt=%#v prior=%#v owner=%#v", first, prior, owner)
	}
	replay := run()
	if !replay.Replayed || replay.Grant != first.Grant {
		t.Fatalf("renewal Rust CLI replay=%#v first=%#v", replay, first)
	}
	return first
}

func runForgeConsoleSchedulerSelectionLeaseE2EWithToken(
	t *testing.T,
	apiURL, token string,
	owner deviceidentity.Owner,
	request schedulerSelectionLeaseRequest,
	idempotencyKey, expectedDeviceID, expectedInstanceID string,
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
		t.Fatalf("Flutter is required for scheduler lease E2E: %v", err)
	}
	input := struct {
		APIURL                 string                         `json:"api_url"`
		AccessToken            string                         `json:"access_token"`
		Owner                  deviceplacement.Owner          `json:"owner"`
		ClientKinds            []string                       `json:"client_kinds"`
		IdempotencyKey         string                         `json:"idempotency_key"`
		Request                schedulerSelectionLeaseRequest `json:"request"`
		ExpectedConversationID string                         `json:"expected_conversation_id"`
		ExpectedRunID          string                         `json:"expected_run_id"`
		ExpectedAttemptID      string                         `json:"expected_attempt_id"`
		ExpectedDeviceID       string                         `json:"expected_device_id"`
		ExpectedInstanceID     string                         `json:"expected_instance_id"`
	}{
		APIURL: apiURL, AccessToken: token,
		Owner:       deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		ClientKinds: []string{"web", "app", "mobile"}, IdempotencyKey: idempotencyKey, Request: request,
		ExpectedConversationID: request.ConversationID, ExpectedRunID: request.RunID,
		ExpectedAttemptID: request.AttemptID, ExpectedDeviceID: expectedDeviceID, ExpectedInstanceID: expectedInstanceID,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter scheduler lease input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "scheduler-selection-lease-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter scheduler lease input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub", "test/forge_scheduler_selection_lease_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_SCHEDULER_SELECTION_LEASE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter scheduler lease E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("scheduler lease Flutter output exceeded the size limit")
	}
}

func runForgeConsoleSchedulerSelectionLeaseGateE2EWithToken(
	t *testing.T,
	apiURL, token string,
	owner deviceidentity.Owner,
	request schedulerSelectionLeaseRequest,
	idempotencyKey, expectedDeviceID, expectedInstanceID string,
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
		t.Fatalf("Flutter is required for scheduler lease Gate E2E: %v", err)
	}
	input := struct {
		APIURL         string                         `json:"api_url"`
		AccessToken    string                         `json:"access_token"`
		Owner          deviceplacement.Owner          `json:"owner"`
		ClientKinds    []string                       `json:"client_kinds"`
		IdempotencyKey string                         `json:"idempotency_key"`
		Request        schedulerSelectionLeaseRequest `json:"request"`
		ExpectedDevice string                         `json:"expected_device_id"`
		ExpectedRunner string                         `json:"expected_instance_id"`
	}{
		APIURL: apiURL, AccessToken: token,
		Owner:       deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		ClientKinds: []string{"web", "app", "mobile"}, IdempotencyKey: idempotencyKey,
		Request: request, ExpectedDevice: expectedDeviceID, ExpectedRunner: expectedInstanceID,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter scheduler lease Gate input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "scheduler-selection-lease-gate-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter scheduler lease Gate input: %v", err)
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
		"test/forge_scheduler_selection_lease_gate_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_SCHEDULER_SELECTION_LEASE_GATE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter scheduler lease Gate E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("scheduler lease Gate Flutter output exceeded the size limit")
	}
}
