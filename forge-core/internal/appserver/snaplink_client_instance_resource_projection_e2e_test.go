package appserver

// This opt-in acceptance test proves that the composed client-instance/
// resource-view candidate can be the only instance observation consumed by
// Runtime TUI and Snaplink Console. It uses real owner-scoped Conversations
// and Prompt writes, while keeping the candidate route mounted only on a
// test mux and asserting production 404 closure.

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/runtimebridge"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestSnaplinkAuthenticatedClientInstanceResourceProjectionE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_RESOURCE_PROJECTION_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_RESOURCE_PROJECTION_E2E=1 for client-instance resource projection E2E")
	}
	configuredExecutable := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredExecutable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for client-instance resource projection E2E")
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
	owner := deviceplacement.Owner{
		Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate",
	}
	resourceSource := &fixtureClientInstanceResourceViewSource{}
	sessions := newAuthenticatedSessionRoutesWithObservationCandidates(bridge, nil)
	testRoutes := http.NewServeMux()
	testRoutes.Handle(clientInstanceResourceViewCandidatePath,
		newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
			Enabled: true,
			Source:  resourceSource,
		}))
	testRoutes.Handle("/", sessions)
	recorder := &conversationHTTPRecorder{}
	server := httptest.NewServer(authenticator.Handler(recorder.wrap(testRoutes)))
	t.Cleanup(server.Close)

	// The resource candidate carries the same five client declarations as the
	// session candidate, plus one bounded display-only device row. No row is
	// inferred from the bearer or from the Conversation list.
	instances, err := deviceplacement.ObserveClientInstanceSessionView(
		deviceplacement.ClientInstanceSessionViewRequest{
			Owner: owner,
			Instances: []deviceplacement.ClientInstanceSessionViewInstance{
				{InstanceID: "client-mobile-001", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{"conversation-b"}, ObservedAtMS: 200_500, Status: "active"},
				{InstanceID: "client-app-001", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{"conversation-a"}, ObservedAtMS: 200_500, Status: "idle"},
				{InstanceID: "client-web-001", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{"conversation-a"}, ObservedAtMS: 200_500, Status: "active"},
				{InstanceID: "client-tui-001", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{"conversation-b"}, ObservedAtMS: 200_500, Status: "active"},
				{InstanceID: "client-cli-001", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{"conversation-a", "conversation-b"}, ObservedAtMS: 200_500, Status: "active"},
			},
		},
	)
	if err != nil {
		t.Fatalf("build resource projection instances: %v", err)
	}
	const scopes = "forge:conversations:read forge:conversations:write forge:devices:read"
	ownerToken := tokenForIndependentClient(identity, scopes, "resource-projection-http-owner")
	httpClient := &http.Client{Timeout: 20 * time.Second}
	first := createSharedConversationAsClientA(t, httpClient, server.URL, ownerToken)
	second := createProjectionConversation(t, httpClient, server.URL, ownerToken)
	for index := range instances.Instances {
		switch instances.Instances[index].InstanceID {
		case "client-mobile-001", "client-tui-001":
			instances.Instances[index].SessionIDs = []string{second.ID}
		case "client-app-001", "client-web-001":
			instances.Instances[index].SessionIDs = []string{first.ID}
		case "client-cli-001":
			instances.Instances[index].SessionIDs = []string{first.ID, second.ID}
		}
		sort.Strings(instances.Instances[index].SessionIDs)
	}
	resourceView := fixtureClientInstanceResourceView(owner)
	resourceView.Instances = append([]deviceplacement.ClientInstanceSessionViewInstance(nil), instances.Instances...)
	if err := resourceView.Validate(); err != nil {
		t.Fatalf("validate resource projection source: %v", err)
	}
	resourceSource.value = resourceView

	response := doConversationClientRequest(
		t, httpClient, server.URL, ownerToken, http.MethodGet,
		clientInstanceResourceViewCandidatePath, "", "", "",
	)
	if response.StatusCode != http.StatusOK {
		t.Fatalf("resource view candidate status=%d body=%q", response.StatusCode, readConversationClientBody(t, response))
	}
	var served deviceplacement.ClientInstanceResourceViewObservation
	if err := json.NewDecoder(response.Body).Decode(&served); err != nil {
		_ = response.Body.Close()
		t.Fatalf("decode resource view candidate: %v", err)
	}
	_ = response.Body.Close()
	if err := served.Validate(); err != nil || len(served.Instances) != 5 || len(served.Devices) != 1 ||
		served.Instances[0].InstanceID != "client-app-001" || served.Instances[4].InstanceID != "client-web-001" ||
		served.Devices[0].DeviceID != "device-a" || served.Authority != (deviceplacement.ClientInstanceResourceViewAuthority{}) {
		t.Fatalf("served resource projection=%#v err=%v", served, err)
	}

	resourceTUIStart := len(recorder.snapshot())
	tuiToken := tokenForIndependentClient(identity, scopes, "resource-projection-tui")
	tuiOutput := runForgeRuntimeClientInstanceResourceProjectionTUI(
		t, executable, server.URL, tuiToken, second.ID,
	)
	for _, want := range []string{
		"remote client-instance/resource-view [forge.client-instance-resource-view/v1]",
		`Client-instance filter set to "client-tui-001"`,
		"Prompt stored. No Run was started.",
		"Prompt submitted from resource-view TUI",
	} {
		if !strings.Contains(tuiOutput, want) {
			t.Fatalf("resource-view-only TUI output omitted %q: %q", want, tuiOutput)
		}
	}
	resourceTUIRequests := recorder.snapshot()[resourceTUIStart:]
	assertResourceProjectionRequestsHaveNoSessionOrExecutionEffects(t, resourceTUIRequests)

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
			token:      tokenForIndependentClient(identity, scopes, "resource-projection-console-web"),
			visible:    first,
			hidden:     second,
			prompt:     "Prompt submitted from authenticated resource-view Web Console instance",
		},
		{
			name:       "app",
			instanceID: "client-app-001",
			token:      tokenForIndependentClient(identity, scopes, "resource-projection-console-app"),
			visible:    first,
			hidden:     second,
			prompt:     "Prompt submitted from authenticated resource-view desktop App instance",
		},
		{
			name:       "mobile",
			instanceID: "client-mobile-001",
			token:      tokenForIndependentClient(identity, scopes, "resource-projection-console-mobile"),
			visible:    second,
			hidden:     first,
			prompt:     "Prompt submitted from authenticated resource-view Mobile instance",
		},
	}
	for _, projection := range consoleCases {
		projection := projection
		t.Run("console_"+projection.name, func(t *testing.T) {
			resourceConsoleStart := len(recorder.snapshot())
			runForgeConsoleClientInstanceResourceProjectionE2EWithToken(
				t, server.URL, projection.token, owner, projection.instanceID,
				projection.visible, projection.hidden, projection.prompt,
			)
			resourceConsoleRequests := recorder.snapshot()[resourceConsoleStart:]
			assertResourceProjectionRequestsHaveNoSessionOrExecutionEffects(t, resourceConsoleRequests)
		})
	}

	assertResourceProjectionPrompt(t, executable, server.URL, identity, scopes,
		first.ID, "Prompt submitted from authenticated resource-view Web Console instance")
	assertResourceProjectionPrompt(t, executable, server.URL, identity, scopes,
		first.ID, "Prompt submitted from authenticated resource-view desktop App instance")
	assertResourceProjectionPrompt(t, executable, server.URL, identity, scopes,
		second.ID, "Prompt submitted from authenticated resource-view Mobile instance")
	assertResourceProjectionPrompt(t, executable, server.URL, identity, scopes,
		second.ID, "Prompt submitted from resource-view TUI")

	assertResourceProjectionRequestsHaveNoSessionOrExecutionEffects(t, recorder.snapshot())
	production := authenticator.Handler(newConversationRoutes(bridge))
	for _, path := range []string{
		clientInstanceSessionViewCandidatePath,
		clientInstanceResourceViewCandidatePath,
	} {
		request := httptest.NewRequest(http.MethodGet, path, nil)
		request.Header.Set("Authorization", "Bearer "+ownerToken)
		productionResponse := httptest.NewRecorder()
		production.ServeHTTP(productionResponse, request)
		if productionResponse.Code != http.StatusNotFound {
			t.Fatalf("production client-instance route %s status=%d body=%q", path, productionResponse.Code, productionResponse.Body.String())
		}
	}
}

