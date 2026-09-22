package appserver

// This opt-in test crosses the real Snaplink JWT boundary for the
// Run/Attempt/lease/dispatch preflight candidate. The candidate remains on an
// inert/test mux and only compares caller-supplied metadata.

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
)

func TestSnaplinkAuthenticatedRunAttemptLeaseDispatchPreflightWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_E2E") != "1" {
		t.Skip("set FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_E2E=1 for the preflight E2E")
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
	request := runAttemptLeaseDispatchPreflightRequest(owner, "conversation-001", "run-001")
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	candidate := newConversationRoutesWithInertExecutionAPI(nil, nil)
	handler := authenticator.Handler(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// The Rust TUI performs its normal owner-scoped session refresh before
		// the explicit preflight command. Keep those reads on the same JWT mux.
		switch {
		case r.Method == http.MethodGet && r.URL.Path == conversationCollectionPath:
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"conversations":[{"conversation":{"id":"conversation-001","scope":{"kind":"global"},"title":"Preflight fixture","created_at_ms":1,"updated_at_ms":1},"aggregate_version":1}],"next_after_id":null,"has_more":false}`))
		case r.Method == http.MethodGet && r.URL.Path == conversationCollectionPath+"/conversation-001/prompts":
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"conversation_id":"conversation-001","prompts":[{"id":"prompt-001","conversation_id":"conversation-001","role":"user","content":"Preflight fixture","created_at_ms":100}],"next_cursor":null,"has_more":false}`))
		case r.Method == http.MethodGet && r.URL.Path == conversationCollectionPath+"/conversation-001/runs":
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"conversation_id":"conversation-001","runs":[{"run_id":"run-001","prompt_id":"prompt-001","created_at_ms":200000,"latest_sequence":2,"status":"completed"}],"has_more":false}`))
		case r.Method == http.MethodGet && r.URL.Path == conversationCollectionPath+"/conversation-001/runs/run-001/timeline":
			w.Header().Set("Content-Type", "application/json")
			afterSequence := r.URL.Query().Get("after_sequence")
			if afterSequence == "2" {
				_, _ = w.Write([]byte(`{"conversation_id":"conversation-001","run_id":"run-001","after_sequence":2,"scanned_through_sequence":2,"has_more":false,"events":[]}`))
			} else {
				_, _ = w.Write([]byte(`{"conversation_id":"conversation-001","run_id":"run-001","after_sequence":0,"scanned_through_sequence":2,"has_more":false,"events":[{"seq":1,"emitted_at_ms":200001,"type":"run_started"},{"seq":2,"emitted_at_ms":200002,"type":"run_finished"}]}`))
			}
		case r.Method == http.MethodGet && r.URL.Path == conversationChangesPath:
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"after_cursor":0,"scanned_through_cursor":0,"has_more":false,"changes":[]}`))
		default:
			candidate.ServeHTTP(w, r)
		}
	}))
	serverHandler := http.Handler(handler)
	if os.Getenv("FORGE_BROWSER_E2E") == "1" {
		webBuildDir := os.Getenv("FORGE_WEB_BUILD_DIR")
		if webBuildDir == "" {
			t.Fatal("FORGE_WEB_BUILD_DIR is required for Run/Attempt/lease preflight Chromium E2E")
		}
		serverHandler = serveForgeConsoleWebAssets(webBuildDir, serverHandler)
	}
	server := httptest.NewServer(serverHandler)
	t.Cleanup(server.Close)
	path := "/api/v1/conversations/conversation-001/runs/run-001/attempt-lease-dispatch-preflight/preview"
	response := snaplinkConversationRequest(t, server.Client(), server.URL, token,
		http.MethodPost, path, "", string(body))
	if response.StatusCode != http.StatusOK {
		t.Fatalf("Snaplink Run/Attempt/lease preflight status=%d body=%q", response.StatusCode,
			readConversationClientBody(t, response))
	}
	responseBody := readConversationClientBody(t, response)
	var observation deviceplacement.RunAttemptLeaseDispatchPreflightObservation
	if err := json.Unmarshal([]byte(responseBody), &observation); err != nil {
		t.Fatalf("decode Snaplink Run/Attempt/lease preflight: %v body=%q", err, responseBody)
	}
	if err := observation.Validate(); err != nil ||
		observation.Owner != owner || observation.ConversationID != request.ConversationID ||
		observation.RunID != request.RunID || observation.SelectedTargetID != nil ||
		observation.Authority != (deviceplacement.RunAttemptLeaseDispatchPreflightAuthority{}) {
		t.Fatalf("Snaplink Run/Attempt/lease preflight=%#v err=%v", observation, err)
	}
	if strings.Contains(responseBody, request.DispatchPlan.Lease.FencingToken) {
		t.Fatalf("Snaplink preflight leaked lease fencing material: %q", responseBody)
	}
	requestPath := filepath.Join(t.TempDir(), "run-attempt-lease-dispatch-preflight-request.json")
	if err := os.WriteFile(requestPath, body, 0o600); err != nil {
		t.Fatal(err)
	}
	if executable := os.Getenv("FORGE_RUNTIME_BIN"); executable != "" {
		runForgeRuntimeRunAttemptLeaseDispatchPreflightRemoteCLI(
			t, executable, server.URL, token, requestPath, request,
		)
		runForgeRuntimeRunAttemptLeaseDispatchPreflightRemoteTUI(
			t, executable, server.URL, token, requestPath, snaplinkForgeTestUser,
		)
	}
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		runForgeConsoleRunAttemptLeaseDispatchPreflightE2EWithToken(
			t, server.URL, token, request,
		)
		if os.Getenv("FORGE_BROWSER_E2E") == "1" {
			runForgeConsoleRunAttemptLeaseDispatchPreflightBrowserE2EWithToken(
				t, server.URL, token, request,
			)
		}
	}

	// The same authenticated production constructor must remain closed even
	// when it is placed behind the very same verifier and request body.
	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	productionRequest := httptest.NewRequest(http.MethodPost, server.URL+path, strings.NewReader(string(body)))
	productionRequest.Header.Set("Authorization", "Bearer "+token)
	productionRequest.Header.Set("Content-Type", "application/json")
	productionResponse := httptest.NewRecorder()
	production.ServeHTTP(productionResponse, productionRequest)
	if productionResponse.Code != http.StatusNotFound || productionResponse.Body.String() != string(notFoundBody) {
		t.Fatalf("production Run/Attempt/lease preflight status=%d body=%q",
			productionResponse.Code, productionResponse.Body.String())
	}
}

