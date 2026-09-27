//go:build linux && !android

package appserver

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestRunAcceptedInventoryActivationMountsOwnerScopedDeviceRoute(t *testing.T) {
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)

	root := privateTestParent(t)
	stateDir := filepath.Join(root, "app-state-activated")
	runtimeStateDir := filepath.Join(root, "runtime-state-activated")
	if err := os.Mkdir(runtimeStateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	runtimeExecutable := filepath.Join(root, "forge-runtime-activated")
	if configuredRuntime := os.Getenv("FORGE_RUNTIME_BIN"); configuredRuntime != "" {
		runtimeExecutable = configuredRuntime
		initializeRuntimeHubForIntegration(t, runtimeExecutable, runtimeStateDir)
	} else if err := os.WriteFile(runtimeExecutable, []byte("test runtime"), 0o700); err != nil {
		t.Fatal(err)
	}
	owner := deviceidentity.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	first := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)
	second := lifecycleRegistrySourceState(t, owner, "device-b", "runner-b", 1)
	second.Inventory.Runner.Liveness = "offline"
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{second, first})
	clientInstancePath := writeClientInstanceSessionViewSourceFile(t, owner)
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
		DeviceFabricActivation:               ptrDeviceFabricRequest(acceptedInventoryActivation()),
		DeviceInventoryLifecycleRegistryFile: registryPath,
		DeviceClientInstanceSessionViewFile:  clientInstancePath,
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
		t.Fatalf("activated server failed before readiness: %v", err)
	case <-time.After(5 * time.Second):
		t.Fatal("activated server did not announce readiness")
	}
	t.Cleanup(func() {
		cancel()
		if err := waitResult(t, result); err != nil {
			t.Error(err)
		}
	})

	request, err := http.NewRequest(http.MethodGet, ready.Listen+deviceInventoryReadCandidatePath, nil)
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Authorization", "Bearer "+token)
	response, err := (&http.Client{Timeout: 5 * time.Second}).Do(request)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	body, err := io.ReadAll(response.Body)
	if err != nil {
		t.Fatal(err)
	}
	if response.StatusCode != http.StatusOK {
		t.Fatalf("activated production inventory status=%d body=%q", response.StatusCode, body)
	}
	var inventory deviceplacement.SessionDeviceObservationInventory
	if err := json.Unmarshal(body, &inventory); err != nil {
		t.Fatalf("decode activated production inventory: %v body=%q", err, body)
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventory(inventory); err != nil {
		t.Fatalf("validate activated production inventory: %v", err)
	}
	if inventory.Owner != (deviceplacement.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}) ||
		len(inventory.Devices) != 2 || inventory.ExecutionAuthorized || inventory.ReservationCreated || inventory.DispatchPerformed {
		t.Fatalf("activated production inventory=%#v", inventory)
	}

	request, err = http.NewRequest(http.MethodGet, ready.Listen+lifecycleRegistryCandidatePath, nil)
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Authorization", "Bearer "+token)
	response, err = (&http.Client{Timeout: 5 * time.Second}).Do(request)
	if err != nil {
		t.Fatal(err)
	}
	body, err = io.ReadAll(response.Body)
	_ = response.Body.Close()
	if err != nil {
		t.Fatal(err)
	}
	if response.StatusCode != http.StatusOK {
		t.Fatalf("activated production lifecycle registry status=%d body=%q", response.StatusCode, body)
	}
	var lifecycle lifecycleRegistryCandidateEnvelope
	if err := json.Unmarshal(body, &lifecycle); err != nil {
		t.Fatalf("decode activated production lifecycle registry: %v", err)
	}
	if lifecycle.Owner.Issuer != issuer || lifecycle.Owner.Subject != snaplinkForgeTestUser ||
		lifecycle.Owner.TenantID != snaplinkForgeTestTenant || len(lifecycle.States) != 2 {
		t.Fatalf("activated production lifecycle registry=%#v", lifecycle)
	}

	var inventoryV2 deviceplacement.SessionDeviceObservationInventoryV2
	request, err = http.NewRequest(http.MethodGet, ready.Listen+deviceInventoryReadCandidateV2Path, nil)
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Authorization", "Bearer "+token)
	response, err = (&http.Client{Timeout: 5 * time.Second}).Do(request)
	if err != nil {
		t.Fatal(err)
	}
	body, err = io.ReadAll(response.Body)
	_ = response.Body.Close()
	if err != nil {
		t.Fatal(err)
	}
	if response.StatusCode != http.StatusOK {
		t.Fatalf("activated production inventory v2 status=%d body=%q", response.StatusCode, body)
	}
	if err := json.Unmarshal(body, &inventoryV2); err != nil {
		t.Fatalf("decode activated production inventory v2: %v body=%q", err, body)
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventoryV2(inventoryV2); err != nil {
		t.Fatalf("validate activated production inventory v2: %v", err)
	}
	if inventoryV2.Owner.Subject != snaplinkForgeTestUser || len(inventoryV2.Devices) != 2 ||
		inventoryV2.ExecutionAuthorized || inventoryV2.ReservationCreated || inventoryV2.DispatchPerformed {
		t.Fatalf("activated production inventory v2=%#v", inventoryV2)
	}

	request, err = http.NewRequest(
		http.MethodPost, ready.Listen+devicePlacementRegistryCandidatePath,
		strings.NewReader(registryPlacementRequirementsBody(t)),
	)
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Authorization", "Bearer "+token)
	request.Header.Set("Content-Type", "application/json")
	response, err = (&http.Client{Timeout: 5 * time.Second}).Do(request)
	if err != nil {
		t.Fatal(err)
	}
	body, err = io.ReadAll(response.Body)
	_ = response.Body.Close()
	if err != nil {
		t.Fatal(err)
	}
	if response.StatusCode != http.StatusOK {
		t.Fatalf("activated production registry placement status=%d body=%q", response.StatusCode, body)
	}
	var placement deviceplacement.PersistedInventoryPlacementV2Evaluation
	if err := json.Unmarshal(body, &placement); err != nil {
		t.Fatalf("decode activated production registry placement: %v body=%q", err, body)
	}
	if !validDevicePlacementRegistryCandidateResult(
		placement,
		model.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant},
		placement.EvaluatedAtMS,
	) || len(placement.Decisions) != 2 || placement.SelectedDeviceID != nil || placement.SelectedInstanceID != nil ||
		placement.Authority != (deviceplacement.PersistedInventoryPlacementBatchAuthority{}) {
		t.Fatalf("activated production registry placement=%#v", placement)
	}

	for _, path := range []string{clientInstanceSessionViewCandidatePath, clientInstanceResourceViewCandidatePath} {
		request, err := http.NewRequest(http.MethodGet, ready.Listen+path, nil)
		if err != nil {
			t.Fatal(err)
		}
		request.Header.Set("Authorization", "Bearer "+token)
		response, err := (&http.Client{Timeout: 5 * time.Second}).Do(request)
		if err != nil {
			t.Fatal(err)
		}
		body, err := io.ReadAll(response.Body)
		_ = response.Body.Close()
		if err != nil {
			t.Fatal(err)
		}
		if response.StatusCode != http.StatusOK {
			t.Fatalf("activated production client-instance path=%s status=%d body=%q", path, response.StatusCode, body)
		}
		if path == clientInstanceSessionViewCandidatePath {
			var view deviceplacement.ClientInstanceSessionViewObservation
			if err := json.Unmarshal(body, &view); err != nil {
				t.Fatalf("decode activated production session view: %v", err)
			}
			if err := view.Validate(); err != nil || len(view.Instances) != 5 || view.Owner.Subject != snaplinkForgeTestUser {
				t.Fatalf("activated production session view=%#v err=%v", view, err)
			}
			continue
		}
		var view deviceplacement.ClientInstanceResourceViewObservation
		if err := json.Unmarshal(body, &view); err != nil {
			t.Fatalf("decode activated production resource view: %v", err)
		}
		if err := view.Validate(); err != nil || len(view.Instances) != 5 || len(view.Devices) != 2 || view.Authority.ExecutionAuthorized || view.Authority.DispatchPerformed {
			t.Fatalf("activated production resource view=%#v err=%v", view, err)
		}
	}

	var acceptedFirstConversationID, acceptedSecondConversationID string
	if executable := os.Getenv("FORGE_RUNTIME_BIN"); executable != "" {
		activatedInventoryV2 := inventoryV2
		activatedInventoryV2.EvaluatedAtMS = 0
		runForgeRuntimeDeviceInventoryV2CandidateE2EWithToken(t, ready.Listen, token, activatedInventoryV2)
		runForgeRuntimeLifecycleRegistryRemoteCLI(t, executable, ready.Listen, token, issuer)
		runForgeRuntimeLifecycleRegistryRemoteTUI(t, executable, ready.Listen, token, issuer)
		runForgeRuntimeRegistryPlacementRemoteCLI(
			t,
			executable,
			ready.Listen,
			token,
			model.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant},
		)
		runForgeRuntimeClientInstanceSessionViewRemoteCLI(t, executable, ready.Listen, token)
		runForgeRuntimeClientInstanceResourceViewRemoteCLI(t, executable, ready.Listen, token)
	}
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		runForgeConsoleClientInstanceSessionViewE2EWithToken(
			t, ready.Listen, token, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant,
		)
		runForgeConsoleClientInstanceResourceViewE2EWithGenericResources(
			t, ready.Listen, token, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant,
		)
		runForgeConsoleLifecycleRegistryE2EWithToken(
			t, ready.Listen, token, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant,
		)
		runForgeConsoleRegistryPlacementE2EWithToken(
			t, ready.Listen, token, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant,
		)
		runForgeConsoleAcceptedDeviceInventoryE2EWithToken(
			t, ready.Listen, token, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant,
		)
		runForgeConsoleAcceptedDeviceInventoryGateE2EWithToken(
			t, ready.Listen, token, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant,
		)
		if executable := os.Getenv("FORGE_RUNTIME_BIN"); executable != "" {
			httpClient := &http.Client{Timeout: 20 * time.Second}
			first := createSharedConversationAsClientA(t, httpClient, ready.Listen, token)
			second := createProjectionConversation(t, httpClient, ready.Listen, token)
			acceptedFirstConversationID = first.ID
			acceptedSecondConversationID = second.ID
			writeClientInstanceSessionViewSourceFileAtPath(t, clientInstancePath, owner, []deviceplacement.ClientInstanceSessionViewInstance{
				{InstanceID: "client-cli-001", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{first.ID, second.ID}, ObservedAtMS: 200500, Status: "active"},
				{InstanceID: "client-tui-001", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{second.ID}, ObservedAtMS: 200500, Status: "active"},
				{InstanceID: "client-web-001", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{first.ID}, ObservedAtMS: 200500, Status: "active"},
				{InstanceID: "client-app-001", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{first.ID}, ObservedAtMS: 200500, Status: "active"},
				{InstanceID: "client-mobile-001", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{second.ID}, ObservedAtMS: 200500, Status: "active"},
			})
			productionOwner := deviceplacement.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
			for _, projection := range []struct {
				name       string
				instanceID string
				visible    model.Conversation
				hidden     model.Conversation
				prompt     string
			}{
				{name: "web", instanceID: "client-web-001", visible: first, hidden: second, prompt: "Prompt from accepted production Web instance"},
				{name: "app", instanceID: "client-app-001", visible: first, hidden: second, prompt: "Prompt from accepted production desktop App instance"},
				{name: "mobile", instanceID: "client-mobile-001", visible: second, hidden: first, prompt: "Prompt from accepted production Mobile instance"},
			} {
				t.Run("production_"+projection.name+"_prompt", func(t *testing.T) {
					runForgeConsoleClientInstanceSessionProjectionE2EWithToken(
						t, ready.Listen, token, productionOwner, projection.instanceID,
						projection.visible, projection.hidden, projection.prompt,
					)
				})
			}
			tuiOutput := runForgeRuntimeClientInstanceProjectionTUI(t, executable, ready.Listen, token, second.ID)
			if !strings.Contains(tuiOutput, `Client-instance filter set to "client-tui-001"`) ||
				!strings.Contains(tuiOutput, "Prompt stored. No Run was started.") ||
				!strings.Contains(tuiOutput, "Prompt submitted from client-instance TUI") {
				t.Fatalf("accepted production TUI omitted instance filter or Prompt receipt: %q", tuiOutput)
			}
			cliSessionsOutput, cliSessionsStderr, err := runForgeRuntimeCLI(
				t, executable, ready.Listen, token, t.TempDir(),
				"--json", "remote", "sessions", "list", "--instance", "client-cli-001",
			)
			if err != nil {
				t.Fatalf("accepted production CLI instance session read failed: stderr=%q stdout=%q err=%v", cliSessionsStderr, cliSessionsOutput, err)
			}
			var cliSessions model.OwnedConversationPage
			if err := json.Unmarshal([]byte(cliSessionsOutput), &cliSessions); err != nil {
				t.Fatalf("decode accepted production CLI instance sessions: %v stdout=%q", err, cliSessionsOutput)
			}
			var cliExpectedVersion uint64
			for _, entry := range cliSessions.Conversations {
				if entry.Conversation.ID == first.ID {
					cliExpectedVersion = entry.AggregateVersion
					break
				}
			}
			if cliExpectedVersion == 0 {
				t.Fatalf("accepted production CLI instance session page omitted %q: %#v", first.ID, cliSessions)
			}
			cliOutput, cliStderr, err := runForgeRuntimeCLI(
				t, executable, ready.Listen, token, t.TempDir(),
				"--json", "--idempotency-key", "accepted-production-cli-prompt",
				"remote", "prompts", "add", first.ID, "--expected-version", strconv.FormatUint(cliExpectedVersion, 10),
				"--instance", "client-cli-001", "Prompt from accepted production CLI instance",
			)
			if err != nil || !strings.Contains(cliOutput, "Prompt from accepted production CLI instance") {
				t.Fatalf("accepted production CLI Prompt failed: stderr=%q stdout=%q err=%v", cliStderr, cliOutput, err)
			}
			for _, expected := range []struct {
				conversationID string
				prompt         string
			}{
				{conversationID: first.ID, prompt: "Prompt from accepted production Web instance"},
				{conversationID: first.ID, prompt: "Prompt from accepted production desktop App instance"},
				{conversationID: first.ID, prompt: "Prompt from accepted production CLI instance"},
				{conversationID: second.ID, prompt: "Prompt from accepted production Mobile instance"},
				{conversationID: second.ID, prompt: "Prompt submitted from client-instance TUI"},
			} {
				output, stderr, err := runForgeRuntimeCLI(
					t, executable, ready.Listen, token, t.TempDir(),
					"--json", "remote", "prompts", "list", expected.conversationID,
				)
				if err != nil {
					t.Fatalf("accepted production Prompt read failed: stderr=%q stdout=%q err=%v", stderr, output, err)
				}
				var history model.ConversationPromptPage
				if err := json.Unmarshal([]byte(output), &history); err != nil ||
					history.ConversationID != expected.conversationID || !promptPageContains(history, expected.prompt) {
					t.Fatalf("accepted production Prompt history=%#v stdout=%q decode=%v", history, output, err)
				}
			}
			liveTUIOutput := runForgeRuntimeClientInstanceLiveRefreshPromptE2EWithToken(
				t, executable, ready.Listen, token, clientInstancePath, owner, second.ID,
			)
			// Restore the canonical five-instance image before the later Gate
			// refresh starts its own before/after assertion.
			writeClientInstanceSessionViewSourceFileAtPath(t, clientInstancePath, owner, []deviceplacement.ClientInstanceSessionViewInstance{
				{InstanceID: "client-cli-001", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{first.ID, second.ID}, ObservedAtMS: 200500, Status: "active"},
				{InstanceID: "client-tui-001", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{second.ID}, ObservedAtMS: 200500, Status: "active"},
				{InstanceID: "client-web-001", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{first.ID}, ObservedAtMS: 200500, Status: "active"},
				{InstanceID: "client-app-001", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{first.ID}, ObservedAtMS: 200500, Status: "active"},
				{InstanceID: "client-mobile-001", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{second.ID}, ObservedAtMS: 200500, Status: "idle"},
			})
			for _, want := range []string{
				"Selected client-instance/session-view refreshed.",
				`Client-instance filter set to "client-tui-live-002"`,
				"Opened session",
				"Prompt stored. No Run was started.",
				"Prompt submitted after client-instance refresh",
			} {
				if !strings.Contains(liveTUIOutput, want) {
					t.Fatalf("accepted production live client-instance TUI omitted %q: %q", want, liveTUIOutput)
				}
			}
			livePromptOutput, livePromptStderr, err := runForgeRuntimeCLI(
				t, executable, ready.Listen, token, t.TempDir(),
				"--json", "remote", "prompts", "list", second.ID,
			)
			if err != nil {
				t.Fatalf("accepted production live-refresh Prompt read failed: stderr=%q stdout=%q err=%v", livePromptStderr, livePromptOutput, err)
			}
			var livePromptHistory model.ConversationPromptPage
			if err := json.Unmarshal([]byte(livePromptOutput), &livePromptHistory); err != nil ||
				!promptPageContains(livePromptHistory, "Prompt submitted after client-instance refresh") {
				t.Fatalf("accepted production live-refresh Prompt history=%#v stdout=%q decode=%v", livePromptHistory, livePromptOutput, err)
			}
			externalTUIOutput := runForgeRuntimeClientInstanceExternalPromptConvergenceE2EWithToken(
				t, executable, ready.Listen, token, clientInstancePath, owner, second.ID,
			)
			for _, want := range []string{
				`Client-instance filter set to "client-tui-live-005"`,
				"Opened session",
				"Prompt history refreshed for the selected session.",
				"Prompt from external Runtime CLI after client-instance refresh",
			} {
				if !strings.Contains(externalTUIOutput, want) {
					t.Fatalf("accepted production external Prompt convergence omitted %q: %q", want, externalTUIOutput)
				}
			}
			writeClientInstanceSessionViewSourceFileAtPath(
				t,
				clientInstancePath,
				owner,
				clientInstanceLiveRefreshRowsForConversations("006", first.ID, second.ID),
			)
			runForgeConsoleClientInstanceExternalPromptConvergenceE2EWithToken(
				t,
				ready.Listen,
				token,
				executable,
				model.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant},
				first.ID,
				"client-web-live-006",
				"Prompt from external Runtime CLI through the Console Gate",
			)
			writeClientInstanceSessionViewSourceFileAtPath(t, clientInstancePath, owner, []deviceplacement.ClientInstanceSessionViewInstance{
				{InstanceID: "client-cli-001", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{first.ID, second.ID}, ObservedAtMS: 200500, Status: "active"},
				{InstanceID: "client-tui-001", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{second.ID}, ObservedAtMS: 200500, Status: "active"},
				{InstanceID: "client-web-001", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{first.ID}, ObservedAtMS: 200500, Status: "active"},
				{InstanceID: "client-app-001", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{first.ID}, ObservedAtMS: 200500, Status: "active"},
				{InstanceID: "client-mobile-001", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{second.ID}, ObservedAtMS: 200500, Status: "idle"},
			})
		}
	}
	if executable := os.Getenv("FORGE_RUNTIME_BIN"); executable != "" {
		runForgeRuntimePersistedInventoryV2TUILiveRefreshE2EWithToken(
			t, executable, ready.Listen, token, registryPath, owner,
		)
		if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
			updated := lifecycleRegistrySourceStateRevision(t, owner, "device-a", "runner-a", 3)
			second := lifecycleRegistrySourceState(t, owner, "device-b", "runner-b", 1)
			updatedPath := filepath.Join(t.TempDir(), "accepted-inventory-gate-refresh.json")
			writeLifecycleRegistrySourceFileAtPath(t, updatedPath, owner,
				[]deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{second, updated})
			updatedJSON, err := os.ReadFile(updatedPath)
			if err != nil {
				t.Fatalf("read accepted inventory Gate refresh image: %v", err)
			}
			updatedClientPath := filepath.Join(t.TempDir(), "accepted-client-instance-gate-refresh.json")
			writeClientInstanceSessionViewSourceFileAtPath(
				t,
				updatedClientPath,
				owner,
				clientInstanceLiveRefreshRowsForConversations("002", acceptedFirstConversationID, acceptedSecondConversationID),
			)
			updatedClientJSON, err := os.ReadFile(updatedClientPath)
			if err != nil {
				t.Fatalf("read accepted client-instance Gate refresh image: %v", err)
			}
			runForgeConsoleAcceptedDeviceInventoryGateE2EWithToken(
				t, ready.Listen, token, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant,
				acceptedInventoryGateRefreshInput{
					registryPath:                   registryPath,
					updatedJSON:                    string(updatedJSON),
					marker:                         "Persisted counters: revision 3 · generation 1 · heartbeat 3",
					clientInstancePath:             clientInstancePath,
					updatedClientInstanceJSON:      string(updatedClientJSON),
					expectedResourceRefreshMarker:  "state: revision=3 · generation=1 · heartbeat=3",
					expectedResourceInstanceMarker: "client-web-live-002",
					promptConversationID:           acceptedSecondConversationID,
					promptInstanceBefore:           "client-tui-001",
					promptInstanceAfter:            "client-tui-live-002",
					promptContent:                  "Prompt from Console after client-instance refresh",
				},
			)
			runForgeConsoleInventoryRefreshConvergenceE2EWithToken(
				t, ready.Listen, token, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant,
				3, 1, 3,
			)
			promptOutput, promptStderr, err := runForgeRuntimeCLI(
				t, executable, ready.Listen, token, t.TempDir(),
				"--json", "remote", "prompts", "list", acceptedSecondConversationID,
			)
			if err != nil {
				t.Fatalf("accepted production Console live-refresh Prompt read failed: stderr=%q stdout=%q err=%v", promptStderr, promptOutput, err)
			}
			var promptHistory model.ConversationPromptPage
			if err := json.Unmarshal([]byte(promptOutput), &promptHistory); err != nil ||
				!promptPageContains(promptHistory, "Prompt from Console after client-instance refresh") {
				t.Fatalf("accepted production Console live-refresh Prompt history=%#v stdout=%q decode=%v", promptHistory, promptOutput, err)
			}
		}
		runForgeRuntimeClientInstanceViewsTUILiveRefreshE2EWithToken(
			t, executable, ready.Listen, token, clientInstancePath, owner,
		)
		runForgeRuntimeClientInstanceViewsCLIRefreshE2EWithToken(
			t, executable, ready.Listen, token, clientInstancePath, owner,
		)
		if acceptedFirstConversationID != "" && acceptedSecondConversationID != "" {
			runForgeRuntimeClientInstanceCLIPromptAfterRefreshE2EWithToken(
				t,
				executable,
				ready.Listen,
				token,
				clientInstancePath,
				owner,
				acceptedFirstConversationID,
			)
		}
	}
}

