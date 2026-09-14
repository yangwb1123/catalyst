package appserver

import (
	"bytes"
	"context"
	"crypto/ed25519"
	"encoding/base64"
	"encoding/json"
	"errors"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"

	"forgeos/forge-core/internal/runtimebridge"
)

func TestIndependentClientsShareOwnedConversationAndPrompts(t *testing.T) {
	configuredExecutable := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredExecutable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for two-client HTTP to Rust Hub integration")
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
	sessions := authenticator.Handler(newConversationRoutes(bridge))
	recorder := &conversationHTTPRecorder{}
	server := httptest.NewUnstartedServer(http.NotFoundHandler())
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test", Commit: "test"},
		server.Listener.Addr().String(), maxInFlightRequests, sessions,
	)
	if err != nil {
		server.Close()
		t.Fatal(err)
	}
	server.Config.Handler = recorder.wrap(routes)
	if os.Getenv("FORGE_BROWSER_E2E") == "1" {
		webBuildDir := os.Getenv("FORGE_WEB_BUILD_DIR")
		if webBuildDir == "" {
			server.Close()
			t.Fatal("FORGE_WEB_BUILD_DIR is required when FORGE_BROWSER_E2E=1")
		}
		server.Config.Handler = serveForgeConsoleWebAssets(webBuildDir, recorder.wrap(routes))
	}
	server.Start()
	t.Cleanup(server.Close)

	clientA := &http.Client{Timeout: 20 * time.Second}
	clientB := &http.Client{Timeout: 20 * time.Second}
	tokenA := tokenForIndependentClient(identity, "forge:conversations:read forge:conversations:write", "client-a")
	tokenB := tokenForIndependentClient(identity, "forge:conversations:read forge:conversations:write", "client-b")
	if tokenA == tokenB {
		t.Fatal("independent clients must use distinct access tokens")
	}
	created := createSharedConversationAsClientA(t, clientA, server.URL, tokenA)
	assertConversationVisibleToClientB(t, clientB, server.URL, tokenB, created)
	promptPath := conversationCollectionPath + "/" + created.ID + "/prompts"
	appendPromptAsClientB(t, clientB, server.URL, tokenB, promptPath)
	assertPromptVisibleToClientA(t, clientA, server.URL, tokenA, promptPath, created.ID)

	cliChangesAfter := uint64(2)
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		firstConsoleRequest := len(recorder.snapshot())
		runForgeConsoleLiveAPITest(t, recorder, server.URL, identity, created.ID)
		consoleRequests := recorder.snapshot()[firstConsoleRequest:]
		assertForgeConsoleAPIAndWidgetRequestsHaveNoExecutionOrDeviceEffects(t, consoleRequests, created.ID)
		assertForgeConsolePromptVisibleToGoClient(t, clientA, server.URL, tokenA, promptPath, created.ID)
		cliChangesAfter = 4
	}
	firstCLIRequest := len(recorder.snapshot())
	cliConversation := assertForgeRuntimeCLIClientsShareOwnedConversationAndPrompts(t, executable, server.URL, identity, cliChangesAfter)
	assertForgeRuntimeCLIRequestsHaveNoExecutionOrDeviceEffects(
		t, recorder.snapshot()[firstCLIRequest:], cliChangesAfter,
	)
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		firstTUIRequest := len(recorder.snapshot())
		runForgeRuntimeTUI(t, executable, server.URL, tokenForIndependentClient(
			identity, "forge:conversations:read forge:conversations:write", "tui-client",
		), created.ID)
		tuiRequests := recorder.snapshot()[firstTUIRequest:]
		assertForgeRuntimeTUIRequestsHaveNoExecutionOrDeviceEffects(t, tuiRequests, created.ID)
		assertForgeTUIWriteVisibleToGoClient(t, clientA, server.URL, tokenA, promptPath, created.ID)
	}
	if os.Getenv("FORGE_BROWSER_E2E") == "1" {
		firstBrowserRequest := len(recorder.snapshot())
		runForgeConsoleBrowserE2E(t, server.URL, identity, created.ID)
		browserRequests := recorder.snapshot()[firstBrowserRequest:]
		assertForgeConsoleBrowserRequestsHaveNoExecutionOrDeviceEffects(t, browserRequests, created.ID)
		assertForgeBrowserWriteVisibleToGoClient(t, clientA, server.URL, tokenA, promptPath, created.ID)
	}
	assertNoPendingRunIntentAfterPrompt(t, bridge, identity, cliConversation.ID)
	assertNoPendingRunIntentAfterPrompt(t, bridge, identity, created.ID)
}

