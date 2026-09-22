package appserver

// This opt-in test crosses the real Snaplink JWT boundary for the injected
// local Runner preview candidate. The candidate remains on a test mux only;
// this test never creates a normal Run or dispatches to a device.

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

func TestSnaplinkAuthenticatedLocalRunnerPreviewWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_LOCAL_RUNNER_PREVIEW_E2E") != "1" {
		t.Skip("set FORGE_LOCAL_RUNNER_PREVIEW_E2E=1 for the local Runner preview E2E")
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
	fake := &localRunnerPreviewRouteFake{output: "snaplink private output"}
	candidate := newConversationRoutesWithInertExecutionAPIAndLocalRunnerPreview(
		nil, nil, &localRunnerPreviewCandidateConfig{
			Enabled: true, Adapter: deviceplacement.LocalRunnerPreviewAdapter{Executor: fake},
		},
	)
	handler := authenticator.Handler(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// The TUI performs its normal owner-scoped session refresh before the
		// explicit preview command. Keep that read-only fixture on the same
		// authenticated test mux while every other path goes to the candidate.
		switch {
		case r.Method == http.MethodGet && r.URL.Path == conversationCollectionPath:
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"conversations":[{"conversation":{"id":"conversation-001","scope":{"kind":"global"},"title":"Local Runner preview fixture","created_at_ms":1,"updated_at_ms":1},"aggregate_version":1}],"next_after_id":null,"has_more":false}`))
		case r.Method == http.MethodGet && r.URL.Path == conversationCollectionPath+"/conversation-001/prompts":
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"conversation_id":"conversation-001","prompts":[],"next_cursor":null,"has_more":false}`))
		default:
			candidate.ServeHTTP(w, r)
		}
	}))
	server := httptest.NewServer(handler)
	t.Cleanup(server.Close)
	request := localRunnerPreviewRouteRequest(t, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant)
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-001/run-intents/intent-001/execution-readiness-preview"
	response := snaplinkConversationRequest(t, server.Client(), server.URL, token,
		http.MethodPost, path, "", string(body))
	if response.StatusCode != http.StatusOK {
		t.Fatalf("Snaplink local Runner preview status=%d body=%q", response.StatusCode, readConversationClientBody(t, response))
	}
	responseBody := readConversationClientBody(t, response)
	var observation deviceplacement.LocalRunnerPreviewObservation
	if err := json.Unmarshal([]byte(responseBody), &observation); err != nil {
		t.Fatalf("decode Snaplink local Runner preview: %v body=%q", err, responseBody)
	}
	if err := observation.Validate(); err != nil || observation.Intent.Owner.Subject != snaplinkForgeTestUser ||
		observation.Intent.ConversationID != "conversation-001" || observation.SessionReceipt.SelectedTargetID != nil ||
		observation.Authority != (deviceplacement.LocalRunnerPreviewAuthority{}) {
		t.Fatalf("Snaplink local Runner preview=%#v calls=%d err=%v", observation, fake.calls, err)
	}
	if strings.Contains(responseBody, "snaplink private output") || strings.Contains(responseBody, "fence-001") {
		t.Fatalf("Snaplink local Runner preview leaked private data: %q", responseBody)
	}
	requestPath := filepath.Join(t.TempDir(), "local-runner-preview-request.json")
	if err := os.WriteFile(requestPath, body, 0o600); err != nil {
		t.Fatal(err)
	}
	if executable := os.Getenv("FORGE_RUNTIME_BIN"); executable != "" {
		runForgeRuntimeLocalRunnerPreviewRemoteCLI(t, executable, server.URL, token, requestPath, request)
		runForgeRuntimeLocalRunnerPreviewRemoteTUI(t, executable, server.URL, token, requestPath, snaplinkForgeTestUser)
	}
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		runForgeConsoleLocalRunnerPreviewE2EWithToken(
			t, server.URL, token, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant,
		)
	}
	expectedCalls := 1
	if os.Getenv("FORGE_RUNTIME_BIN") != "" {
		expectedCalls += 2
	}
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		expectedCalls++
	}
	if fake.calls != expectedCalls {
		t.Fatalf("Snaplink local Runner preview executor calls=%d want=%d", fake.calls, expectedCalls)
	}
	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	productionRequest := httptest.NewRequest(http.MethodPost, server.URL+path, strings.NewReader(string(body)))
	productionRequest.Header.Set("Authorization", "Bearer "+token)
	productionRequest.Header.Set("Content-Type", "application/json")
	productionResponse := httptest.NewRecorder()
	production.ServeHTTP(productionResponse, productionRequest)
	if productionResponse.Code != http.StatusNotFound {
		t.Fatalf("production local Runner preview status=%d body=%q", productionResponse.Code, productionResponse.Body.String())
	}
}

func runForgeRuntimeLocalRunnerPreviewRemoteCLI(
	t *testing.T, executable, apiURL, accessToken, inputPath string,
	request deviceplacement.LocalRunnerPreviewRequest,
) {
	t.Helper()
	output, stderr, err := runForgeRuntimeCLI(t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "remote", "runner", "execution-readiness-preview", "--input", inputPath)
	if err != nil {
		t.Fatalf("authenticated Rust local Runner preview failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var observation deviceplacement.LocalRunnerPreviewObservation
	if err := json.Unmarshal([]byte(output), &observation); err != nil {
		t.Fatalf("decode authenticated Rust local Runner preview: %v stdout=%q", err, output)
	}
	if err := observation.Validate(); err != nil ||
		observation.Intent.Owner.Subject != request.Intent.Owner.Subject ||
		observation.Intent.ConversationID != request.Intent.ConversationID ||
		observation.SessionReceipt.SelectedTargetID != nil ||
		observation.Authority != (deviceplacement.LocalRunnerPreviewAuthority{}) {
		t.Fatalf("authenticated Rust local Runner preview=%#v err=%v", observation, err)
	}
}

func runForgeRuntimeLocalRunnerPreviewRemoteTUI(
	t *testing.T, executable, apiURL, accessToken, inputPath, subject string,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("authenticated Rust local Runner preview TUI requires script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("runner-execution-readiness-preview --input " + inputPath + "\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated Rust local Runner preview TUI failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("authenticated Rust local Runner preview TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"local Runner execution-readiness preview [forge.runner-local-execution-preview/v1]",
		"owner=" + subject + " conversation=conversation-001 prompt=prompt-001 run=run-001",
		"authority: device_identity_verified=false command_persisted=false reservation_created=false execution_authorized=false dispatch_performed=false audit_published=false",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("authenticated Rust local Runner preview TUI omitted %q: %q", want, output)
		}
	}
	if strings.Contains(output, "snaplink private output") || strings.Contains(output, "fence-001") {
		t.Fatalf("authenticated Rust local Runner preview TUI leaked private data: %q", output)
	}
}

func runForgeConsoleLocalRunnerPreviewE2EWithToken(
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
		t.Fatalf("Flutter is required for the local Runner preview E2E: %v", err)
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
		t.Fatalf("encode Flutter local Runner preview input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "local-runner-preview-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter local Runner preview input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub", "test/forge_local_runner_preview_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_LOCAL_RUNNER_PREVIEW_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter local Runner preview E2E failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("Flutter local Runner preview E2E output exceeded the size limit")
	}
}