func TestRunAcceptedObserveActivationMountsReadOnlyDeviceRoutes(t *testing.T) {
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)

	root := privateTestParent(t)
	stateDir := filepath.Join(root, "app-state-observe-activated")
	runtimeStateDir := filepath.Join(root, "runtime-state-observe-activated")
	if err := os.Mkdir(runtimeStateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	runtimeExecutable := filepath.Join(root, "forge-runtime-observe-activated")
	if configuredRuntime := os.Getenv("FORGE_RUNTIME_BIN"); configuredRuntime != "" {
		runtimeExecutable = configuredRuntime
		initializeRuntimeHubForIntegration(t, runtimeExecutable, runtimeStateDir)
	} else if err := os.WriteFile(runtimeExecutable, []byte("test runtime"), 0o700); err != nil {
		t.Fatal(err)
	}
	owner := deviceidentity.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{
		lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1),
	})
	clientInstancePath := writeClientInstanceSessionViewSourceFile(t, owner)
	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeObserve
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
		t.Fatalf("accepted observe server failed before readiness: %v", err)
	case <-time.After(5 * time.Second):
		t.Fatal("accepted observe server did not announce readiness")
	}
	t.Cleanup(func() {
		cancel()
		if err := waitResult(t, result); err != nil {
			t.Error(err)
		}
	})

	client := &http.Client{Timeout: 5 * time.Second}
	request := func(method, path, body string) (int, []byte) {
		t.Helper()
		var reader io.Reader
		if body != "" {
			reader = strings.NewReader(body)
		}
		req, err := http.NewRequest(method, ready.Listen+path, reader)
		if err != nil {
			t.Fatal(err)
		}
		req.Header.Set("Authorization", "Bearer "+token)
		if body != "" {
			req.Header.Set("Content-Type", "application/json")
		}
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

	status, body := request(http.MethodGet, lifecycleRegistryCandidatePath, "")
	if status != http.StatusOK {
		t.Fatalf("accepted observe lifecycle status=%d body=%q", status, body)
	}
	var lifecycle lifecycleRegistryCandidateEnvelope
	if err := json.Unmarshal(body, &lifecycle); err != nil {
		t.Fatalf("decode accepted observe lifecycle: %v", err)
	}
	if lifecycle.Owner.Subject != snaplinkForgeTestUser || len(lifecycle.States) != 1 {
		t.Fatalf("accepted observe lifecycle=%#v", lifecycle)
	}

	status, body = request(http.MethodGet, deviceInventoryReadCandidatePath, "")
	if status != http.StatusOK {
		t.Fatalf("accepted observe v1 status=%d body=%q", status, body)
	}
	var inventory deviceplacement.SessionDeviceObservationInventory
	if err := json.Unmarshal(body, &inventory); err != nil {
		t.Fatalf("decode accepted observe v1: %v", err)
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventory(inventory); err != nil {
		t.Fatalf("validate accepted observe v1: %v", err)
	}
	if len(inventory.Devices) != 1 || inventory.Owner.Subject != snaplinkForgeTestUser ||
		inventory.ExecutionAuthorized || inventory.ReservationCreated || inventory.DispatchPerformed {
		t.Fatalf("accepted observe v1=%#v", inventory)
	}

	status, body = request(http.MethodGet, deviceInventoryReadCandidateV2Path, "")
	if status != http.StatusOK {
		t.Fatalf("accepted observe v2 status=%d body=%q", status, body)
	}
	var inventoryV2 deviceplacement.SessionDeviceObservationInventoryV2
	if err := json.Unmarshal(body, &inventoryV2); err != nil {
		t.Fatalf("decode accepted observe v2: %v", err)
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventoryV2(inventoryV2); err != nil {
		t.Fatalf("validate accepted observe v2: %v", err)
	}
	if len(inventoryV2.Devices) != 1 || inventoryV2.Owner.Subject != snaplinkForgeTestUser ||
		inventoryV2.ExecutionAuthorized || inventoryV2.ReservationCreated || inventoryV2.DispatchPerformed {
		t.Fatalf("accepted observe v2=%#v", inventoryV2)
	}

	status, body = request(http.MethodPost, devicePlacementRegistryCandidatePath, registryPlacementRequirementsBody(t))
	if status != http.StatusOK {
		t.Fatalf("accepted observe placement status=%d body=%q", status, body)
	}
	var placement deviceplacement.PersistedInventoryPlacementV2Evaluation
	if err := json.Unmarshal(body, &placement); err != nil {
		t.Fatalf("decode accepted observe placement: %v", err)
	}
	if !validDevicePlacementRegistryCandidateResult(
		placement,
		model.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant},
		placement.EvaluatedAtMS,
	) || placement.SelectedDeviceID != nil || placement.SelectedInstanceID != nil ||
		placement.Authority != (deviceplacement.PersistedInventoryPlacementBatchAuthority{}) {
		t.Fatalf("accepted observe placement=%#v", placement)
	}

	status, body = request(http.MethodGet, clientInstanceSessionViewCandidatePath, "")
	if status != http.StatusOK {
		t.Fatalf("accepted observe client-instance session status=%d body=%q", status, body)
	}
	var sessionView deviceplacement.ClientInstanceSessionViewObservation
	if err := json.Unmarshal(body, &sessionView); err != nil {
		t.Fatalf("decode accepted observe client-instance session: %v", err)
	}
	if err := sessionView.Validate(); err != nil || len(sessionView.Instances) != 5 || !sessionView.ReadOnly || sessionView.Owner.Subject != snaplinkForgeTestUser {
		t.Fatalf("accepted observe client-instance session=%#v err=%v", sessionView, err)
	}

	status, body = request(http.MethodGet, clientInstanceResourceViewCandidatePath, "")
	if status != http.StatusOK {
		t.Fatalf("accepted observe client-instance resource status=%d body=%q", status, body)
	}
	var resourceView deviceplacement.ClientInstanceResourceViewObservation
	if err := json.Unmarshal(body, &resourceView); err != nil {
		t.Fatalf("decode accepted observe client-instance resource: %v", err)
	}
	if err := resourceView.Validate(); err != nil || len(resourceView.Instances) != 5 || len(resourceView.Devices) != 1 || !resourceView.ReadOnly || resourceView.Authority != (deviceplacement.ClientInstanceResourceViewAuthority{}) {
		t.Fatalf("accepted observe client-instance resource=%#v err=%v", resourceView, err)
	}

	status, body = request(http.MethodPut, lifecycleRegistryCandidatePath, `{"states":[]}`)
	if status != http.StatusNotFound || string(body) != string(notFoundBody) {
		t.Fatalf("accepted observe lifecycle mutation status=%d body=%q", status, body)
	}
}

func TestRunAcceptedExecuteActivationMountsAdmissionRoutes(t *testing.T) {
	if os.Getenv("FORGE_RUNTIME_BIN") == "" {
		t.Skip("accepted execute scheduler preview E2E requires a configured Forge Runtime binary for durable Run binding")
	}
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)

	root := privateTestParent(t)
	stateDir := filepath.Join(root, "app-state-execute-admission")
	runtimeStateDir := filepath.Join(root, "runtime-state-execute-admission")
	if err := os.Mkdir(runtimeStateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	runtimeExecutable := filepath.Join(root, "forge-runtime-execute-admission")
	configuredRuntime := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredRuntime != "" {
		runtimeExecutable = configuredRuntime
		initializeRuntimeHubForIntegration(t, runtimeExecutable, runtimeStateDir)
	} else if err := os.WriteFile(runtimeExecutable, []byte("test runtime"), 0o700); err != nil {
		t.Fatal(err)
	}
	owner := deviceidentity.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	schedulerConversationID, schedulerRunID := "conversation-1", "run-1"
	if configuredRuntime != "" {
		schedulerConversationID, schedulerRunID = seedSchedulerSelectionPreviewSession(
			t, runtimeExecutable, runtimeStateDir, owner,
		)
	}
	clientInstancePath := filepath.Join(root, "client-instance-session-view-scheduler-preview.json")
	writeClientInstanceSessionViewSourceFileAtPath(
		t,
		clientInstancePath,
		owner,
		[]deviceplacement.ClientInstanceSessionViewInstance{
			{InstanceID: "client-web-001", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{schedulerConversationID}, ObservedAtMS: 200500, Status: "active"},
			{InstanceID: "client-app-001", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{schedulerConversationID}, ObservedAtMS: 200500, Status: "active"},
			{InstanceID: "client-mobile-001", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{schedulerConversationID}, ObservedAtMS: 200500, Status: "active"},
		},
	)
	now := time.Now().UnixMilli()
	state := schedulerLeaseFreshLifecycleState(t, owner, "device-a", "runner-a", 1, now)
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})
	policyPath := writePlacementPolicySourceFile(t, owner, []deviceplacement.PlacementPolicy{
		schedulerLeasePolicy(state),
	})
	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{
		Status: "accepted", AcceptanceID: "p4-execute-admission-001", AcceptedAtUnixMS: 1,
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
		t.Fatalf("accepted execute admission server failed before readiness: %v", err)
	case <-time.After(5 * time.Second):
		t.Fatal("accepted execute admission server did not announce readiness")
	}
	t.Cleanup(func() {
		cancel()
		if err := waitResult(t, result); err != nil {
			t.Error(err)
		}
	})
	client := &http.Client{Timeout: 5 * time.Second}
	request := func(method, path, body string, idempotencyKey string) (int, []byte) {
		t.Helper()
		var reader io.Reader
		if body != "" {
			reader = strings.NewReader(body)
		}
		req, err := http.NewRequest(method, ready.Listen+path, reader)
		if err != nil {
			t.Fatal(err)
		}
		req.Header.Set("Authorization", "Bearer "+token)
		if body != "" {
			req.Header.Set("Content-Type", "application/json")
		}
		if idempotencyKey != "" {
			req.Header.Set("Idempotency-Key", idempotencyKey)
		}
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
	status, body := request(http.MethodGet, lifecycleRegistryCandidatePath, "", "")
	if status != http.StatusOK {
		t.Fatalf("accepted execute lifecycle status=%d body=%q", status, body)
	}
	var placementRequest devicePlacementRegistryCandidateRequest
	if err := json.Unmarshal([]byte(registryPlacementRequirementsBody(t)), &placementRequest); err != nil {
		t.Fatalf("decode execute scheduler requirements: %v", err)
	}
	schedulerPreviewRequest := schedulerSelectionPreviewRequest{
		ConversationID: schedulerConversationID, RunID: schedulerRunID, AttemptID: "attempt-1",
		Requirements: placementRequest.Requirements,
	}
	schedulerRequest, err := json.Marshal(schedulerPreviewRequest)
	if err != nil {
		t.Fatalf("encode execute scheduler request: %v", err)
	}
	status, body = request(http.MethodPost, schedulerSelectionPreviewPath, string(schedulerRequest), "")
	if status != http.StatusOK {
		t.Fatalf("accepted execute scheduler selection status=%d body=%q", status, body)
	}
	var selection deviceplacement.SchedulerSelectionPreviewObservation
	if err := json.Unmarshal(body, &selection); err != nil {
		t.Fatalf("decode accepted execute scheduler selection: %v", err)
	}
	if err := selection.Validate(); err != nil || !selection.SelectionAvailable ||
		selection.SelectedDeviceID == nil || *selection.SelectedDeviceID != "device-a" ||
		selection.SelectedInstanceID == nil || *selection.SelectedInstanceID != "runner-a" ||
		selection.SelectionReason != "first_sorted_eligible_candidate" ||
		selection.Authority != (deviceplacement.SchedulerSelectionPreviewAuthority{}) {
		t.Fatalf("accepted execute scheduler selection=%#v err=%v", selection, err)
	}
	policyPreviewExpectation := schedulerSelectionPreviewExpectation{
		SelectionAvailable: true,
		SelectionReason:    "first_sorted_eligible_candidate",
		DeviceID:           "device-a",
		InstanceID:         "runner-a",
	}
	for _, route := range []struct {
		method string
		path   string
		body   string
		key    string
	}{
		{method: http.MethodGet, path: conversationCollectionPath + "/conversation-1/execution-consents"},
		{method: http.MethodGet, path: conversationCollectionPath + "/conversation-1/run-intents"},
		{method: http.MethodDelete, path: "/api/v1/execution-consents/grant-1", key: "execute-admission-revoke"},
		{method: http.MethodPost, path: conversationCollectionPath + "/conversation-1/runs/run-1/runner-dispatch-plan-preview", body: `{}`},
	} {
		status, body = request(route.method, route.path, route.body, route.key)
		if route.method == http.MethodPost {
			if status != http.StatusBadRequest {
				t.Fatalf("accepted execute preflight route %s %s status=%d body=%q", route.method, route.path, status, body)
			}
			continue
		}
		if status == http.StatusNotFound && string(body) == string(notFoundBody) {
			t.Fatalf("accepted execute admission route %s %s remained closed: body=%q", route.method, route.path, body)
		}
	}
	if executable := os.Getenv("FORGE_RUNTIME_BIN"); executable != "" {
		runForgeRuntimeSchedulerSelectionRemoteCLIWithRequest(
			t,
			executable,
			ready.Listen,
			token,
			model.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant},
			schedulerPreviewRequest,
			policyPreviewExpectation,
		)
		runForgeRuntimeSchedulerSelectionRemoteTUIWithRequest(
			t,
			executable,
			ready.Listen,
			token,
			model.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant},
			schedulerPreviewRequest,
			policyPreviewExpectation,
		)
	}
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		runForgeConsoleSchedulerSelectionPreviewE2EWithToken(
			t,
			ready.Listen,
			token,
			model.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant},
			placementRequest.Requirements,
			schedulerConversationID,
			schedulerRunID,
			policyPreviewExpectation,
		)
		if configuredRuntime != "" {
			runForgeConsoleSchedulerSelectionPreviewGateE2EWithToken(
				t,
				ready.Listen,
				token,
				model.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant},
				placementRequest.Requirements,
				schedulerConversationID,
				schedulerRunID,
				policyPreviewExpectation,
			)
		}
	}
}