func assertForgeRuntimeCLIClientsShareOwnedConversationAndPrompts(
	t *testing.T,
	executable, apiURL string,
	identity *conversationTestIdentity,
	changesAfter uint64,
) model.Conversation {
	t.Helper()
	const scopes = "forge:conversations:read forge:conversations:write"
	tokenA := tokenForIndependentClient(identity, scopes, "cli-client-a")
	tokenB := tokenForIndependentClient(identity, scopes, "cli-client-b")
	tokenC := tokenForPrincipal(identity, "account-foreign", scopes, "cli-client-c")
	clientAHome := t.TempDir()
	clientBHome := t.TempDir()
	clientCHome := t.TempDir()

	createOutput, createStderr, err := runForgeRuntimeCLI(t, executable, apiURL, tokenA, clientAHome,
		"--json", "--idempotency-key", "cli-client-a-create",
		"remote", "sessions", "create", "--scope", "global", "--title", "Shared from CLI client A")
	if err != nil {
		t.Fatalf("CLI client A create failed: stderr=%q stdout=%q err=%v", createStderr, createOutput, err)
	}
	var created model.Conversation
	if err := json.Unmarshal([]byte(createOutput), &created); err != nil || created.ID == "" || created.Title != "Shared from CLI client A" {
		t.Fatalf("CLI client A create response=%q conversation=%#v decode=%v", createOutput, created, err)
	}

	listOutput, listStderr, err := runForgeRuntimeCLI(t, executable, apiURL, tokenB, clientBHome,
		"--json", "remote", "sessions", "list")
	if err != nil {
		t.Fatalf("CLI client B list failed: stderr=%q stdout=%q err=%v", listStderr, listOutput, err)
	}
	var page model.OwnedConversationPage
	if err := json.Unmarshal([]byte(listOutput), &page); err != nil || len(page.Conversations) != 2 || page.HasMore {
		t.Fatalf("CLI client B owner page=%#v stdout=%q decode=%v", page, listOutput, err)
	}
	if !ownedConversationPageContains(page, created.ID) {
		t.Fatalf("CLI client B could not see CLI client A Conversation %q: %#v", created.ID, page)
	}

	promptContent := "Prompt submitted by remote CLI client B"
	appendOutput, appendStderr, err := runForgeRuntimeCLI(t, executable, apiURL, tokenB, clientBHome,
		"--json", "--idempotency-key", "cli-client-b-prompt",
		"remote", "prompts", "add", created.ID, "--expected-version", "1", promptContent)
	if err != nil {
		t.Fatalf("CLI client B Prompt append failed: stderr=%q stdout=%q err=%v", appendStderr, appendOutput, err)
	}

	historyOutput, historyStderr, err := runForgeRuntimeCLI(t, executable, apiURL, tokenA, clientAHome,
		"--json", "remote", "prompts", "list", created.ID)
	if err != nil {
		t.Fatalf("CLI client A Prompt history read failed: stderr=%q stdout=%q err=%v", historyStderr, historyOutput, err)
	}
	var history model.ConversationPromptPage
	if err := json.Unmarshal([]byte(historyOutput), &history); err != nil || history.ConversationID != created.ID ||
		len(history.Prompts) != 1 || history.Prompts[0].Role != "user" || history.Prompts[0].Content != promptContent {
		t.Fatalf("CLI client A Prompt history=%#v stdout=%q decode=%v", history, historyOutput, err)
	}

	changesOutput, changesStderr, err := runForgeRuntimeCLI(t, executable, apiURL, tokenA, clientAHome,
		"--json", "remote", "changes", "list", "--after-cursor", strconv.FormatUint(changesAfter, 10))
	if err != nil {
		t.Fatalf("CLI client A change feed read failed: stderr=%q stdout=%q err=%v", changesStderr, changesOutput, err)
	}
	var changes model.OwnedConversationChangePage
	if err := json.Unmarshal([]byte(changesOutput), &changes); err != nil || changes.AfterCursor != changesAfter ||
		changes.ScannedThroughCursor != changesAfter+2 || changes.HasMore || len(changes.Changes) != 2 ||
		changes.Changes[0].ConversationID != created.ID || changes.Changes[0].Kind != "conversation_created" ||
		changes.Changes[1].ConversationID != created.ID || changes.Changes[1].Kind != "prompt_appended" {
		t.Fatalf("CLI client A change feed=%#v stdout=%q decode=%v", changes, changesOutput, err)
	}

	foreignListOutput, foreignListStderr, err := runForgeRuntimeCLI(t, executable, apiURL, tokenC, clientCHome,
		"--json", "remote", "sessions", "list")
	if err != nil {
		t.Fatalf("foreign CLI client list failed: stderr=%q stdout=%q err=%v", foreignListStderr, foreignListOutput, err)
	}
	var foreignPage model.OwnedConversationPage
	if err := json.Unmarshal([]byte(foreignListOutput), &foreignPage); err != nil || len(foreignPage.Conversations) != 0 || foreignPage.HasMore {
		t.Fatalf("foreign CLI client received owner data: page=%#v stdout=%q decode=%v", foreignPage, foreignListOutput, err)
	}
	foreignHistoryOutput, foreignHistoryStderr, err := runForgeRuntimeCLI(t, executable, apiURL, tokenC, clientCHome,
		"--json", "remote", "prompts", "list", created.ID)
	if err == nil || !strings.Contains(foreignHistoryStderr, "HTTP 404 (not_found)") {
		t.Fatalf("foreign CLI client Conversation read should be hidden: stderr=%q stdout=%q err=%v",
			foreignHistoryStderr, foreignHistoryOutput, err)
	}

	runsOutput, runsStderr, err := runForgeRuntimeCLI(t, executable, apiURL, tokenA, clientAHome,
		"--json", "remote", "runs", "list", created.ID, "--limit", "25")
	if err != nil {
		t.Fatalf("CLI client A Run observation failed: stderr=%q stdout=%q err=%v", runsStderr, runsOutput, err)
	}
	var runPage runmodel.OwnedRunPage
	if err := json.Unmarshal([]byte(runsOutput), &runPage); err != nil || runPage.ConversationID != created.ID ||
		runPage.Runs == nil || len(runPage.Runs) != 0 || runPage.HasMore {
		t.Fatalf("Prompt append unexpectedly started a Run: page=%#v stdout=%q decode=%v", runPage, runsOutput, err)
	}
	return created
}

