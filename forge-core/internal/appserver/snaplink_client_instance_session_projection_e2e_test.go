package appserver

// This opt-in integration test proves the client-instance session projection
// against two real owner Conversations and the existing Rust Runtime CLI/TUI
// transport.  The instance view is still an injected candidate observation;
// production route wiring remains closed and is asserted as 404.

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
	sessions := newAuthenticatedSessionRoutesWithObservationCandidates(bridge, nil)
	testRoutes := http.NewServeMux()
	testRoutes.Handle(clientInstanceSessionViewCandidatePath,
		newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
			Enabled: true,
			Source:  source,
		}))
	testRoutes.Handle("/", sessions)
	recorder := &conversationHTTPRecorder{}
	server := httptest.NewServer(authenticator.Handler(recorder.wrap(testRoutes)))
	t.Cleanup(server.Close)

	const scopes = "forge:conversations:read forge:conversations:write"
	ownerToken := tokenForIndependentClient(identity, scopes, "projection-http-owner")
	httpClient := &http.Client{Timeout: 20 * time.Second}
	first := createSharedConversationAsClientA(t, httpClient, server.URL, ownerToken)
	second := createProjectionConversation(t, httpClient, server.URL, ownerToken)

	owner := deviceplacement.Owner{
		Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate",
	}
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
				projection.visible, projection.hidden, projection.prompt,
			)
		})
	}

	firstTUIRequest := len(recorder.snapshot())
	tuiToken := tokenForIndependentClient(identity, scopes, "projection-tui")
	tuiOutput := runForgeRuntimeClientInstanceProjectionTUI(t, executable, server.URL, tuiToken, second.ID)
	if !strings.Contains(tuiOutput, `Client-instance filter set to "client-tui-001"`) ||
		!strings.Contains(tuiOutput, "Prompt stored. No Run was started.") ||
		!strings.Contains(tuiOutput, "Prompt submitted from client-instance TUI") {
		t.Fatalf("TUI projection output omitted filter or Prompt receipt: %q", tuiOutput)
	}
	assertClientInstanceProjectionRequestsHaveNoExecutionOrDeviceEffects(t, recorder.snapshot()[firstTUIRequest:])

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
		firstHistory.ConversationID != first.ID || len(firstHistory.Prompts) != 2 ||
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
}

func promptPageContains(page model.ConversationPromptPage, content string) bool {
	for _, prompt := range page.Prompts {
		if prompt.Role == "user" && prompt.Content == content {
			return true
		}
	}
	return false
}

// runForgeConsoleClientInstanceSessionProjectionE2EWithToken drives the real
// shared Web/App/Mobile Flutter Sessions Gate against the same authenticated
// candidate mux and real owner Conversations used by this test. The candidate
// is supplied only through this opt-in harness; normal Gate construction
// remains request-free and the production route remains closed.
func runForgeConsoleClientInstanceSessionProjectionE2EWithToken(
	t *testing.T,
	apiURL, token string,
	owner deviceplacement.Owner,
	instanceID string,
	visible, hidden model.Conversation,
	prompt string,
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
		t.Fatalf("Flutter is required for client-instance session projection E2E: %v", err)
	}
	inputJSON, err := json.Marshal(struct {
		APIURL              string                `json:"api_url"`
		AccessToken         string                `json:"access_token"`
		Owner               deviceplacement.Owner `json:"owner"`
		InstanceID          string                `json:"instance_id"`
		VisibleConversation string                `json:"visible_conversation_id"`
		HiddenConversation  string                `json:"hidden_conversation_id"`
		VisibleTitle        string                `json:"visible_title"`
		HiddenTitle         string                `json:"hidden_title"`
		Prompt              string                `json:"prompt"`
	}{
		APIURL: apiURL, AccessToken: token, Owner: owner,
		InstanceID:          instanceID,
		VisibleConversation: visible.ID,
		HiddenConversation:  hidden.ID,
		VisibleTitle:        visible.Title,
		HiddenTitle:         hidden.Title,
		Prompt:              prompt,
	})
	if err != nil {
		t.Fatalf("encode Flutter client-instance session projection input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "client-instance-session-projection-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter client-instance projection input: %v", err)
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
		"test/forge_client_instance_session_projection_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_CLIENT_INSTANCE_SESSION_PROJECTION_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter client-instance session projection E2E failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Flutter client-instance session projection output exceeded the size limit")
	}
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

func runForgeRuntimeClientInstanceProjectionTUI(
	t *testing.T,
	executable, apiURL, accessToken, conversationID string,
) string {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("client-instance projection TUI E2E requires script: %v", err)
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
			"instance client-tui-001\n" +
			"open " + conversationID + "\n" +
			"prompt Prompt submitted from client-instance TUI\n" +
			"quit\n",
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("client-instance projection TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("client-instance projection TUI output exceeded the size limit")
	}
	return stdoutBuffer.String()
}

func assertClientInstanceProjectionRequestsHaveNoExecutionOrDeviceEffects(
	t *testing.T,
	requests []recordedConversationRequest,
) {
	t.Helper()
	for _, request := range requests {
		path := strings.ToLower(request.path)
		forbidden := []string{
			"/api/v1/devices",
			"/api/v1/device-placement",
			"/api/v1/reservation",
			"/api/v1/dispatch",
			"/api/v1/runner",
			"/api/v1/execution",
		}
		for _, fragment := range forbidden {
			if strings.Contains(path, fragment) {
				t.Fatalf("client-instance projection issued forbidden device/execution request: %#v", request)
			}
		}
		if request.path == conversationCollectionPath && strings.Contains(request.query, "instance") {
			t.Fatalf("client-instance projection leaked an instance query into the authenticated session list: %#v", request)
		}
	}
}