func runForgeConsoleLifecycleRegistryE2EWithToken(
	t *testing.T, apiURL, token, issuer, subject, tenant string,
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
		t.Fatalf("Flutter is required for the lifecycle registry E2E: %v", err)
	}
	input := struct {
		APIURL      string `json:"api_url"`
		AccessToken string `json:"access_token"`
		Issuer      string `json:"issuer"`
		Subject     string `json:"subject"`
		Tenant      string `json:"tenant_id"`
	}{APIURL: apiURL, AccessToken: token, Issuer: issuer, Subject: subject, Tenant: tenant}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter lifecycle registry input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "lifecycle-registry-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter lifecycle registry input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub", "test/forge_lifecycle_registry_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_LIFECYCLE_REGISTRY_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter lifecycle registry E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("lifecycle registry Flutter output exceeded the size limit")
	}
}

func runForgeConsoleRegistryPlacementE2EWithToken(
	t *testing.T, apiURL, token, issuer, subject, tenant string,
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
		t.Fatalf("Flutter is required for registry placement E2E: %v", err)
	}
	var request devicePlacementRegistryCandidateRequest
	if err := json.Unmarshal([]byte(registryPlacementRequirementsBody(t)), &request); err != nil {
		t.Fatalf("decode registry placement requirements for Flutter: %v", err)
	}
	input := struct {
		APIURL       string                       `json:"api_url"`
		AccessToken  string                       `json:"access_token"`
		Issuer       string                       `json:"issuer"`
		Subject      string                       `json:"subject"`
		Tenant       string                       `json:"tenant_id"`
		Requirements deviceplacement.Requirements `json:"requirements"`
	}{
		APIURL: apiURL, AccessToken: token, Issuer: issuer, Subject: subject, Tenant: tenant,
		Requirements: request.Requirements,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter registry placement input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "registry-placement-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter registry placement input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub", "test/forge_registry_placement_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_REGISTRY_PLACEMENT_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter registry placement E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("registry placement Flutter output exceeded the size limit")
	}
}

