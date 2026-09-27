package appserver

import (
	"context"
	"encoding/json"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"time"
)

func runForgeConsoleLiveAPITest(
	t *testing.T,
	recorder *conversationHTTPRecorder,
	apiURL string,
	identity *conversationTestIdentity,
	conversationID string,
) {
	t.Helper()
	const scopes = "forge:conversations:read forge:conversations:write"
	token := tokenForPrincipalWithTTL(identity, "account-42", scopes, "console-client", 15*time.Minute)
	runForgeConsoleLiveAPITestWithToken(
		t, recorder, apiURL, token, conversationID, 2,
		"Prompt submitted by Flutter Console API client", forgeConsoleWidgetPromptContent,
		"prompt sent from client B", 2, "console-client-prompt",
		devicePlacementPreviewBody(identity.issuer, "account-42", "tenant-slate"), "", false,
	)
}

func runForgeConsoleLiveAPITestWithToken(
	t *testing.T,
	recorder *conversationHTTPRecorder,
	apiURL, token, conversationID string,
	expectedVersion int,
	prompt, widgetPrompt, existingPrompt string,
	afterCursor int, idempotencyKey string, placementRequest string,
	pendingIntentID string, pendingIntentFresh bool,
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

	input := struct {
		APIURL                       string          `json:"api_url"`
		AccessToken                  string          `json:"access_token"`
		ConversationID               string          `json:"conversation_id"`
		ExpectedVersion              int             `json:"expected_version"`
		IdempotencyKey               string          `json:"idempotency_key"`
		Prompt                       string          `json:"prompt"`
		WidgetPrompt                 string          `json:"widget_prompt"`
		ExistingPrompt               string          `json:"existing_prompt"`
		AfterCursor                  int             `json:"after_cursor"`
		PlacementRequest             json.RawMessage `json:"placement_request"`
		PendingIntentID              string          `json:"pending_intent_id,omitempty"`
		PendingIntentPrompt          string          `json:"pending_intent_prompt,omitempty"`
		PendingIntentExpectedVersion int             `json:"pending_intent_expected_version,omitempty"`
		PendingIntentKey             string          `json:"pending_intent_key,omitempty"`
		PendingIntentFresh           bool            `json:"pending_intent_fresh,omitempty"`
	}{
		APIURL: apiURL, AccessToken: token, ConversationID: conversationID,
		ExpectedVersion: expectedVersion, IdempotencyKey: idempotencyKey,
		Prompt: prompt, WidgetPrompt: widgetPrompt,
		ExistingPrompt: existingPrompt, AfterCursor: afterCursor,
		PlacementRequest: json.RawMessage(placementRequest), PendingIntentID: pendingIntentID,
		PendingIntentFresh: pendingIntentFresh,
		PendingIntentPrompt: func() string {
			if pendingIntentID != "" {
				if pendingIntentFresh {
					return "pending intent from Flutter Console"
				}
				return "prompt sent from client B"
			}
			return ""
		}(),
		PendingIntentExpectedVersion: func() int {
			if pendingIntentID != "" {
				if pendingIntentFresh {
					return expectedVersion - 1
				}
				return 1
			}
			return 0
		}(),
		PendingIntentKey: func() string {
			if pendingIntentID != "" {
				if pendingIntentFresh {
					return "snaplink-flutter-pending-fresh"
				}
				return "snaplink-execution-submit"
			}
			return ""
		}(),
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

// runForgeConsoleDeviceInventoryCandidateE2EWithToken exercises the private
// candidate mount used by the inert integration test. It runs a separate
// Flutter Sessions test with an explicit owner/reader injection; the caller
// filters the resulting recorder snapshot so the candidate contributes one
// inventory GET without changing the default shared-session flow.
func runForgeConsoleDeviceInventoryCandidateE2EWithToken(
	t *testing.T,
	apiURL, token, issuer, subject, tenant string,
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
		t.Fatalf("Flutter is required for the device inventory candidate E2E: %v", err)
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
		t.Fatalf("encode Flutter device inventory input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "device-inventory-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter device inventory input: %v", err)
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
		"test/forge_device_inventory_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_DEVICE_INVENTORY_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter device inventory candidate E2E failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Flutter device inventory candidate E2E output exceeded the size limit")
	}
}

// runForgeConsolePendingIntentWidgetE2EWithToken exercises the private,
// explicitly injected Sessions metadata reader. The default Gate leaves this
// reader unset, so this test-only mount is the only path that issues the
// pending-intent list GET from the product widget.
func runForgeConsolePendingIntentWidgetE2EWithToken(
	t *testing.T,
	apiURL, token, conversationID, pendingIntentID string,
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
		t.Fatalf("Flutter is required for the pending Run-intent widget E2E: %v", err)
	}
	input := struct {
		APIURL          string `json:"api_url"`
		AccessToken     string `json:"access_token"`
		ConversationID  string `json:"conversation_id"`
		PendingIntentID string `json:"pending_intent_id"`
	}{
		APIURL: apiURL, AccessToken: token, ConversationID: conversationID,
		PendingIntentID: pendingIntentID,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter pending Run-intent widget input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "pending-intent-widget-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter pending Run-intent widget input: %v", err)
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
		"test/forge_pending_run_intent_widget_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = forgeConsoleTestEnvironment(inputPath, flutterHome)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter pending Run-intent widget E2E failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Flutter pending Run-intent widget E2E output exceeded the size limit")
	}
}