func assertNoPendingRunIntentAfterPrompt(
	t *testing.T,
	client *runtimebridge.Client,
	identity *conversationTestIdentity,
	conversationID string,
) {
	t.Helper()
	owner := model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	page, err := client.OwnedConversationPendingRunIntents(context.Background(), owner, conversationID, nil, 25)
	if err != nil || page.ConversationID != conversationID || len(page.Intents) != 0 || page.NextCursor != nil || page.HasMore {
		t.Fatalf("storage-only Prompt created a pending Run intent: page=%#v err=%v", page, err)
	}
}

func ownedConversationPageContains(page model.OwnedConversationPage, conversationID string) bool {
	for _, entry := range page.Conversations {
		if entry.Conversation.ID == conversationID {
			return true
		}
	}
	return false
}

func runForgeRuntimeCLI(
	t *testing.T,
	executable, apiURL, accessToken, home string,
	args ...string,
) (stdout, stderr string, err error) {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, executable, args...)
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	err = command.Run()
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		return stdoutBuffer.String(), stderrBuffer.String(), errors.New("forge-runtime CLI output exceeded the size limit")
	}
	return stdoutBuffer.String(), stderrBuffer.String(), err
}

func runForgeRuntimeTUI(t *testing.T, executable, apiURL, accessToken, conversationID string) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("the live TUI integration requires a PTY launcher named script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("open " + conversationID + "\nprompt " + forgeRuntimeTUIPromptContent + "\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Forge remote TUI failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Forge remote TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"Shared from CLI client A",
		"prompt sent from client B",
		"Prompt stored. No Run was started.",
		forgeRuntimeTUIPromptContent,
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("Forge remote TUI output omitted %q: %q", want, output)
		}
	}
}

func forgeRuntimeCLIEnvironment(apiURL, accessToken, home string) []string {
	env := make([]string, 0, 8)
	for _, value := range os.Environ() {
		key, _, _ := strings.Cut(value, "=")
		switch strings.ToUpper(key) {
		case "PATH", "SYSTEMROOT", "WINDIR", "TMP", "TEMP", "TMPDIR":
			env = append(env, value)
		}
	}
	return append(env,
		"FORGE_API_URL="+apiURL,
		"FORGE_ACCESS_TOKEN="+accessToken,
		"HOME="+home,
		"USERPROFILE="+home,
		"XDG_CONFIG_HOME="+filepath.Join(home, "config"),
		"XDG_STATE_HOME="+filepath.Join(home, "state"),
	)
}

const maxForgeRuntimeCLIOutputBytes = 2 * 1024 * 1024
const forgeConsoleWidgetPromptContent = "Prompt submitted from Flutter Console screen"
const forgeRuntimeTUIPromptContent = "Prompt submitted from Forge remote TUI"
const forgeConsoleBrowserPromptContent = "Prompt submitted from Forge Console browser"

type boundedCLIOutput struct {
	buffer   bytes.Buffer
	exceeded bool
}

func (output *boundedCLIOutput) Write(value []byte) (int, error) {
	remaining := maxForgeRuntimeCLIOutputBytes - output.buffer.Len()
	if remaining <= 0 {
		output.exceeded = true
		return len(value), nil
	}
	if len(value) > remaining {
		_, _ = output.buffer.Write(value[:remaining])
		output.exceeded = true
		return len(value), nil
	}
	return output.buffer.Write(value)
}

func (output *boundedCLIOutput) String() string {
	return output.buffer.String()
}