func runForgeConsoleAcceptedDeviceInventoryE2EWithToken(
	t *testing.T, apiURL, token, issuer, subject, tenant string,
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
		t.Fatalf("Flutter is required for accepted device inventory E2E: %v", err)
	}
	input := struct {
		APIURL      string `json:"api_url"`
		AccessToken string `json:"access_token"`
		Issuer      string `json:"issuer"`
		Subject     string `json:"subject"`
		Tenant      string `json:"tenant_id"`
	}{APIURL: apiURL, AccessToken: token, Issuer: issuer, Subject: subject, Tenant: tenant}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode accepted device inventory input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "accepted-device-inventory-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private accepted device inventory input: %v", err)
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
		"test/forge_accepted_device_inventory_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_ACCEPTED_DEVICE_INVENTORY_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter accepted device inventory E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("accepted device inventory Flutter output exceeded the size limit")
	}
}

type acceptedInventoryGateRefreshInput struct {
	registryPath                   string
	updatedJSON                    string
	marker                         string
	clientInstancePath             string
	updatedClientInstanceJSON      string
	expectedResourceRefreshMarker  string
	expectedResourceInstanceMarker string
	promptConversationID           string
	promptInstanceBefore           string
	promptInstanceAfter            string
	promptContent                  string
}