// runForgeConsolePendingRunIntentGateE2EWithToken exercises the explicit
// scheduling-review submitter through the authenticated Sessions Gate. The
// default Gate leaves this candidate disabled; this helper supplies the
// owner/origin bindings and lets the Flutter test perform exactly one
// metadata-only POST.
func runForgeConsolePendingRunIntentGateE2EWithToken(
	t *testing.T,
	apiURL, token, conversationID, issuer, subject, tenant string,
	expectedVersion int,
	content, idempotencyKey string,
) {
	runForgeConsolePendingRunIntentGateModeE2EWithToken(
		t, apiURL, token, conversationID, issuer, subject, tenant,
		expectedVersion, content, idempotencyKey, true, "",
	)
}

// runForgeConsolePendingRunIntentGateReadE2EWithToken starts a fresh
// authenticated Sessions Gate after another client has written the receipt.
// The test intentionally leaves the Gate's submit action untouched so the
// only write in this phase is the external client write performed by the
// caller.
func runForgeConsolePendingRunIntentGateReadE2EWithToken(
	t *testing.T,
	apiURL, token, conversationID, issuer, subject, tenant string,
	expectedVersion int,
	content, pendingIntentID string,
) {
	runForgeConsolePendingRunIntentGateModeE2EWithToken(
		t, apiURL, token, conversationID, issuer, subject, tenant,
		expectedVersion, content, "read-only-gate-unused-key", false, pendingIntentID,
	)
}

// runForgeConsolePendingRunIntentGateLiveConvergenceE2EWithToken keeps the
// authenticated Sessions Gate mounted while a separate Runtime CLI process
// writes a pending Run-intent. The Flutter test then waits for the owner
// change feed to refresh Prompt history and pending metadata without a local
// Console POST.
func runForgeConsolePendingRunIntentGateLiveConvergenceE2EWithToken(
	t *testing.T,
	apiURL, token, conversationID, issuer, subject, tenant string,
	runtimeExecutable string,
	expectedVersion int,
	content, idempotencyKey string,
	existingPendingCount int,
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
		t.Fatalf("Flutter is required for live pending Run-intent Gate E2E: %v", err)
	}
	if runtimeExecutable == "" || expectedVersion < 1 || content == "" || idempotencyKey == "" || existingPendingCount < 1 {
		t.Fatalf("invalid live pending Run-intent Gate E2E input: runtime=%q version=%d content=%q key=%q existing=%d", runtimeExecutable, expectedVersion, content, idempotencyKey, existingPendingCount)
	}
	input := struct {
		APIURL               string `json:"api_url"`
		AccessToken          string `json:"access_token"`
		ConversationID       string `json:"conversation_id"`
		Issuer               string `json:"issuer"`
		Subject              string `json:"subject"`
		TenantID             string `json:"tenant_id"`
		ExpectedVersion      int    `json:"expected_version"`
		Content              string `json:"content"`
		IdempotencyKey       string `json:"idempotency_key"`
		RuntimeExecutable    string `json:"runtime_executable"`
		ExistingPendingCount int    `json:"existing_pending_count"`
	}{
		APIURL: apiURL, AccessToken: token, ConversationID: conversationID,
		Issuer: issuer, Subject: subject, TenantID: tenant,
		ExpectedVersion: expectedVersion, Content: content, IdempotencyKey: idempotencyKey,
		RuntimeExecutable: runtimeExecutable, ExistingPendingCount: existingPendingCount,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode live Flutter pending Run-intent Gate input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "pending-run-intent-gate-live-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private live Flutter pending Run-intent Gate input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 180*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub",
		"--dart-define=FORGE_CONVERSATIONS_API_ORIGIN="+apiURL,
		"test/forge_pending_run_intent_gate_live_convergence_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_PENDING_RUN_INTENT_LIVE_CONVERGENCE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter live pending Run-intent Gate E2E failed: stdout=%q stderr=%q", stdoutBuffer.String(), stderrBuffer.String())
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("live pending Run-intent Gate E2E output exceeded the size limit")
	}
}