func runForgeRuntimeClientInstanceResourceProjectionTUI(
	t *testing.T,
	executable, apiURL, accessToken, conversationID string,
) string {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("resource-view projection TUI E2E requires script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader(
		"client-instances resource-view\n" +
			"instance client-tui-001\n" +
			"open " + conversationID + "\n" +
			"prompt Prompt submitted from resource-view TUI\n" +
			"quit\n",
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("resource-view-only TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("resource-view-only TUI output exceeded the size limit")
	}
	return stdoutBuffer.String()
}

func runForgeConsoleClientInstanceResourceProjectionE2EWithToken(
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
		t.Fatalf("Flutter is required for resource-view projection E2E: %v", err)
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
		InstanceID: instanceID, VisibleConversation: visible.ID,
		HiddenConversation: hidden.ID, VisibleTitle: visible.Title,
		HiddenTitle: hidden.Title,
		Prompt:      prompt,
	})
	if err != nil {
		t.Fatalf("encode Flutter resource projection input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "client-instance-resource-projection-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter resource projection input: %v", err)
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
		"test/forge_client_instance_resource_projection_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_CLIENT_INSTANCE_RESOURCE_PROJECTION_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter resource-view projection E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Flutter resource-view projection output exceeded the size limit")
	}
}

func assertResourceProjectionPrompt(
	t *testing.T,
	executable, apiURL string,
	identity *conversationTestIdentity,
	scopes, conversationID, expected string,
) {
	t.Helper()
	output, stderr, err := runForgeRuntimeCLI(
		t, executable, apiURL,
		tokenForIndependentClient(identity, scopes, "resource-projection-reader-"+conversationID),
		t.TempDir(), "--json", "remote", "prompts", "list", conversationID,
	)
	if err != nil {
		t.Fatalf("resource projection Prompt read failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var history model.ConversationPromptPage
	if err := json.Unmarshal([]byte(output), &history); err != nil || history.ConversationID != conversationID {
		t.Fatalf("resource projection Prompt history=%#v stdout=%q decode=%v", history, output, err)
	}
	for _, prompt := range history.Prompts {
		if prompt.Content == expected {
			return
		}
	}
	t.Fatalf("resource projection Prompt %q was not visible in conversation %q: %#v", expected, conversationID, history)
}

func assertResourceProjectionRequestsHaveNoSessionOrExecutionEffects(
	t *testing.T,
	requests []recordedConversationRequest,
) {
	t.Helper()
	for _, request := range requests {
		path := strings.ToLower(request.path)
		if request.path == clientInstanceSessionViewCandidatePath {
			t.Fatalf("resource-view-only consumer requested session-view: %#v", request)
		}
		for _, fragment := range []string{
			"/api/v1/devices",
			"/api/v1/device-placement",
			"/api/v1/reservation",
			"/api/v1/dispatch",
			"/api/v1/runner",
			"/api/v1/execution",
		} {
			if strings.Contains(path, fragment) {
				t.Fatalf("resource-view projection issued forbidden device/execution request: %#v", request)
			}
		}
		if request.path == conversationCollectionPath && strings.Contains(request.query, "instance") {
			t.Fatalf("resource-view projection leaked an instance query into the authenticated session list: %#v", request)
		}
	}
}