func runForgeConsoleRunAttemptLeaseDispatchPreflightBrowserE2EWithToken(
	t *testing.T,
	apiURL, token string,
	request deviceplacement.RunAttemptLeaseDispatchPreflightRequest,
) {
	t.Helper()
	pythonBinary := os.Getenv("FORGE_BROWSER_PYTHON")
	if pythonBinary == "" {
		pythonBinary = "python3"
	}
	pythonExecutable, err := exec.LookPath(pythonBinary)
	if err != nil {
		t.Fatalf("Python is required for Run/Attempt/lease preflight Chromium E2E: %v", err)
	}
	workingDirectory, err := os.Getwd()
	if err != nil {
		t.Fatalf("resolve Forge Core repository: %v", err)
	}
	repoRoot := filepath.Clean(filepath.Join(workingDirectory, "..", "..", ".."))
	runnerPath := filepath.Join(repoRoot, "scripts", "forge_console_browser_e2e.py")
	if _, err := os.Stat(runnerPath); err != nil {
		t.Fatalf("Forge Web browser runner not found at %s: %v", runnerPath, err)
	}
	expected, err := deviceplacement.ObserveRunAttemptLeaseDispatchPreflight(request)
	if err != nil {
		t.Fatalf("observe Forge Web preflight fixture: %v", err)
	}
	input := struct {
		PageURL      string                                                      `json:"page_url"`
		AccessToken  string                                                      `json:"access_token"`
		Conversation string                                                      `json:"conversation_id"`
		Prompt       string                                                      `json:"prompt"`
		Title        string                                                      `json:"conversation_title"`
		Preflight    deviceplacement.RunAttemptLeaseDispatchPreflightRequest     `json:"run_attempt_lease_dispatch_preflight_request"`
		Expected     deviceplacement.RunAttemptLeaseDispatchPreflightObservation `json:"run_attempt_lease_dispatch_preflight_expected"`
	}{
		PageURL: apiURL + "/forge/", AccessToken: token,
		Conversation: request.ConversationID, Prompt: "", Title: "Preflight fixture",
		Preflight: request,
		Expected:  expected,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Forge Web preflight input: %v", err)
	}
	inputPath := filepath.Join(t.TempDir(), "forge-browser-preflight-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Forge Web preflight input: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, pythonExecutable, runnerPath, inputPath)
	command.Env = append(os.Environ(), "FORGE_BROWSER_PYTHON="+pythonExecutable)
	if browserExecutable := os.Getenv("FORGE_BROWSER_EXECUTABLE"); browserExecutable != "" {
		command.Env = append(command.Env, "FORGE_BROWSER_EXECUTABLE="+browserExecutable)
	}
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Forge Web Run/Attempt/lease preflight E2E failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("Forge Web Run/Attempt/lease preflight E2E output exceeded the size limit")
	}
}