func runForgeConsolePendingRunIntentGateModeE2EWithToken(
	t *testing.T,
	apiURL, token, conversationID, issuer, subject, tenant string,
	expectedVersion int,
	content, idempotencyKey string,
	submit bool, pendingIntentID string,
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
		t.Fatalf("Flutter is required for the pending Run-intent Gate E2E: %v", err)
	}
	if expectedVersion < 1 || content == "" || idempotencyKey == "" {
		t.Fatalf("invalid pending Run-intent Gate E2E input: version=%d content=%q key=%q", expectedVersion, content, idempotencyKey)
	}
	input := struct {
		APIURL          string `json:"api_url"`
		AccessToken     string `json:"access_token"`
		ConversationID  string `json:"conversation_id"`
		Issuer          string `json:"issuer"`
		Subject         string `json:"subject"`
		TenantID        string `json:"tenant_id"`
		ExpectedVersion int    `json:"expected_version"`
		Content         string `json:"content"`
		IdempotencyKey  string `json:"idempotency_key"`
		Submit          bool   `json:"submit"`
		PendingIntentID string `json:"pending_intent_id,omitempty"`
	}{
		APIURL: apiURL, AccessToken: token, ConversationID: conversationID,
		Issuer: issuer, Subject: subject, TenantID: tenant,
		ExpectedVersion: expectedVersion, Content: content, IdempotencyKey: idempotencyKey,
		Submit: submit, PendingIntentID: pendingIntentID,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter pending Run-intent Gate input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "pending-run-intent-gate-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter pending Run-intent Gate input: %v", err)
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
		"test/forge_pending_run_intent_gate_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_PENDING_RUN_INTENT_GATE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter pending Run-intent Gate E2E failed: stdout=%q stderr=%q", stdoutBuffer.String(), stderrBuffer.String())
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Flutter pending Run-intent Gate E2E output exceeded the size limit")
	}
}

func runForgeConsoleBrowserE2E(
	t *testing.T,
	apiURL string,
	identity *conversationTestIdentity,
	conversationID string,
) {
	t.Helper()
	const scopes = "forge:conversations:read forge:conversations:write"
	token := tokenForPrincipalWithTTL(identity, "account-42", scopes, "browser-client", 15*time.Minute)
	runForgeConsoleBrowserE2EWithToken(
		t, apiURL, token, conversationID, forgeConsoleBrowserPromptContent,
		true,
	)
}

func runForgeConsoleBrowserE2EWithToken(
	t *testing.T,
	apiURL, token, conversationID, prompt string,
	deepLink bool,
) {
	t.Helper()
	runForgeConsoleBrowserProcess(t, apiURL, token, conversationID, prompt, "", nil, nil, nil, nil, true, deepLink)
}

func runForgeConsoleBrowserRunObservationE2EWithToken(
	t *testing.T,
	apiURL, token, conversationID, runID string,
	runObserved json.RawMessage,
	sessionObservation json.RawMessage,
	runnerExecutionObservation json.RawMessage,
	sessionRunnerReceiptObservation json.RawMessage,
) {
	t.Helper()
	runForgeConsoleBrowserProcess(
		t, apiURL, token, conversationID, "", runID, runObserved, sessionObservation,
		runnerExecutionObservation,
		sessionRunnerReceiptObservation,
		false, false,
	)
}