type recordedConversationRequest struct {
	method string
	path   string
	query  string
}

type conversationHTTPRecorder struct {
	mu       sync.Mutex
	requests []recordedConversationRequest
}

func (recorder *conversationHTTPRecorder) wrap(next http.Handler) http.Handler {
	return http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		recorder.mu.Lock()
		recorder.requests = append(recorder.requests, recordedConversationRequest{
			method: request.Method,
			path:   request.URL.EscapedPath(),
			query:  request.URL.RawQuery,
		})
		recorder.mu.Unlock()
		next.ServeHTTP(writer, request)
	})
}

func (recorder *conversationHTTPRecorder) snapshot() []recordedConversationRequest {
	recorder.mu.Lock()
	defer recorder.mu.Unlock()
	return append([]recordedConversationRequest(nil), recorder.requests...)
}

func assertForgeRuntimeCLIRequestsHaveNoExecutionOrDeviceEffects(
	t *testing.T,
	requests []recordedConversationRequest,
	changesAfter uint64,
) {
	t.Helper()
	if len(requests) != 8 {
		t.Fatalf("CLI issued %d HTTP requests; expected only the 8 shared-session/read-only verification calls: %#v", len(requests), requests)
	}
	conversationID, ok := conversationIDFromPromptPath(requests[2].path)
	if !ok {
		t.Fatalf("CLI append request path is invalid: %#v", requests[2])
	}
	want := []recordedConversationRequest{
		{method: http.MethodPost, path: conversationCollectionPath},
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodPost, path: conversationCollectionPath + "/" + conversationID + "/prompts"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/prompts", query: "limit=128"},
		{method: http.MethodGet, path: conversationChangesPath, query: "after_cursor=" + strconv.FormatUint(changesAfter, 10) + "&limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/prompts", query: "limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/runs", query: "limit=25"},
	}
	for index, request := range requests {
		if request != want[index] {
			t.Fatalf("CLI request[%d]=%#v want=%#v (device and Run effect routes must remain untouched)", index, request, want[index])
		}
	}
}

func assertForgeRuntimeTUIRequestsHaveNoExecutionOrDeviceEffects(
	t *testing.T,
	requests []recordedConversationRequest,
	conversationID string,
) {
	t.Helper()
	promptPath := conversationCollectionPath + "/" + conversationID + "/prompts"
	want := []recordedConversationRequest{
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: promptPath, query: "limit=128"},
		{method: http.MethodPost, path: promptPath},
		{method: http.MethodGet, path: promptPath, query: "limit=128"},
	}
	if len(requests) != len(want) {
		t.Fatalf("TUI issued %d HTTP requests; expected only the 4 shared-session calls: %#v", len(requests), requests)
	}
	for index, request := range requests {
		if request != want[index] {
			t.Fatalf("TUI request[%d]=%#v want=%#v (Run effect and device routes must remain untouched)", index, request, want[index])
		}
	}
}

func runForgeConsoleLiveAPITest(
	t *testing.T,
	recorder *conversationHTTPRecorder,
	apiURL string,
	identity *conversationTestIdentity,
	conversationID string,
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
		t.Fatalf("Flutter is required when FORGE_CONSOLE_E2E=1: %v", err)
	}

	const scopes = "forge:conversations:read forge:conversations:write"
	token := tokenForPrincipalWithTTL(identity, "account-42", scopes, "console-client", 15*time.Minute)
	input := struct {
		APIURL          string `json:"api_url"`
		AccessToken     string `json:"access_token"`
		ConversationID  string `json:"conversation_id"`
		ExpectedVersion int    `json:"expected_version"`
		IdempotencyKey  string `json:"idempotency_key"`
		Prompt          string `json:"prompt"`
		WidgetPrompt    string `json:"widget_prompt"`
		ExistingPrompt  string `json:"existing_prompt"`
		AfterCursor     int    `json:"after_cursor"`
	}{
		APIURL: apiURL, AccessToken: token, ConversationID: conversationID,
		ExpectedVersion: 2, IdempotencyKey: "console-client-prompt",
		Prompt:         "Prompt submitted by Flutter Console API client",
		WidgetPrompt:   forgeConsoleWidgetPromptContent,
		ExistingPrompt: "prompt sent from client B", AfterCursor: 2,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter integration input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "console-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter integration input: %v", err)
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
		"test/forge_coordinator_live_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = forgeConsoleTestEnvironment(inputPath, flutterHome)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	err = command.Run()
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Flutter Console integration test output exceeded the size limit")
	}
	if err != nil {
		t.Fatalf("Flutter Console gate/widget/API integration failed: stdout=%q stderr=%q requests=%#v err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), recorder.snapshot(), err)
	}
}

