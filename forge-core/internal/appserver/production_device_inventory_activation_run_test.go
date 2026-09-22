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
		if executable := os.Getenv("FORGE_RUNTIME_BIN"); executable != "" {
			httpClient := &http.Client{Timeout: 20 * time.Second}
			first := createSharedConversationAsClientA(t, httpClient, ready.Listen, token)
			second := createProjectionConversation(t, httpClient, ready.Listen, token)
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
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)

	root := privateTestParent(t)
	stateDir := filepath.Join(root, "app-state-execute-admission")
	runtimeStateDir := filepath.Join(root, "runtime-state-execute-admission")
	if err := os.Mkdir(runtimeStateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	runtimeExecutable := filepath.Join(root, "forge-runtime-execute-admission")
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
	schedulerRequest, err := json.Marshal(schedulerSelectionPreviewRequest{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		Requirements: placementRequest.Requirements,
	})
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
	if err := selection.Validate(); err != nil || selection.SelectionAvailable || selection.SelectedDeviceID != nil || selection.SelectedInstanceID != nil ||
		selection.SelectionReason != "no_eligible_candidate" || selection.Authority != (deviceplacement.SchedulerSelectionPreviewAuthority{}) {
		t.Fatalf("accepted execute scheduler selection=%#v err=%v", selection, err)
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