func runForgeConsoleAcceptedDeviceInventoryGateE2EWithToken(
	t *testing.T, apiURL, token, issuer, subject, tenant string,
	refresh ...acceptedInventoryGateRefreshInput,
) {
	t.Helper()
	if len(refresh) > 1 {
		t.Fatalf("accepted inventory Gate refresh input may be supplied at most once")
	}
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
		t.Fatalf("Flutter is required for accepted device inventory Gate E2E: %v", err)
	}
	input := struct {
		APIURL                         string `json:"api_url"`
		AccessToken                    string `json:"access_token"`
		Issuer                         string `json:"issuer"`
		Subject                        string `json:"subject"`
		Tenant                         string `json:"tenant_id"`
		RegistryPath                   string `json:"registry_path,omitempty"`
		UpdatedRegistryJSON            string `json:"updated_registry_json,omitempty"`
		ExpectedMarker                 string `json:"expected_refresh_marker,omitempty"`
		ClientInstancePath             string `json:"client_instance_path,omitempty"`
		UpdatedClientJSON              string `json:"updated_client_instance_json,omitempty"`
		ExpectedResourceMarker         string `json:"expected_resource_refresh_marker,omitempty"`
		ExpectedResourceInstanceMarker string `json:"expected_resource_instance_marker,omitempty"`
		PromptConversationID           string `json:"live_prompt_conversation_id,omitempty"`
		PromptInstanceBefore           string `json:"live_prompt_instance_before,omitempty"`
		PromptInstanceAfter            string `json:"live_prompt_instance_after,omitempty"`
		PromptContent                  string `json:"live_prompt_content,omitempty"`
	}{APIURL: apiURL, AccessToken: token, Issuer: issuer, Subject: subject, Tenant: tenant}
	if len(refresh) == 1 {
		input.RegistryPath = refresh[0].registryPath
		input.UpdatedRegistryJSON = refresh[0].updatedJSON
		input.ExpectedMarker = refresh[0].marker
		input.ClientInstancePath = refresh[0].clientInstancePath
		input.UpdatedClientJSON = refresh[0].updatedClientInstanceJSON
		input.ExpectedResourceMarker = refresh[0].expectedResourceRefreshMarker
		input.ExpectedResourceInstanceMarker = refresh[0].expectedResourceInstanceMarker
		input.PromptConversationID = refresh[0].promptConversationID
		input.PromptInstanceBefore = refresh[0].promptInstanceBefore
		input.PromptInstanceAfter = refresh[0].promptInstanceAfter
		input.PromptContent = refresh[0].promptContent
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode accepted device inventory Gate input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "accepted-device-inventory-gate-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private accepted device inventory Gate input: %v", err)
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
		"test/forge_accepted_device_inventory_gate_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_ACCEPTED_DEVICE_INVENTORY_GATE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter accepted device inventory Gate E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("accepted device inventory Gate Flutter output exceeded the size limit")
	}
}