func runForgeConsoleBrowserE2E(
	t *testing.T,
	apiURL string,
	identity *conversationTestIdentity,
	conversationID string,
) {
	t.Helper()
	pythonBinary := os.Getenv("FORGE_BROWSER_PYTHON")
	if pythonBinary == "" {
		pythonBinary = "python3"
	}
	pythonExecutable, err := exec.LookPath(pythonBinary)
	if err != nil {
		t.Fatalf("Python is required when FORGE_BROWSER_E2E=1: %v", err)
	}
	workingDirectory, err := os.Getwd()
	if err != nil {
		t.Fatalf("resolve Forge Core repository: %v", err)
	}
	repoRoot := filepath.Clean(filepath.Join(workingDirectory, "..", "..", ".."))
	runnerPath := filepath.Join(repoRoot, "scripts", "forge_console_browser_e2e.py")
	if _, err := os.Stat(runnerPath); err != nil {
		t.Fatalf("Forge Console browser E2E runner not found at %s: %v", runnerPath, err)
	}
	const scopes = "forge:conversations:read forge:conversations:write"
	token := tokenForPrincipalWithTTL(identity, "account-42", scopes, "browser-client", 15*time.Minute)
	input := struct {
		PageURL        string `json:"page_url"`
		AccessToken    string `json:"access_token"`
		ConversationID string `json:"conversation_id"`
		Prompt         string `json:"prompt"`
	}{
		PageURL: apiURL + "/forge/", AccessToken: token,
		ConversationID: conversationID, Prompt: forgeConsoleBrowserPromptContent,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Forge browser integration input: %v", err)
	}
	inputPath := filepath.Join(t.TempDir(), "forge-browser-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Forge browser integration input: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, pythonExecutable, runnerPath, inputPath)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	err = command.Run()
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Forge Console browser test output exceeded the size limit")
	}
	if err != nil {
		t.Fatalf("Forge Console browser E2E failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
}

func serveForgeConsoleWebAssets(webBuildDir string, apiRoutes http.Handler) http.Handler {
	staticFiles := http.StripPrefix("/forge/", http.FileServer(http.Dir(webBuildDir)))
	indexPath := filepath.Join(webBuildDir, "index.html")
	return http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		if request.URL.Path == "/forge" {
			http.Redirect(writer, request, "/forge/", http.StatusPermanentRedirect)
			return
		}
		if request.URL.Path == "/forge/" {
			http.ServeFile(writer, request, indexPath)
			return
		}
		if strings.HasPrefix(request.URL.Path, "/forge/") {
			relativePath := filepath.Clean(filepath.FromSlash(strings.TrimPrefix(request.URL.Path, "/forge/")))
			if relativePath == "." || relativePath == ".." || strings.HasPrefix(relativePath, ".."+string(filepath.Separator)) {
				http.NotFound(writer, request)
				return
			}
			assetPath := filepath.Join(webBuildDir, relativePath)
			if info, err := os.Stat(assetPath); err == nil && !info.IsDir() {
				staticFiles.ServeHTTP(writer, request)
				return
			}
			if filepath.Ext(relativePath) == "" {
				http.ServeFile(writer, request, indexPath)
				return
			}
			http.NotFound(writer, request)
			return
		}
		apiRoutes.ServeHTTP(writer, request)
	})
}

func forgeConsoleTestEnvironment(inputPath, home string) []string {
	env := make([]string, 0, 12)
	for _, value := range os.Environ() {
		key, _, _ := strings.Cut(value, "=")
		switch strings.ToUpper(key) {
		case "PATH", "SYSTEMROOT", "WINDIR", "TMP", "TEMP", "TMPDIR", "LANG", "LC_ALL":
			env = append(env, value)
		}
	}
	return append(env,
		"CI=true",
		"FLUTTER_SUPPRESS_ANALYTICS=true",
		"HOME="+home,
		"USERPROFILE="+home,
		"XDG_CONFIG_HOME="+filepath.Join(home, "config"),
		"XDG_CACHE_HOME="+filepath.Join(home, "cache"),
		"FORGE_CONSOLE_E2E_INPUT="+inputPath,
	)
}

func assertForgeConsoleAPIAndWidgetRequestsHaveNoExecutionOrDeviceEffects(
	t *testing.T,
	requests []recordedConversationRequest,
	conversationID string,
) {
	t.Helper()
	if len(requests) != 8 {
		t.Fatalf("Console issued %d HTTP requests; expected 4 API-service and 4 screen calls: %#v", len(requests), requests)
	}
	promptPath := conversationCollectionPath + "/" + conversationID + "/prompts"
	want := map[recordedConversationRequest]int{
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=50"}:                                  2,
		{method: http.MethodGet, path: promptPath, query: "limit=100"}:                                                 2,
		{method: http.MethodPost, path: promptPath}:                                                                    2,
		{method: http.MethodGet, path: conversationChangesPath, query: "after_cursor=2&limit=128"}:                     1,
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/runs", query: "limit=25"}: 1,
	}
	got := make(map[recordedConversationRequest]int, len(want))
	for _, request := range requests {
		got[request]++
	}
	for request, count := range want {
		if got[request] != count {
			t.Fatalf("Console request %#v count=%d want=%d (only one read-only Run page is allowed)", request, got[request], count)
		}
	}
	if len(got) != len(want) {
		for request := range got {
			if _, ok := want[request]; !ok {
				t.Fatalf("unexpected Console API path %s %s?%s (device and effect routes must remain untouched)", request.method, request.path, request.query)
			}
		}
	}
}