func runForgeConsoleRunAttemptLeaseDispatchPreflightE2EWithToken(
	t *testing.T, apiURL, token string,
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
		t.Fatalf("Flutter is required for the Run/Attempt/lease preflight E2E: %v", err)
	}
	input := struct {
		APIURL      string                                                  `json:"api_url"`
		AccessToken string                                                  `json:"access_token"`
		Request     deviceplacement.RunAttemptLeaseDispatchPreflightRequest `json:"request"`
	}{APIURL: apiURL, AccessToken: token, Request: request}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter Run/Attempt/lease preflight input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "run-attempt-lease-dispatch-preflight-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter Run/Attempt/lease preflight input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub", "test/forge_run_attempt_lease_dispatch_preflight_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter Run/Attempt/lease preflight E2E failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("Flutter Run/Attempt/lease preflight E2E output exceeded the size limit")
	}
}

func runForgeRuntimeRunAttemptLeaseDispatchPreflightRemoteCLI(
	t *testing.T,
	executable, apiURL, accessToken, inputPath string,
	request deviceplacement.RunAttemptLeaseDispatchPreflightRequest,
) {
	t.Helper()
	output, stderr, err := runForgeRuntimeCLI(t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "remote", "run-attempt-lease-dispatch-preflight-preview", "--input", inputPath)
	if err != nil {
		t.Fatalf("authenticated Rust Run/Attempt/lease preflight CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var observation deviceplacement.RunAttemptLeaseDispatchPreflightObservation
	if err := json.Unmarshal([]byte(output), &observation); err != nil {
		t.Fatalf("decode authenticated Rust Run/Attempt/lease preflight CLI: %v stdout=%q", err, output)
	}
	if err := observation.Validate(); err != nil ||
		observation.Owner != request.Owner ||
		observation.ConversationID != request.ConversationID ||
		observation.RunID != request.RunID || observation.SelectedTargetID != nil ||
		observation.Authority != (deviceplacement.RunAttemptLeaseDispatchPreflightAuthority{}) {
		t.Fatalf("authenticated Rust Run/Attempt/lease preflight CLI=%#v err=%v", observation, err)
	}
}

func runForgeRuntimeRunAttemptLeaseDispatchPreflightRemoteTUI(
	t *testing.T,
	executable, apiURL, accessToken, inputPath, subject string,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("authenticated Rust Run/Attempt/lease preflight TUI requires script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader(
		"run-attempt-lease-dispatch-preflight-remote-preview --input " + inputPath + "\nquit\n",
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated Rust Run/Attempt/lease preflight TUI failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("authenticated Rust Run/Attempt/lease preflight TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"Run/Attempt/lease dispatch preflight [forge.run-attempt-lease-dispatch-preflight/v1]",
		"owner=" + subject + " conversation=conversation-001 run=run-001 status=nonterminal",
		"declarative_preflight_ready=true selected_target_id=null",
		"dispatch_performed=false",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("authenticated Rust Run/Attempt/lease preflight TUI omitted %q: %q", want, output)
		}
	}
	if strings.Contains(output, "fence-001") || strings.Contains(output, "/api/v1/devices") {
		t.Fatalf("authenticated Rust Run/Attempt/lease preflight TUI leaked private data or device request: %q", output)
	}
}