func runForgeConsoleClientInstanceExternalPromptConvergenceE2EWithToken(
	t *testing.T,
	apiURL, accessToken, runtimeExecutable string,
	owner model.Owner,
	conversationID, instanceID, content string,
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
		t.Fatalf("Flutter is required for client-instance external Prompt convergence: %v", err)
	}
	input := struct {
		APIURL            string `json:"api_url"`
		AccessToken       string `json:"access_token"`
		ConversationID    string `json:"conversation_id"`
		InstanceID        string `json:"instance_id"`
		Issuer            string `json:"issuer"`
		Subject           string `json:"subject"`
		Tenant            string `json:"tenant_id"`
		RuntimeExecutable string `json:"runtime_executable"`
		Content           string `json:"content"`
		IdempotencyKey    string `json:"idempotency_key"`
	}{
		APIURL: apiURL, AccessToken: accessToken, ConversationID: conversationID,
		InstanceID: instanceID, Issuer: owner.Issuer, Subject: owner.Subject,
		Tenant: owner.TenantID, RuntimeExecutable: runtimeExecutable, Content: content,
		IdempotencyKey: "accepted-production-console-external-prompt-006",
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode client-instance Console convergence input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "client-instance-session-convergence-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write client-instance Console convergence input: %v", err)
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
		"test/forge_client_instance_session_gate_live_convergence_e2e_test.dart",
	)
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_CLIENT_INSTANCE_SESSION_GATE_LIVE_CONVERGENCE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter client-instance external Prompt convergence failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("client-instance external Prompt convergence Flutter output exceeded the size limit")
	}
}

func runForgeConsoleSchedulerSelectionPreviewE2EWithToken(
	t *testing.T, apiURL, token string, owner model.Owner, requirements deviceplacement.Requirements,
	conversationID, runID string, expectation schedulerSelectionPreviewExpectation,
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
		t.Fatalf("Flutter is required for the scheduler selection preview E2E: %v", err)
	}
	input := struct {
		APIURL                 string                           `json:"api_url"`
		AccessToken            string                           `json:"access_token"`
		Owner                  deviceplacement.Owner            `json:"owner"`
		ClientKinds            []string                         `json:"client_kinds"`
		Request                schedulerSelectionPreviewRequest `json:"request"`
		ExpectedConversationID string                           `json:"expected_conversation_id"`
		ExpectedRunID          string                           `json:"expected_run_id"`
		ExpectedAttemptID      string                           `json:"expected_attempt_id"`
		ExpectedSelection      bool                             `json:"expected_selection_available"`
		ExpectedReason         string                           `json:"expected_selection_reason"`
		ExpectedDeviceID       string                           `json:"expected_device_id"`
		ExpectedInstanceID     string                           `json:"expected_instance_id"`
	}{
		APIURL: apiURL, AccessToken: token,
		Owner:       deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		ClientKinds: []string{"web", "app", "mobile"},
		Request: schedulerSelectionPreviewRequest{
			ConversationID: conversationID, RunID: runID, AttemptID: "attempt-1",
			Requirements: requirements,
		},
		ExpectedConversationID: conversationID, ExpectedRunID: runID, ExpectedAttemptID: "attempt-1",
		ExpectedSelection: expectation.SelectionAvailable, ExpectedReason: expectation.SelectionReason,
		ExpectedDeviceID: expectation.DeviceID, ExpectedInstanceID: expectation.InstanceID,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter scheduler selection preview input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "scheduler-selection-preview-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter scheduler selection preview input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub", "test/forge_scheduler_selection_preview_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_SCHEDULER_SELECTION_PREVIEW_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter scheduler selection preview E2E failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("scheduler selection preview Flutter output exceeded the size limit")
	}
}

func runForgeConsoleSchedulerSelectionPreviewGateE2EWithToken(
	t *testing.T, apiURL, token string, owner model.Owner, requirements deviceplacement.Requirements,
	conversationID, runID string, expectation schedulerSelectionPreviewExpectation,
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
		t.Fatalf("Flutter is required for the scheduler selection Gate E2E: %v", err)
	}
	input := struct {
		APIURL                 string                           `json:"api_url"`
		AccessToken            string                           `json:"access_token"`
		Owner                  deviceplacement.Owner            `json:"owner"`
		Request                schedulerSelectionPreviewRequest `json:"request"`
		ExpectedConversationID string                           `json:"expected_conversation_id"`
		ExpectedRunID          string                           `json:"expected_run_id"`
		ExpectedAttemptID      string                           `json:"expected_attempt_id"`
		ExpectedSelection      bool                             `json:"expected_selection_available"`
		ExpectedReason         string                           `json:"expected_selection_reason"`
		ExpectedDeviceID       string                           `json:"expected_device_id"`
		ExpectedInstanceID     string                           `json:"expected_instance_id"`
	}{
		APIURL: apiURL, AccessToken: token,
		Owner: deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		Request: schedulerSelectionPreviewRequest{
			ConversationID: conversationID, RunID: runID, AttemptID: "attempt-1",
			Requirements: requirements,
		},
		ExpectedConversationID: conversationID, ExpectedRunID: runID, ExpectedAttemptID: "attempt-1",
		ExpectedSelection: expectation.SelectionAvailable, ExpectedReason: expectation.SelectionReason,
		ExpectedDeviceID: expectation.DeviceID, ExpectedInstanceID: expectation.InstanceID,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter scheduler selection Gate input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "scheduler-selection-preview-gate-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter scheduler selection Gate input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub", "--dart-define=FORGE_CONVERSATIONS_API_ORIGIN="+apiURL,
		"test/forge_scheduler_selection_preview_gate_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_SCHEDULER_SELECTION_PREVIEW_GATE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter scheduler selection Gate E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("scheduler selection Gate Flutter output exceeded the size limit")
	}
}