func assertForgeConsolePromptVisibleToGoClient(
	t *testing.T,
	client *http.Client,
	baseURL, token, promptPath, conversationID string,
) {
	t.Helper()
	historyResponse := doConversationClientRequest(t, client, baseURL, token,
		http.MethodGet, promptPath+"?limit=100", "", "", "")
	if historyResponse.StatusCode != http.StatusOK {
		t.Fatalf("Go client Console history status=%d body=%q", historyResponse.StatusCode, readConversationClientBody(t, historyResponse))
	}
	var history model.ConversationPromptPage
	if err := json.NewDecoder(historyResponse.Body).Decode(&history); err != nil {
		_ = historyResponse.Body.Close()
		t.Fatalf("decode Go client Console history: %v", err)
	}
	_ = historyResponse.Body.Close()
	if history.ConversationID != conversationID || len(history.Prompts) != 3 ||
		!promptContentIsPresent(history.Prompts, "prompt sent from client B") ||
		!promptContentIsPresent(history.Prompts, "Prompt submitted by Flutter Console API client") ||
		!promptContentIsPresent(history.Prompts, forgeConsoleWidgetPromptContent) {
		t.Fatalf("Go client could not read the Flutter Console Prompt from Rust Hub: %#v", history)
	}
}

func assertForgeTUIWriteVisibleToGoClient(
	t *testing.T,
	client *http.Client,
	baseURL, token, promptPath, conversationID string,
) {
	t.Helper()
	historyResponse := doConversationClientRequest(t, client, baseURL, token,
		http.MethodGet, promptPath+"?limit=100", "", "", "")
	if historyResponse.StatusCode != http.StatusOK {
		t.Fatalf("Go client TUI history status=%d body=%q", historyResponse.StatusCode, readConversationClientBody(t, historyResponse))
	}
	var history model.ConversationPromptPage
	if err := json.NewDecoder(historyResponse.Body).Decode(&history); err != nil {
		_ = historyResponse.Body.Close()
		t.Fatalf("decode Go client TUI history: %v", err)
	}
	_ = historyResponse.Body.Close()
	if history.ConversationID != conversationID || len(history.Prompts) != 4 ||
		!promptContentIsPresent(history.Prompts, "prompt sent from client B") ||
		!promptContentIsPresent(history.Prompts, "Prompt submitted by Flutter Console API client") ||
		!promptContentIsPresent(history.Prompts, forgeConsoleWidgetPromptContent) ||
		!promptContentIsPresent(history.Prompts, forgeRuntimeTUIPromptContent) {
		t.Fatalf("Go client could not read the TUI Prompt from Rust Hub: %#v", history)
	}
}

func assertForgeConsoleBrowserRequestsHaveNoExecutionOrDeviceEffects(
	t *testing.T,
	requests []recordedConversationRequest,
	conversationID string,
) {
	t.Helper()
	promptPath := conversationCollectionPath + "/" + conversationID + "/prompts"
	required := map[recordedConversationRequest]int{
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=50"}: 1,
		{method: http.MethodPost, path: promptPath}:                                   1,
	}
	got := make(map[recordedConversationRequest]int, len(required))
	targetPromptReads := 0
	targetRunReads := 0
	for _, request := range requests {
		switch {
		case request == (recordedConversationRequest{method: http.MethodGet, path: conversationCollectionPath, query: "limit=50"}),
			request == (recordedConversationRequest{method: http.MethodPost, path: promptPath}):
			got[request]++
		case request.method == http.MethodGet && request.query == "limit=100":
			readConversationID, ok := conversationIDFromPromptPath(request.path)
			if !ok {
				t.Fatalf("unexpected Forge browser Prompt-history request %#v", request)
			}
			if readConversationID == conversationID {
				targetPromptReads++
			}
		case request.method == http.MethodGet && request.query == "limit=25":
			readConversationID, ok := conversationIDFromRunPath(request.path)
			if !ok {
				t.Fatalf("unexpected Forge browser Run-page request %#v", request)
			}
			if readConversationID == conversationID {
				targetRunReads++
			}
		case request.path == conversationChangesPath:
			parts := strings.Split(request.query, "&")
			if request.method != http.MethodGet || len(parts) != 2 ||
				!strings.HasPrefix(parts[0], "after_cursor=") || parts[1] != "limit=128" {
				t.Fatalf("unexpected Forge browser change-feed request %#v", request)
			}
			if _, err := strconv.ParseUint(strings.TrimPrefix(parts[0], "after_cursor="), 10, 64); err != nil {
				t.Fatalf("invalid Forge browser change-feed cursor in request %#v: %v", request, err)
			}
		default:
			t.Fatalf("unexpected Forge browser API request %#v (Run writes, devices, and scheduling are forbidden)", request)
		}
	}
	if targetPromptReads == 0 || targetRunReads == 0 {
		t.Fatalf("Forge browser did not read target session Prompt/Run pages: prompts=%d runs=%d requests=%#v",
			targetPromptReads, targetRunReads, requests)
	}
	for request, minimum := range required {
		if got[request] < minimum {
			t.Fatalf("Forge browser request %#v count=%d want at least %d; observed %#v", request, got[request], minimum, requests)
		}
	}
}