func runForgeConsoleBrowserProcess(
	t *testing.T,
	apiURL, token, conversationID, prompt, runID string,
	runObserved json.RawMessage,
	sessionObservation json.RawMessage,
	runnerExecutionObservation json.RawMessage,
	sessionRunnerReceiptObservation json.RawMessage,
	reloadSession bool,
	deepLink bool,
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
	input := struct {
		PageURL                         string          `json:"page_url"`
		AccessToken                     string          `json:"access_token"`
		ConversationID                  string          `json:"conversation_id"`
		Prompt                          string          `json:"prompt"`
		RunID                           string          `json:"run_id,omitempty"`
		RunObserved                     json.RawMessage `json:"run_observed,omitempty"`
		ReloadSession                   bool            `json:"reload_session,omitempty"`
		DeepLink                        bool            `json:"deep_link,omitempty"`
		SessionObservation              json.RawMessage `json:"session_observation,omitempty"`
		RunnerExecutionObservation      json.RawMessage `json:"runner_execution_observation,omitempty"`
		SessionRunnerReceiptObservation json.RawMessage `json:"session_runner_receipt_observation,omitempty"`
	}{
		PageURL: apiURL + "/forge/", AccessToken: token,
		ConversationID: conversationID, Prompt: prompt, RunID: runID, RunObserved: runObserved,
		ReloadSession: reloadSession, DeepLink: deepLink,
		SessionObservation: sessionObservation, RunnerExecutionObservation: runnerExecutionObservation,
		SessionRunnerReceiptObservation: sessionRunnerReceiptObservation,
	}
	if deepLink {
		input.PageURL = apiURL + "/forge/conversations/" + url.PathEscape(conversationID)
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
	conversationID, pendingIntentID string,
	pendingIntentFresh bool,
) {
	t.Helper()
	expectedRequests := 10
	if pendingIntentID != "" {
		expectedRequests = 13
		if pendingIntentFresh {
			expectedRequests++
		}
	}
	if len(requests) != expectedRequests {
		t.Fatalf("Console issued %d HTTP requests; expected %d API-service and screen calls: %#v", len(requests), expectedRequests, requests)
	}
	promptPath := conversationCollectionPath + "/" + conversationID + "/prompts"
	changesQuery := "after_cursor=2&limit=128"
	if pendingIntentID != "" {
		changesQuery = "after_cursor=4&limit=128"
	}
	want := map[recordedConversationRequest]int{
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=50"}:                                  2,
		{method: http.MethodGet, path: promptPath, query: "limit=100"}:                                                 2,
		{method: http.MethodPost, path: promptPath}:                                                                    3,
		{method: http.MethodGet, path: conversationChangesPath, query: changesQuery}:                                   1,
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/runs", query: "limit=25"}: 1,
		{method: http.MethodPost, path: devicePlacementPreviewPath}:                                                    1,
	}
	if pendingIntentID != "" {
		intentPath := conversationCollectionPath + "/" + conversationID + "/run-intents"
		want[recordedConversationRequest{method: http.MethodPost, path: intentPath}] = 1
		want[recordedConversationRequest{method: http.MethodGet, path: intentPath, query: "limit=25"}] = 1
		want[recordedConversationRequest{
			method: http.MethodGet,
			path:   intentPath + "/" + pendingIntentID + "/timeline",
			query:  "after_sequence=0&limit=25",
		}] = 1
		if pendingIntentFresh {
			freshTimelinePathPrefix := intentPath + "/"
			var freshTimeline []recordedConversationRequest
			for _, request := range requests {
				if request.method == http.MethodGet &&
					request.query == "after_sequence=0&limit=25" &&
					strings.HasPrefix(request.path, freshTimelinePathPrefix) &&
					strings.HasSuffix(request.path, "/timeline") &&
					request.path != intentPath+"/"+pendingIntentID+"/timeline" {
					freshTimeline = append(freshTimeline, request)
				}
			}
			if len(freshTimeline) != 1 {
				t.Fatalf("expected one fresh pending-intent timeline read, got %#v", freshTimeline)
			}
			want[freshTimeline[0]] = 1
		}
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