func runForgeRuntimeRegistryPlacementRemoteCLI(
	t *testing.T,
	executable, apiURL, accessToken string,
	owner model.Owner,
) {
	t.Helper()
	inputPath := filepath.Join(t.TempDir(), "registry-placement-requirements.json")
	if err := os.WriteFile(inputPath, []byte(registryPlacementRequirementsBody(t)), 0o600); err != nil {
		t.Fatalf("write registry placement requirements for Runtime CLI: %v", err)
	}
	output, stderr, err := runForgeRuntimeCLI(
		t,
		executable,
		apiURL,
		accessToken,
		t.TempDir(),
		"--json", "remote", "placement", "registry-preview", "--input", inputPath,
	)
	if err != nil {
		t.Fatalf("authenticated registry placement Rust CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var decoded deviceplacement.PersistedInventoryPlacementV2Evaluation
	if err := json.Unmarshal([]byte(output), &decoded); err != nil {
		t.Fatalf("decode authenticated registry placement Rust CLI: %v stdout=%q", err, output)
	}
	if !validDevicePlacementRegistryCandidateResult(decoded, owner, int64(decoded.EvaluatedAtMS)) ||
		len(decoded.Decisions) != 2 || decoded.SelectedDeviceID != nil || decoded.SelectedInstanceID != nil ||
		decoded.Authority != (deviceplacement.PersistedInventoryPlacementBatchAuthority{}) {
		t.Fatalf("authenticated registry placement Rust CLI=%#v", decoded)
	}
}

func runForgeRuntimeSchedulerSelectionRemoteCLI(
	t *testing.T,
	executable, apiURL, accessToken string,
	owner model.Owner,
	conversationID, runID string,
) {
	t.Helper()
	var placementRequest devicePlacementRegistryCandidateRequest
	if err := json.Unmarshal([]byte(registryPlacementRequirementsBody(t)), &placementRequest); err != nil {
		t.Fatalf("decode scheduler selection requirements for Runtime CLI: %v", err)
	}
	request := schedulerSelectionPreviewRequest{
		ConversationID: conversationID,
		RunID:          runID,
		AttemptID:      "attempt-1",
		Requirements:   placementRequest.Requirements,
	}
	runForgeRuntimeSchedulerSelectionRemoteCLIWithRequest(
		t, executable, apiURL, accessToken, owner, request,
		schedulerSelectionPreviewExpectation{
			SelectionReason: "no_eligible_candidate",
		},
	)
}

func runForgeRuntimeLifecycleRegistryRemoteCLI(
	t *testing.T,
	executable, apiURL, accessToken, issuer string,
) {
	t.Helper()
	output, stderr, err := runForgeRuntimeCLI(
		t,
		executable,
		apiURL,
		accessToken,
		t.TempDir(),
		"--json", "remote", "lifecycle-registry", "show",
	)
	if err != nil {
		t.Fatalf("authenticated lifecycle registry Rust CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var decoded lifecycleRegistryCandidateEnvelope
	if err := json.Unmarshal([]byte(output), &decoded); err != nil {
		t.Fatalf("decode authenticated lifecycle registry Rust CLI: %v stdout=%q", err, output)
	}
	if decoded.Owner.Issuer != issuer || decoded.Owner.Subject != snaplinkForgeTestUser ||
		decoded.Owner.TenantID != snaplinkForgeTestTenant || len(decoded.States) != 2 || decoded.SchemaVersion == "" {
		t.Fatalf("authenticated lifecycle registry Rust CLI=%#v", decoded)
	}
}

func runForgeRuntimeLifecycleRegistryRemoteTUI(
	t *testing.T,
	executable, apiURL, accessToken, issuer string,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("authenticated lifecycle registry TUI E2E requires script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("lifecycle-registry show\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated lifecycle registry Rust TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("authenticated lifecycle registry Rust TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"remote lifecycle registry [forge.device-enrollment-heartbeat-lifecycle-file-set/v1]",
		"owner=" + issuer + "/" + snaplinkForgeTestUser,
		"tenant=" + snaplinkForgeTestTenant,
		"states=2",
		"device=device-a",
		"device=device-b",
		"observation_only:",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("authenticated lifecycle registry Rust TUI output omitted %q: %q", want, output)
		}
	}
}

func runForgeRuntimePersistedInventoryV2TUILiveRefreshE2EWithToken(
	t *testing.T,
	executable, apiURL, accessToken, registryPath string,
	owner deviceidentity.Owner,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("live persisted inventory TUI E2E requires script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 45*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui",
	)
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	stdin, err := command.StdinPipe()
	if err != nil {
		t.Fatalf("live persisted inventory TUI stdin pipe: %v", err)
	}
	var stdoutBuffer, stderrBuffer synchronizedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Start(); err != nil {
		t.Fatalf("live persisted inventory TUI start: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	writeTUI := func(input string) {
		if _, err := stdin.Write([]byte(input)); err != nil {
			t.Fatalf("live persisted inventory TUI input %q: %v", input, err)
		}
	}
	writeTUI("inventory read-v2\n")
	waitForTUIOutput(t, &stdoutBuffer, "remote device inventory [forge.device-inventory-observation/v2]")

	updated := lifecycleRegistrySourceStateRevision(t, owner, "device-a", "runner-a", 2)
	second := lifecycleRegistrySourceState(t, owner, "device-b", "runner-b", 1)
	writeLifecycleRegistrySourceFileAtPath(t, registryPath, owner,
		[]deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{second, updated})

	writeTUI("sync\nquit\n")
	if err := stdin.Close(); err != nil {
		t.Fatalf("live persisted inventory TUI stdin close: %v", err)
	}
	if err := command.Wait(); err != nil {
		t.Fatalf("live persisted inventory TUI failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.Exceeded() || stderrBuffer.Exceeded() {
		t.Fatalf("live persisted inventory TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"Selected inventory observation refreshed.",
		"revision=2 generation=1 heartbeat=2",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("live persisted inventory TUI output omitted %q: %q", want, output)
		}
	}
}

func runForgeRuntimeClientInstanceViewsTUILiveRefreshE2EWithToken(
	t *testing.T,
	executable, apiURL, accessToken, clientInstancePath string,
	owner deviceidentity.Owner,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("live client-instance views TUI E2E requires script: %v", err)
	}
	writeClientInstanceSessionViewSourceFileAtPath(t, clientInstancePath, owner, clientInstanceLiveRefreshRows("001"))

	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 45*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui",
	)
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	stdin, err := command.StdinPipe()
	if err != nil {
		t.Fatalf("live client-instance views TUI stdin pipe: %v", err)
	}
	var stdoutBuffer, stderrBuffer synchronizedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Start(); err != nil {
		t.Fatalf("live client-instance views TUI start: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	writeTUI := func(input string) {
		if _, err := stdin.Write([]byte(input)); err != nil {
			t.Fatalf("live client-instance views TUI input %q: %v", input, err)
		}
	}
	writeTUI("client-instances session-view\n")
	waitForTUIOutput(t, &stdoutBuffer, "remote client-instance/session-view [forge.client-instance-session-view/v1]")
	waitForTUIOutput(t, &stdoutBuffer, "instance client-cli-live-001:")
	writeTUI("client-instances resource-view\n")
	waitForTUIOutput(t, &stdoutBuffer, "remote client-instance/resource-view [forge.client-instance-resource-view/v1]")
	waitForTUIOutput(t, &stdoutBuffer, "instance client-web-live-001:")

	writeClientInstanceSessionViewSourceFileAtPath(t, clientInstancePath, owner, clientInstanceLiveRefreshRows("002"))
	writeTUI("sync\nquit\n")
	if err := stdin.Close(); err != nil {
		t.Fatalf("live client-instance views TUI stdin close: %v", err)
	}
	if err := command.Wait(); err != nil {
		t.Fatalf("live client-instance views TUI failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.Exceeded() || stderrBuffer.Exceeded() {
		t.Fatalf("live client-instance views TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"Selected client-instance/session-view refreshed.",
		"Selected client-instance/resource-view refreshed.",
		"instance client-cli-live-002:",
		"instance client-web-live-002:",
		"device device-a: runner=runner-a",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("live client-instance views TUI output omitted %q: %q", want, output)
		}
	}
}

func runForgeRuntimeClientInstanceLiveRefreshPromptE2EWithToken(
	t *testing.T,
	executable, apiURL, accessToken, clientInstancePath string,
	owner deviceidentity.Owner,
	conversationID string,
) string {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("live client-instance Prompt TUI E2E requires script: %v", err)
	}
	writeClientInstanceSessionViewSourceFileAtPath(
		t,
		clientInstancePath,
		owner,
		clientInstanceLiveRefreshRowsForConversations("001", "conversation-a", conversationID),
	)

	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 45*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui",
	)
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	stdin, err := command.StdinPipe()
	if err != nil {
		t.Fatalf("live client-instance Prompt TUI stdin pipe: %v", err)
	}
	var stdoutBuffer, stderrBuffer synchronizedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Start(); err != nil {
		t.Fatalf("live client-instance Prompt TUI start: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	writeTUI := func(input string) {
		if _, err := stdin.Write([]byte(input)); err != nil {
			t.Fatalf("live client-instance Prompt TUI input %q: %v", input, err)
		}
	}
	writeTUI("client-instances session-view\n")
	waitForTUIOutput(t, &stdoutBuffer, "remote client-instance/session-view [forge.client-instance-session-view/v1]")
	writeTUI("instance client-tui-live-001\n")
	waitForTUIOutput(t, &stdoutBuffer, `Client-instance filter set to "client-tui-live-001"`)
	writeTUI("open " + conversationID + "\n")
	waitForTUIOutput(t, &stdoutBuffer, "Opened session")

	writeClientInstanceSessionViewSourceFileAtPath(
		t,
		clientInstancePath,
		owner,
		clientInstanceLiveRefreshRowsForConversations("002", "conversation-a", conversationID),
	)
	writeTUI("sync\n")
	waitForTUIOutput(t, &stdoutBuffer, "Selected client-instance/session-view refreshed.")
	writeTUI("instance client-tui-live-002\n")
	waitForTUIOutput(t, &stdoutBuffer, `Client-instance filter set to "client-tui-live-002"`)
	writeTUI("open " + conversationID + "\n")
	waitForTUIOutput(t, &stdoutBuffer, "Opened session")
	writeTUI("prompt Prompt submitted after client-instance refresh\n")
	waitForTUIOutput(t, &stdoutBuffer, "Prompt stored. No Run was started.")
	writeTUI("quit\n")
	if err := stdin.Close(); err != nil {
		t.Fatalf("live client-instance Prompt TUI stdin close: %v", err)
	}
	if err := command.Wait(); err != nil {
		t.Fatalf("live client-instance Prompt TUI failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.Exceeded() || stderrBuffer.Exceeded() {
		t.Fatalf("live client-instance Prompt TUI output exceeded the size limit")
	}
	return stdoutBuffer.String()
}

func runForgeRuntimeClientInstanceExternalPromptConvergenceE2EWithToken(
	t *testing.T,
	executable, apiURL, accessToken, clientInstancePath string,
	owner deviceidentity.Owner,
	conversationID string,
) string {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("external Prompt convergence TUI E2E requires script: %v", err)
	}
	writeClientInstanceSessionViewSourceFileAtPath(
		t,
		clientInstancePath,
		owner,
		clientInstanceLiveRefreshRowsForConversations("005", "conversation-a", conversationID),
	)

	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui",
	)
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	stdin, err := command.StdinPipe()
	if err != nil {
		t.Fatalf("external Prompt convergence TUI stdin pipe: %v", err)
	}
	var stdoutBuffer, stderrBuffer synchronizedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Start(); err != nil {
		t.Fatalf("external Prompt convergence TUI start: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	writeTUI := func(input string) {
		if _, err := stdin.Write([]byte(input)); err != nil {
			t.Fatalf("external Prompt convergence TUI input %q: %v", input, err)
		}
	}
	writeTUI("client-instances session-view\n")
	waitForTUIOutput(t, &stdoutBuffer, "remote client-instance/session-view [forge.client-instance-session-view/v1]")
	writeTUI("instance client-tui-live-005\n")
	waitForTUIOutput(t, &stdoutBuffer, `Client-instance filter set to "client-tui-live-005"`)
	writeTUI("open " + conversationID + "\n")
	waitForTUIOutput(t, &stdoutBuffer, "Opened session")

	sessionsOutput, sessionsStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "remote", "sessions", "list", "--instance", "client-cli-live-005",
	)
	if err != nil {
		t.Fatalf("external Prompt convergence CLI session read failed: stderr=%q stdout=%q err=%v", sessionsStderr, sessionsOutput, err)
	}
	var sessions model.OwnedConversationPage
	if err := json.Unmarshal([]byte(sessionsOutput), &sessions); err != nil {
		t.Fatalf("decode external Prompt convergence CLI sessions: %v stdout=%q", err, sessionsOutput)
	}
	var expectedVersion uint64
	for _, entry := range sessions.Conversations {
		if entry.Conversation.ID == conversationID {
			expectedVersion = entry.AggregateVersion
			break
		}
	}
	if expectedVersion == 0 {
		t.Fatalf("external Prompt convergence CLI session page omitted %q: %#v", conversationID, sessions)
	}
	const content = "Prompt from external Runtime CLI after client-instance refresh"
	cliOutput, cliStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "--idempotency-key", "accepted-production-external-cli-prompt",
		"remote", "prompts", "add", conversationID,
		"--expected-version", strconv.FormatUint(expectedVersion, 10),
		"--instance", "client-cli-live-005", content,
	)
	if err != nil || !strings.Contains(cliOutput, content) {
		t.Fatalf("external Prompt convergence CLI write failed: stderr=%q stdout=%q err=%v", cliStderr, cliOutput, err)
	}
	writeTUI("sync\n")
	waitForTUIOutput(t, &stdoutBuffer, "Prompt history refreshed for the selected session.")
	waitForTUIOutput(t, &stdoutBuffer, content)
	writeTUI("quit\n")
	if err := stdin.Close(); err != nil {
		t.Fatalf("external Prompt convergence TUI stdin close: %v", err)
	}
	if err := command.Wait(); err != nil {
		t.Fatalf("external Prompt convergence TUI failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.Exceeded() || stderrBuffer.Exceeded() {
		t.Fatalf("external Prompt convergence TUI output exceeded the size limit")
	}
	return stdoutBuffer.String()
}

func clientInstanceLiveRefreshRows(suffix string) []deviceplacement.ClientInstanceSessionViewInstance {
	return clientInstanceLiveRefreshRowsForConversations(suffix, "conversation-a", "conversation-b")
}

func clientInstanceLiveRefreshRowsForConversations(
	suffix, firstConversationID, secondConversationID string,
) []deviceplacement.ClientInstanceSessionViewInstance {
	return []deviceplacement.ClientInstanceSessionViewInstance{
		{InstanceID: "client-cli-live-" + suffix, ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{firstConversationID, secondConversationID}, ObservedAtMS: 201500, Status: "active"},
		{InstanceID: "client-tui-live-" + suffix, ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{secondConversationID}, ObservedAtMS: 201500, Status: "active"},
		{InstanceID: "client-web-live-" + suffix, ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{firstConversationID}, ObservedAtMS: 201500, Status: "active"},
		{InstanceID: "client-app-live-" + suffix, ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{firstConversationID}, ObservedAtMS: 201500, Status: "active"},
		{InstanceID: "client-mobile-live-" + suffix, ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{secondConversationID}, ObservedAtMS: 201500, Status: "active"},
	}
}

func runForgeRuntimeClientInstanceViewsCLIRefreshE2EWithToken(
	t *testing.T,
	executable, apiURL, accessToken, clientInstancePath string,
	owner deviceidentity.Owner,
) {
	t.Helper()
	writeClientInstanceSessionViewSourceFileAtPath(t, clientInstancePath, owner, clientInstanceLiveRefreshRows("003"))

	sessionOutput, sessionStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "remote", "client-instances", "session-view",
	)
	if err != nil {
		t.Fatalf("live client-instance session CLI refresh failed: stderr=%q stdout=%q err=%v", sessionStderr, sessionOutput, err)
	}
	var sessionView deviceplacement.ClientInstanceSessionViewObservation
	if err := json.Unmarshal([]byte(sessionOutput), &sessionView); err != nil {
		t.Fatalf("decode live client-instance session CLI refresh: %v stdout=%q", err, sessionOutput)
	}
	if err := sessionView.Validate(); err != nil || len(sessionView.Instances) != 5 ||
		sessionView.Instances[0].InstanceID != "client-app-live-003" ||
		sessionView.Instances[4].InstanceID != "client-web-live-003" {
		t.Fatalf("live client-instance session CLI refresh=%#v err=%v", sessionView, err)
	}

	resourceOutput, resourceStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "remote", "client-instances", "resource-view",
	)
	if err != nil {
		t.Fatalf("live client-instance resource CLI refresh failed: stderr=%q stdout=%q err=%v", resourceStderr, resourceOutput, err)
	}
	var resourceView deviceplacement.ClientInstanceResourceViewObservation
	if err := json.Unmarshal([]byte(resourceOutput), &resourceView); err != nil {
		t.Fatalf("decode live client-instance resource CLI refresh: %v stdout=%q", err, resourceOutput)
	}
	if err := resourceView.Validate(); err != nil || len(resourceView.Instances) != 5 || len(resourceView.Devices) != 2 ||
		resourceView.Instances[0].InstanceID != "client-app-live-003" ||
		resourceView.Instances[4].InstanceID != "client-web-live-003" ||
		resourceView.Devices[0].DeviceID != "device-a" {
		t.Fatalf("live client-instance resource CLI refresh=%#v err=%v", resourceView, err)
	}
}

func runForgeRuntimeClientInstanceCLIPromptAfterRefreshE2EWithToken(
	t *testing.T,
	executable, apiURL, accessToken, clientInstancePath string,
	owner deviceidentity.Owner,
	conversationID string,
) {
	t.Helper()
	const prompt = "Prompt from Runtime CLI after client-instance refresh"
	writeClientInstanceSessionViewSourceFileAtPath(
		t,
		clientInstancePath,
		owner,
		clientInstanceLiveRefreshRowsForConversations("004", conversationID, conversationID+"-other"),
	)

	sessionOutput, sessionStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "remote", "sessions", "list", "--instance", "client-cli-live-004",
	)
	if err != nil {
		t.Fatalf("live client-instance CLI Prompt session read failed: stderr=%q stdout=%q err=%v", sessionStderr, sessionOutput, err)
	}
	var sessions model.OwnedConversationPage
	if err := json.Unmarshal([]byte(sessionOutput), &sessions); err != nil {
		t.Fatalf("decode live client-instance CLI Prompt sessions: %v stdout=%q", err, sessionOutput)
	}
	var expectedVersion uint64
	for _, entry := range sessions.Conversations {
		if entry.Conversation.ID == conversationID {
			expectedVersion = entry.AggregateVersion
			break
		}
	}
	if expectedVersion == 0 {
		t.Fatalf("live client-instance CLI Prompt session page omitted %q: %#v", conversationID, sessions)
	}
	promptOutput, promptStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "--idempotency-key", "accepted-production-live-cli-prompt",
		"remote", "prompts", "add", conversationID,
		"--expected-version", strconv.FormatUint(expectedVersion, 10),
		"--instance", "client-cli-live-004", prompt,
	)
	if err != nil || !strings.Contains(promptOutput, prompt) {
		t.Fatalf("live client-instance CLI Prompt failed: stderr=%q stdout=%q err=%v", promptStderr, promptOutput, err)
	}
	historyOutput, historyStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "remote", "prompts", "list", conversationID,
	)
	if err != nil {
		t.Fatalf("live client-instance CLI Prompt history read failed: stderr=%q stdout=%q err=%v", historyStderr, historyOutput, err)
	}
	var history model.ConversationPromptPage
	if err := json.Unmarshal([]byte(historyOutput), &history); err != nil ||
		!promptPageContains(history, prompt) {
		t.Fatalf("live client-instance CLI Prompt history=%#v stdout=%q decode=%v", history, historyOutput, err)
	}
}

func lifecycleRegistrySourceStateRevision(
	t *testing.T,
	owner deviceidentity.Owner,
	deviceID, instanceID string,
	revision uint64,
) deviceinventory.PersistedEnrollmentHeartbeatLifecycleState {
	t.Helper()
	if revision == 0 {
		t.Fatal("lifecycle registry revision must be positive")
	}
	value := lifecycleRegistrySourceState(t, owner, deviceID, instanceID, 1)
	value.Revision = revision
	value.Heartbeat.Revision = revision
	value.Heartbeat.Instance.HeartbeatSequence = revision
	value.Heartbeat.Instance.ServerObservedAtMS += (revision - 1) * 1_000
	value.Heartbeat.Instance.CapabilityLeaseExpiresAtMS += (revision - 1) * 1_000
	value.Inventory.Revision = revision
	value.Inventory.Runner.HeartbeatSequence = revision
	value.Inventory.Runner.ServerObservedAtMS += (revision - 1) * 1_000
	value.Inventory.Runner.CapabilityLeaseExpiresAtMS += (revision - 1) * 1_000
	return value
}