func assertForgeBrowserWriteVisibleToGoClient(
	t *testing.T,
	client *http.Client,
	baseURL, token, promptPath, conversationID string,
) {
	t.Helper()
	historyResponse := doConversationClientRequest(t, client, baseURL, token,
		http.MethodGet, promptPath+"?limit=100", "", "", "")
	if historyResponse.StatusCode != http.StatusOK {
		t.Fatalf("Go client browser history status=%d body=%q", historyResponse.StatusCode, readConversationClientBody(t, historyResponse))
	}
	var history model.ConversationPromptPage
	if err := json.NewDecoder(historyResponse.Body).Decode(&history); err != nil {
		_ = historyResponse.Body.Close()
		t.Fatalf("decode Go client browser history: %v", err)
	}
	_ = historyResponse.Body.Close()
	if history.ConversationID != conversationID || len(history.Prompts) != 5 ||
		!promptContentIsPresent(history.Prompts, "prompt sent from client B") ||
		!promptContentIsPresent(history.Prompts, "Prompt submitted by Flutter Console API client") ||
		!promptContentIsPresent(history.Prompts, forgeConsoleWidgetPromptContent) ||
		!promptContentIsPresent(history.Prompts, forgeRuntimeTUIPromptContent) ||
		!promptContentIsPresent(history.Prompts, forgeConsoleBrowserPromptContent) {
		t.Fatalf("Go client could not read the Forge browser Prompt from Rust Hub: %#v", history)
	}
}

func promptContentIsPresent(prompts []model.ConversationPrompt, content string) bool {
	for _, prompt := range prompts {
		if prompt.Role == "user" && prompt.Content == content {
			return true
		}
	}
	return false
}

func conversationIDFromPromptPath(path string) (string, bool) {
	prefix := conversationCollectionPath + "/"
	suffix := "/prompts"
	if !strings.HasPrefix(path, prefix) || !strings.HasSuffix(path, suffix) {
		return "", false
	}
	id := strings.TrimSuffix(strings.TrimPrefix(path, prefix), suffix)
	return id, id != "" && !strings.Contains(id, "/")
}

func conversationIDFromRunPath(path string) (string, bool) {
	prefix := conversationCollectionPath + "/"
	suffix := "/runs"
	if !strings.HasPrefix(path, prefix) || !strings.HasSuffix(path, suffix) {
		return "", false
	}
	id := strings.TrimSuffix(strings.TrimPrefix(path, prefix), suffix)
	return id, id != "" && !strings.Contains(id, "/")
}

func createSharedConversationAsClientA(
	t *testing.T,
	client *http.Client,
	baseURL, token string,
) model.Conversation {
	t.Helper()
	createdResponse := doConversationClientRequest(t, client, baseURL, token,
		http.MethodPost, conversationCollectionPath, "application/json", "client-a-create",
		`{"scope":{"kind":"global"},"title":"Shared from client A"}`)
	if createdResponse.StatusCode != http.StatusCreated {
		t.Fatalf("client A create status=%d body=%q", createdResponse.StatusCode, readConversationClientBody(t, createdResponse))
	}
	var created model.Conversation
	if err := json.NewDecoder(createdResponse.Body).Decode(&created); err != nil || created.ID == "" {
		t.Fatalf("client A create response=%#v decode=%v", created, err)
	}
	_ = createdResponse.Body.Close()
	return created
}

func assertConversationVisibleToClientB(
	t *testing.T,
	client *http.Client,
	baseURL, token string,
	created model.Conversation,
) {
	t.Helper()
	listResponse := doConversationClientRequest(t, client, baseURL, token,
		http.MethodGet, conversationCollectionPath+"?limit=50", "", "", "")
	if listResponse.StatusCode != http.StatusOK {
		t.Fatalf("client B list status=%d body=%q", listResponse.StatusCode, readConversationClientBody(t, listResponse))
	}
	var page model.OwnedConversationPage
	if err := json.NewDecoder(listResponse.Body).Decode(&page); err != nil || len(page.Conversations) != 1 ||
		page.Conversations[0].Conversation.ID != created.ID {
		t.Fatalf("client B list=%#v decode=%v", page, err)
	}
	_ = listResponse.Body.Close()
}

