package appserver

// This opt-in integration test proves the client-instance session projection
// against two real owner Conversations and the existing Rust Runtime CLI/TUI
// transport. The CLI create and Prompt write also read the injected
// owner-bound observations before their storage POSTs; production route wiring
// remains closed and is asserted as 404.

import (
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
	"forgeos/forge-core/internal/runtimebridge"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestSnaplinkAuthenticatedClientInstanceSessionProjectionE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_SESSION_PROJECTION_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_SESSION_PROJECTION_E2E=1 for client-instance session projection E2E")
	}
	configuredExecutable := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredExecutable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for client-instance session projection E2E")
	}
	executable, err := exec.LookPath(configuredExecutable)
	if err != nil {
		t.Fatalf("FORGE_RUNTIME_BIN must name a built forge-runtime executable available to the test: %v", err)
	}
	executable, err = filepath.Abs(executable)
	if err != nil {
		t.Fatalf("resolve forge-runtime executable path: %v", err)
	}
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	if err := os.Mkdir(runtimeState, 0o700); err != nil {
		t.Fatal(err)
	}
	initializeRuntimeHubForIntegration(t, executable, runtimeState)
	appState := filepath.Join(t.TempDir(), "app-server-state")
	if err := os.Mkdir(appState, 0o700); err != nil {
		t.Fatal(err)
	}
	bridge, err := runtimebridge.New(runtimebridge.Config{
		Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState,
	})
	if err != nil {
		t.Fatal(err)
	}

	identity, authenticator := newMultiPrincipalConversationTestIdentity(t)
	source := &staticClientInstanceSessionViewSource{}
	resourceSource := &fixtureClientInstanceResourceViewSource{}
	owner := deviceplacement.Owner{
		Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate",
	}
	// The explicit CLI Prompt-write guard reads the same owner-bound inventory
	// image as the resource projection. Keep this candidate separate from the
	// ordinary session/resource sources so the test can prove the extra read
	// occurs only after the pair has converged.
	inventory := fixtureDeviceInventoryReadV2Value(owner)
	inventory.Devices[0].Device.ReservationState = "none"
	inventory.Devices[0].Device.GPUs = []deviceplacement.GPUDeclarationV2{}
	inventorySource := &fixtureDeviceInventoryReadV2Source{value: inventory}
	sessions := newAuthenticatedSessionRoutesWithObservationCandidates(bridge, nil)
	testRoutes := http.NewServeMux()
	testRoutes.Handle(deviceInventoryReadCandidateV2Path,
		newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
			Enabled: true,
			Source:  inventorySource,
		}))
	testRoutes.Handle(clientInstanceSessionViewCandidatePath,
		newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
			Enabled: true,
			Source:  source,
		}))
	testRoutes.Handle(clientInstanceResourceViewCandidatePath,
		newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
			Enabled: true,
			Source:  resourceSource,
		}))
	testRoutes.Handle("/", sessions)
	recorder := &conversationHTTPRecorder{}
	server := httptest.NewServer(authenticator.Handler(recorder.wrap(testRoutes)))
	t.Cleanup(server.Close)

	const scopes = "forge:conversations:read forge:conversations:write " + deviceInventoryReadCandidateScope
	ownerToken := tokenForIndependentClient(identity, scopes, "projection-http-owner")
	httpClient := &http.Client{Timeout: 20 * time.Second}
	first := createSharedConversationAsClientA(t, httpClient, server.URL, ownerToken)
	second := createProjectionConversation(t, httpClient, server.URL, ownerToken)

	view, err := deviceplacement.ObserveClientInstanceSessionView(
		deviceplacement.ClientInstanceSessionViewRequest{
			Owner: owner,
			Instances: []deviceplacement.ClientInstanceSessionViewInstance{
				{InstanceID: "client-mobile-001", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{second.ID}, ObservedAtMS: 200_500, Status: "active"},
				{InstanceID: "client-app-001", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{first.ID}, ObservedAtMS: 200_500, Status: "idle"},
				{InstanceID: "client-web-001", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{first.ID}, ObservedAtMS: 200_500, Status: "active"},
				{InstanceID: "client-tui-001", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{second.ID}, ObservedAtMS: 200_500, Status: "active"},
				{InstanceID: "client-cli-001", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{first.ID, second.ID}, ObservedAtMS: 200_500, Status: "active"},
			},
		},
	)
	if err != nil {
		t.Fatalf("build five-client session view: %v", err)
	}
	if len(view.Instances) != 5 || view.Instances[0].InstanceID != "client-app-001" ||
		view.Instances[4].InstanceID != "client-web-001" || view.Authority != (deviceplacement.ClientInstanceSessionViewAuthority{}) {
		t.Fatalf("unexpected five-client session view: %#v", view)
	}
	wantClientKinds := map[string]string{
		"client-cli-001":    deviceplacement.ClientKindCLI,
		"client-tui-001":    deviceplacement.ClientKindTUI,
		"client-web-001":    deviceplacement.ClientKindWeb,
		"client-app-001":    deviceplacement.ClientKindApp,
		"client-mobile-001": deviceplacement.ClientKindMobile,
	}
	for _, instance := range view.Instances {
		if want := wantClientKinds[instance.InstanceID]; want == "" || instance.ClientKind != want {
			t.Fatalf("five-client session view has unexpected client kind: %#v", view.Instances)
		}
	}
	// The test server has no registration store. The candidate receives this
	// explicit observation only after the real Conversations exist.
	source.value = view
	resourceView := fixtureClientInstanceResourceView(owner)
	resourceView.Instances = append([]deviceplacement.ClientInstanceSessionViewInstance(nil), view.Instances...)
	if err := resourceView.Validate(); err != nil {
		t.Fatalf("build paired resource view: %v", err)
	}
	resourceSource.value = resourceView

	servedViewResponse := doConversationClientRequest(
		t, httpClient, server.URL, ownerToken, http.MethodGet,
		clientInstanceSessionViewCandidatePath, "", "", "",
	)
	if servedViewResponse.StatusCode != http.StatusOK {
		t.Fatalf("session view candidate status=%d body=%q", servedViewResponse.StatusCode, readConversationClientBody(t, servedViewResponse))
	}
	var servedView deviceplacement.ClientInstanceSessionViewObservation
	if err := json.NewDecoder(servedViewResponse.Body).Decode(&servedView); err != nil {
		_ = servedViewResponse.Body.Close()
		t.Fatalf("decode session view candidate: %v", err)
	}
	_ = servedViewResponse.Body.Close()
	if err := servedView.Validate(); err != nil || len(servedView.Instances) != 5 {
		t.Fatalf("served five-client session view=%#v err=%v", servedView, err)
	}
	projectionCases := []struct {
		instanceID string
		wantIDs    []string
	}{
		{instanceID: "client-cli-001", wantIDs: []string{first.ID, second.ID}},
		{instanceID: "client-tui-001", wantIDs: []string{second.ID}},
		{instanceID: "client-web-001", wantIDs: []string{first.ID}},
		{instanceID: "client-app-001", wantIDs: []string{first.ID}},
		{instanceID: "client-mobile-001", wantIDs: []string{second.ID}},
	}
	for _, projection := range projectionCases {
		t.Run("cli_"+projection.instanceID, func(t *testing.T) {
			output, stderr, err := runForgeRuntimeCLI(
				t, executable, server.URL,
				tokenForIndependentClient(identity, scopes, "projection-"+projection.instanceID),
				t.TempDir(), "--json", "remote", "sessions", "list", "--instance", projection.instanceID,
			)
			if err != nil {
				t.Fatalf("CLI instance projection failed: stderr=%q stdout=%q err=%v", stderr, output, err)
			}
			var page model.OwnedConversationPage
			if err := json.Unmarshal([]byte(output), &page); err != nil {
				t.Fatalf("decode CLI instance projection: %v stdout=%q", err, output)
			}
			if page.HasMore || page.NextAfterID != nil || len(page.Conversations) != len(projection.wantIDs) {
				t.Fatalf("CLI instance %q page=%#v stdout=%q", projection.instanceID, page, output)
			}
			seen := make(map[string]struct{}, len(page.Conversations))
			for _, entry := range page.Conversations {
				seen[entry.Conversation.ID] = struct{}{}
			}
			for _, expectedID := range projection.wantIDs {
				if _, ok := seen[expectedID]; !ok {
					t.Fatalf("CLI instance %q omitted %q; page=%#v", projection.instanceID, expectedID, page)
				}
			}
		})
	}
	assertCLIInstanceCreatePreflight(t, executable, server.URL, ownerToken, recorder)
	conversationOnlyToken := tokenForIndependentClient(
		identity, "forge:conversations:read forge:conversations:write", "projection-cli-instance-no-device-scope",
	)
	assertCLIInstanceCreateRequiresResourceScope(t, executable, server.URL, conversationOnlyToken, recorder)
	assertTUIInstanceCreatePreflight(
		t, executable, server.URL,
		tokenForIndependentClient(identity, scopes, "projection-tui-instance-create"), recorder,
	)

	// The CLI must carry the selected instance boundary through an actual
	// owner-scoped Prompt write, not only through the session list. Online
	// filtering reads the converged session/resource pair and inventory/resource
	// image before the single Prompt POST; the selected CLI row declares the first
	// Conversation.
	cliWriteStart := len(recorder.snapshot())
	cliPromptOutput, cliPromptStderr, err := runForgeRuntimeCLI(
		t, executable, server.URL,
		tokenForIndependentClient(identity, scopes, "projection-cli-write"),
		t.TempDir(), "--json", "--idempotency-key", "projection-cli-instance-prompt", "remote", "prompts", "add", first.ID,
		"--expected-version", "1", "--instance", "client-cli-001",
		"Prompt submitted from authenticated CLI instance",
	)
	if err != nil || !strings.Contains(cliPromptOutput, "Prompt submitted from authenticated CLI instance") {
		t.Fatalf("CLI instance Prompt write failed: stderr=%q stdout=%q err=%v", cliPromptStderr, cliPromptOutput, err)
	}
	cliWriteRequests := recorder.snapshot()[cliWriteStart:]
	if len(cliWriteRequests) != 5 ||
		cliWriteRequests[0].path != clientInstanceSessionViewCandidatePath ||
		cliWriteRequests[1].path != clientInstanceResourceViewCandidatePath ||
		cliWriteRequests[2].path != deviceInventoryReadCandidateV2Path ||
		cliWriteRequests[3].path != clientInstanceResourceViewCandidatePath ||
		cliWriteRequests[len(cliWriteRequests)-1].path != conversationCollectionPath+"/"+first.ID+"/prompts" {
		t.Fatalf("CLI instance Prompt write did not converge pair and inventory/resource before POST: %#v", cliWriteRequests)
	}
	hiddenWriteStart := len(recorder.snapshot())
	_, hiddenPromptStderr, hiddenPromptErr := runForgeRuntimeCLI(
		t, executable, server.URL,
		tokenForIndependentClient(identity, scopes, "projection-cli-hidden-write"),
		t.TempDir(), "--json", "--idempotency-key", "projection-cli-hidden-prompt", "remote", "prompts", "add", second.ID,
		"--expected-version", "1", "--instance", "client-web-001",
		"Prompt hidden from authenticated Web instance",
	)
	if hiddenPromptErr == nil || !strings.Contains(hiddenPromptStderr, "no Prompt request was sent") {
		t.Fatalf("hidden CLI instance Prompt write was not rejected: stderr=%q err=%v", hiddenPromptStderr, hiddenPromptErr)
	}
	for _, request := range recorder.snapshot()[hiddenWriteStart:] {
		if request.path == deviceInventoryReadCandidateV2Path ||
			request.path == conversationCollectionPath+"/"+second.ID+"/prompts" {
			t.Fatalf("hidden CLI instance Prompt write escaped the pair guard: %#v", request)
		}
	}
	if inventorySource.calls != 1 || inventorySource.owner != (model.Owner{
		Issuer: identity.issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	}) {
		t.Fatalf("CLI Prompt inventory source=%#v; want one owner-bound read and no hidden read", inventorySource)
	}

	// The shared Flutter Sessions surface is used by Web, desktop App, and
	// Mobile. Exercise each declared instance independently so this acceptance
	// path proves filtering and Prompt writes are not only a Web case.
	consoleCases := []struct {
		name       string
		instanceID string
		token      string
		visible    model.Conversation
		hidden     model.Conversation
		prompt     string
	}{
		{
			name:       "web",
			instanceID: "client-web-001",
			token:      tokenForIndependentClient(identity, scopes, "projection-console-web"),
			visible:    first,
			hidden:     second,
			prompt:     "Prompt submitted from authenticated Web Console instance",
		},
		{
			name:       "app",
			instanceID: "client-app-001",
			token:      tokenForIndependentClient(identity, scopes, "projection-console-app"),
			visible:    first,
			hidden:     second,
			prompt:     "Prompt submitted from authenticated desktop App instance",
		},
		{
			name:       "mobile",
			instanceID: "client-mobile-001",
			token:      tokenForIndependentClient(identity, scopes, "projection-console-mobile"),
			visible:    second,
			hidden:     first,
			prompt:     "Prompt submitted from authenticated Mobile instance",
		},
	}
	for _, projection := range consoleCases {
		projection := projection
		t.Run("console_"+projection.name, func(t *testing.T) {
			runForgeConsoleClientInstanceSessionProjectionE2EWithToken(
				t, server.URL, projection.token, owner, projection.instanceID,
				projection.visible, projection.hidden, projection.prompt, recorder,
			)
		})
	}

	firstTUIRequest := len(recorder.snapshot())
	tuiInventoryCallsBefore := inventorySource.calls
	tuiToken := tokenForIndependentClient(identity, scopes, "projection-tui")
	tuiOutput := runForgeRuntimeClientInstanceProjectionTUI(t, executable, server.URL, tuiToken, second.ID)
	if !strings.Contains(tuiOutput, `Client-instance filter set to "client-tui-001"`) ||
		!strings.Contains(tuiOutput, "Prompt stored. No Run was started.") ||
		!strings.Contains(tuiOutput, "Prompt submitted from client-instance TUI") {
		t.Fatalf("TUI projection output omitted filter or Prompt receipt: %q", tuiOutput)
	}
	tuiRequests := recorder.snapshot()[firstTUIRequest:]
	assertRuntimeTUIInstancePromptFreshness(t, tuiRequests, second.ID)
	assertClientInstanceProjectionRequestsHaveNoExecutionOrDeviceEffects(t, tuiRequests)
	if inventorySource.calls != tuiInventoryCallsBefore+2 {
		t.Fatalf("TUI instance Prompt inventory source calls=%d; want initial plus one fresh owner-bound read after %d", inventorySource.calls, tuiInventoryCallsBefore)
	}

	readerOutput, readerStderr, err := runForgeRuntimeCLI(
		t, executable, server.URL,
		tokenForIndependentClient(identity, scopes, "projection-prompt-reader"), t.TempDir(),
		"--json", "remote", "prompts", "list", second.ID,
	)
	if err != nil {
		t.Fatalf("second client Prompt read failed: stderr=%q stdout=%q err=%v", readerStderr, readerOutput, err)
	}
	var history model.ConversationPromptPage
	if err := json.Unmarshal([]byte(readerOutput), &history); err != nil || history.ConversationID != second.ID ||
		len(history.Prompts) != 2 ||
		!promptPageContains(history, "Prompt submitted from authenticated Mobile instance") ||
		!promptPageContains(history, "Prompt submitted from client-instance TUI") {
		t.Fatalf("second client Prompt history=%#v stdout=%q decode=%v", history, readerOutput, err)
	}
	firstHistoryOutput, firstHistoryStderr, err := runForgeRuntimeCLI(
		t, executable, server.URL,
		tokenForIndependentClient(identity, scopes, "projection-console-prompt-reader"), t.TempDir(),
		"--json", "remote", "prompts", "list", first.ID,
	)
	if err != nil {
		t.Fatalf("Console instance Prompt read failed: stderr=%q stdout=%q err=%v", firstHistoryStderr, firstHistoryOutput, err)
	}
	var firstHistory model.ConversationPromptPage
	if err := json.Unmarshal([]byte(firstHistoryOutput), &firstHistory); err != nil ||
		firstHistory.ConversationID != first.ID || len(firstHistory.Prompts) != 3 ||
		!promptPageContains(firstHistory, "Prompt submitted from authenticated CLI instance") ||
		!promptPageContains(firstHistory, "Prompt submitted from authenticated desktop App instance") ||
		!promptPageContains(firstHistory, "Prompt submitted from authenticated Web Console instance") {
		t.Fatalf("first client Prompt history=%#v stdout=%q decode=%v", firstHistory, firstHistoryOutput, err)
	}

	assertClientInstanceProjectionRequestsHaveNoExecutionOrDeviceEffects(t, recorder.snapshot())
	production := authenticator.Handler(newConversationRoutes(bridge))
	request := httptest.NewRequest(http.MethodGet, clientInstanceSessionViewCandidatePath, nil)
	request.Header.Set("Authorization", "Bearer "+ownerToken)
	productionResponse := httptest.NewRecorder()
	production.ServeHTTP(productionResponse, request)
	if productionResponse.Code != http.StatusNotFound {
		t.Fatalf("production client-instance session projection route status=%d body=%q", productionResponse.Code, productionResponse.Body.String())
	}
	resourceRequest := httptest.NewRequest(http.MethodGet, clientInstanceResourceViewCandidatePath, nil)
	resourceRequest.Header.Set("Authorization", "Bearer "+ownerToken)
	resourceResponse := httptest.NewRecorder()
	production.ServeHTTP(resourceResponse, resourceRequest)
	if resourceResponse.Code != http.StatusNotFound {
		t.Fatalf("production client-instance resource projection route status=%d body=%q", resourceResponse.Code, resourceResponse.Body.String())
	}
	inventoryRequest := httptest.NewRequest(http.MethodGet, deviceInventoryReadCandidateV2Path, nil)
	inventoryRequest.Header.Set("Authorization", "Bearer "+ownerToken)
	inventoryResponse := httptest.NewRecorder()
	production.ServeHTTP(inventoryResponse, inventoryRequest)
	if inventoryResponse.Code != http.StatusNotFound {
		t.Fatalf("production inventory projection route status=%d body=%q", inventoryResponse.Code, inventoryResponse.Body.String())
	}
}

func promptPageContains(page model.ConversationPromptPage, content string) bool {
	for _, prompt := range page.Prompts {
		if prompt.Role == "user" && prompt.Content == content {
			return true
		}
	}
	return false
}

func createProjectionConversation(
	t *testing.T,
	client *http.Client,
	baseURL, token string,
) model.Conversation {
	t.Helper()
	response := doConversationClientRequest(
		t, client, baseURL, token, http.MethodPost, conversationCollectionPath,
		"application/json", "client-instance-projection-create-b",
		`{"scope":{"kind":"global"},"title":"Shared from client-instance projection"}`,
	)
	if response.StatusCode != http.StatusCreated {
		t.Fatalf("client-instance projection Conversation create status=%d body=%q", response.StatusCode, readConversationClientBody(t, response))
	}
	var conversation model.Conversation
	if err := json.NewDecoder(response.Body).Decode(&conversation); err != nil || conversation.ID == "" {
		_ = response.Body.Close()
		t.Fatalf("client-instance projection Conversation response=%#v decode=%v", conversation, err)
	}
	_ = response.Body.Close()
	return conversation
}