func appendPromptAsClientB(t *testing.T, client *http.Client, baseURL, token, promptPath string) {
	t.Helper()
	appendResponse := doConversationClientRequest(t, client, baseURL, token,
		http.MethodPost, promptPath, "application/json", "client-b-prompt",
		`{"content":"prompt sent from client B","expected_version":1}`)
	if appendResponse.StatusCode != http.StatusCreated {
		t.Fatalf("client B append status=%d body=%q", appendResponse.StatusCode, readConversationClientBody(t, appendResponse))
	}
	_ = appendResponse.Body.Close()
}

func assertPromptVisibleToClientA(
	t *testing.T,
	client *http.Client,
	baseURL, token, promptPath, conversationID string,
) {
	t.Helper()
	historyResponse := doConversationClientRequest(t, client, baseURL, token,
		http.MethodGet, promptPath, "", "", "")
	if historyResponse.StatusCode != http.StatusOK {
		t.Fatalf("client A history status=%d body=%q", historyResponse.StatusCode, readConversationClientBody(t, historyResponse))
	}
	var history model.ConversationPromptPage
	if err := json.NewDecoder(historyResponse.Body).Decode(&history); err != nil ||
		history.ConversationID != conversationID || len(history.Prompts) != 1 ||
		history.Prompts[0].Content != "prompt sent from client B" {
		t.Fatalf("client A history=%#v decode=%v", history, err)
	}
	_ = historyResponse.Body.Close()

	changesResponse := doConversationClientRequest(t, client, baseURL, token,
		http.MethodGet, conversationChangesPath+"?after_cursor=1&limit=50", "", "", "")
	if changesResponse.StatusCode != http.StatusOK {
		t.Fatalf("client A changes status=%d body=%q", changesResponse.StatusCode, readConversationClientBody(t, changesResponse))
	}
	var changes model.OwnedConversationChangePage
	if err := json.NewDecoder(changesResponse.Body).Decode(&changes); err != nil || len(changes.Changes) != 1 ||
		changes.Changes[0].ConversationID != conversationID || changes.Changes[0].Kind != "prompt_appended" {
		t.Fatalf("client A changes=%#v decode=%v", changes, err)
	}
	_ = changesResponse.Body.Close()
}

func tokenForIndependentClient(identity *conversationTestIdentity, scopes, clientID string) string {
	return tokenForPrincipal(identity, "account-42", scopes, clientID)
}

func tokenForPrincipal(identity *conversationTestIdentity, subject, scopes, clientID string) string {
	return tokenForPrincipalWithTTL(identity, subject, scopes, clientID, 5*time.Minute)
}

func tokenForPrincipalWithTTL(identity *conversationTestIdentity, subject, scopes, clientID string, ttl time.Duration) string {
	header, _ := json.Marshal(map[string]string{"typ": "at+jwt", "alg": "EdDSA", "kid": identity.keyID})
	claims, _ := json.Marshal(map[string]any{
		"iss": identity.issuer, "sub": subject, "aud": "forge-api",
		"exp": time.Now().Add(ttl).Unix(), "iat": time.Now().Add(-time.Minute).Unix(),
		"tenant_id": "tenant-slate", "scope": scopes, "jti": clientID,
	})
	input := base64URL(header) + "." + base64URL(claims)
	signature := ed25519Sign(identity, []byte(input))
	return input + "." + base64URL(signature)
}

func doConversationClientRequest(
	t *testing.T,
	client *http.Client,
	baseURL, token, method, target, contentType, idempotencyKey, body string,
) *http.Response {
	t.Helper()
	request, err := http.NewRequest(method, baseURL+target, strings.NewReader(body))
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Authorization", "Bearer "+token)
	if contentType != "" {
		request.Header.Set("Content-Type", contentType)
	}
	if idempotencyKey != "" {
		request.Header.Set("Idempotency-Key", idempotencyKey)
	}
	response, err := client.Do(request)
	if err != nil {
		t.Fatal(err)
	}
	return response
}

func readConversationClientBody(t *testing.T, response *http.Response) string {
	t.Helper()
	defer response.Body.Close()
	body, err := io.ReadAll(response.Body)
	if err != nil {
		t.Fatal(err)
	}
	return string(body)
}

func base64URL(value []byte) string {
	return base64.RawURLEncoding.EncodeToString(value)
}

func ed25519Sign(identity *conversationTestIdentity, value []byte) []byte {
	return ed25519.Sign(identity.key, value)
}
