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
	"reflect"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
)

// TestRunAcceptedExecuteSessionRunnerReconciliationE2E proves the smallest
// accepted EXECUTE + P4 session Runner reconciliation chain: Core receives a
// caller-supplied receipt history through the authenticated history preview,
// then feeds that canonical result into the authenticated reconciliation
// projection. Both handlers remain pure observations; this test does not
// configure a lease registry or a Runner authority.
func TestRunAcceptedExecuteSessionRunnerReconciliationE2E(t *testing.T) {
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)

	root := privateTestParent(t)
	stateDir := filepath.Join(root, "app-state-session-runner-reconciliation-e2e")
	runtimeStateDir := filepath.Join(root, "runtime-state-session-runner-reconciliation-e2e")
	if err := os.Mkdir(runtimeStateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	runtimeExecutable := filepath.Join(root, "forge-runtime-session-runner-reconciliation-e2e")
	if err := os.WriteFile(runtimeExecutable, []byte("test runtime"), 0o700); err != nil {
		t.Fatal(err)
	}

	owner := deviceidentity.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	historyBody := acceptedSessionRunnerReconciliationHistoryBody(t, deviceplacement.Owner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	})
	var suppliedHistory deviceplacement.SessionRunnerReceiptHistoryObservation
	if err := json.Unmarshal(historyBody, &suppliedHistory); err != nil {
		t.Fatalf("decode session Runner receipt history fixture: %v", err)
	}
	if err := suppliedHistory.Validate(); err != nil {
		t.Fatalf("validate session Runner receipt history fixture: %v", err)
	}

	state := lifecycleRegistrySourceState(t, owner, "device-a", "runner-session-reconciliation-e2e", 1)
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})
	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{
		Status: "accepted", AcceptanceID: "p4-session-runner-reconciliation-e2e-001", AcceptedAtUnixMS: 1,
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
		t.Fatalf("session Runner reconciliation E2E server failed before readiness: %v", err)
	case <-time.After(5 * time.Second):
		t.Fatal("session Runner reconciliation E2E server did not announce readiness")
	}
	t.Cleanup(func() {
		cancel()
		if err := waitResult(t, result); err != nil {
			t.Error(err)
		}
	})

	client := &http.Client{Timeout: 5 * time.Second}
	post := func(path string, payload []byte) (int, []byte) {
		t.Helper()
		req, err := http.NewRequest(http.MethodPost, ready.Listen+path, strings.NewReader(string(payload)))
		if err != nil {
			t.Fatal(err)
		}
		req.Header.Set("Authorization", "Bearer "+token)
		req.Header.Set("Content-Type", "application/json")
		response, err := client.Do(req)
		if err != nil {
			t.Fatal(err)
		}
		defer response.Body.Close()
		body, err := io.ReadAll(response.Body)
		if err != nil {
			t.Fatal(err)
		}
		return response.StatusCode, body
	}

	historyPath := sessionRunnerReceiptHistoryPathURL(suppliedHistory.ConversationID, suppliedHistory.RunID)
	status, canonicalHistoryBody := post(historyPath, historyBody)
	if status != http.StatusOK {
		t.Fatalf("session Runner receipt history status=%d body=%q", status, canonicalHistoryBody)
	}
	var canonicalHistory deviceplacement.SessionRunnerReceiptHistoryObservation
	if err := json.Unmarshal(canonicalHistoryBody, &canonicalHistory); err != nil {
		t.Fatalf("decode canonical session Runner receipt history: %v body=%q", err, canonicalHistoryBody)
	}
	if err := canonicalHistory.Validate(); err != nil || !reflect.DeepEqual(canonicalHistory, suppliedHistory) {
		t.Fatalf("canonical session Runner receipt history=%#v supplied=%#v err=%v", canonicalHistory, suppliedHistory, err)
	}

	reconciliationPath := sessionRunnerReconciliationProjectionPathURL(canonicalHistory.ConversationID, canonicalHistory.RunID)
	status, projectionBody := post(reconciliationPath, canonicalHistoryBody)
	if status != http.StatusOK {
		t.Fatalf("session Runner reconciliation status=%d body=%q", status, projectionBody)
	}
	var projection deviceplacement.SessionRunnerReconciliationProjection
	if err := json.Unmarshal(projectionBody, &projection); err != nil {
		t.Fatalf("decode session Runner reconciliation: %v body=%q", err, projectionBody)
	}
	want, err := deviceplacement.ProjectSessionRunnerReconciliation(canonicalHistory)
	if err != nil {
		t.Fatal(err)
	}
	if err := projection.Validate(); err != nil || projection != want {
		t.Fatalf("session Runner reconciliation projection=%#v want=%#v err=%v", projection, want, err)
	}
	if projection.SelectedTargetID != nil || projection.AutomaticRetry || !projection.ManualReviewRequired ||
		!projection.ReconciliationRequired || projection.Authority != (deviceplacement.SessionRunnerReceiptHistoryAuthority{}) {
		t.Fatalf("unsafe session Runner reconciliation projection=%#v", projection)
	}
	for _, forbidden := range []string{"fencing_token", "receipt_sha256", "argv", "workspace"} {
		if strings.Contains(string(projectionBody), forbidden) {
			t.Fatalf("session Runner reconciliation leaked %q: %q", forbidden, projectionBody)
		}
	}

	// The authenticated Runtime CLI consumes the same complete history and
	// revalidates the Core-derived projection. It needs no Hub session read, so
	// the opt-in check remains useful even when this harness uses the inert
	// Runtime executable fallback.
	if configuredRuntime := os.Getenv("FORGE_RUNTIME_BIN"); configuredRuntime != "" {
		runForgeRuntimeSessionRunnerReconciliationRemoteCLI(
			t, configuredRuntime, ready.Listen, token, canonicalHistoryBody,
			ownerToPlacement(owner), canonicalHistory.ConversationID,
			canonicalHistory.PromptID, canonicalHistory.RunID,
		)
	}
}

func runForgeRuntimeSessionRunnerReconciliationRemoteCLI(
	t *testing.T, executable, apiURL, accessToken string, historyBody []byte,
	owner deviceplacement.Owner, conversationID, promptID, runID string,
) {
	t.Helper()
	inputPath := filepath.Join(t.TempDir(), "session-runner-reconciliation-history.json")
	if err := os.WriteFile(inputPath, historyBody, 0o600); err != nil {
		t.Fatalf("write session Runner reconciliation history for Runtime CLI: %v", err)
	}
	output, stderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, accessToken, t.TempDir(), "--json", "remote",
		"session-runner-reconciliation", "remote-preview", "--input", inputPath,
	)
	if err != nil {
		t.Fatalf("authenticated session Runner reconciliation Runtime CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var projection deviceplacement.SessionRunnerReconciliationProjection
	if err := json.Unmarshal([]byte(output), &projection); err != nil {
		t.Fatalf("decode session Runner reconciliation Runtime CLI: %v stdout=%q", err, output)
	}
	assertAcceptedSessionRunnerReconciliationProjectionE2E(
		t, projection, owner, conversationID, promptID, runID,
	)
}

func runForgeRuntimeSessionRunnerReconciliationRemoteTUI(
	t *testing.T, executable, apiURL, accessToken string, historyBody []byte, conversationID string,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("session Runner reconciliation TUI E2E requires script: %v", err)
	}
	inputPath := filepath.Join(t.TempDir(), "session-runner-reconciliation-history.json")
	if err := os.WriteFile(inputPath, historyBody, 0o600); err != nil {
		t.Fatalf("write session Runner reconciliation history for TUI: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(
		ctx, ptyScript, "-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui",
	)
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader(
		"open " + conversationID + "\n" +
			"session-runner-reconciliation-remote-preview --input " + inputPath + "\nquit\n",
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated session Runner reconciliation TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("session Runner reconciliation TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"session Runner reconciliation projection [forge.session-runner-reconciliation-projection/v1]",
		"reconciliation: kind=manual reason=uncertain_terminal_receipt required=true manual_review_required=true automatic_retry=false follow_up=reconciliation_manual",
		"binding: preview_only=true selected_target=none",
		"authority: identity_verified=false receipt_persisted=false execution_authorized=false dispatch_performed=false audit_published=false",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("session Runner reconciliation TUI output omitted %q: %q", want, output)
		}
	}
	for _, forbidden := range []string{"fencing_token", "receipt_sha256", "argv", "workspace", "/api/v1/devices"} {
		if strings.Contains(output, forbidden) {
			t.Fatalf("session Runner reconciliation TUI leaked %q: %q", forbidden, output)
		}
	}
}

func assertAcceptedSessionRunnerReconciliationProjectionE2E(
	t *testing.T, projection deviceplacement.SessionRunnerReconciliationProjection,
	owner deviceplacement.Owner, conversationID, promptID, runID string,
) {
	t.Helper()
	if err := projection.Validate(); err != nil || projection.Owner != owner ||
		projection.ConversationID != conversationID || projection.PromptID != promptID ||
		projection.RunID != runID || projection.LatestDispositionKind != "uncertain" ||
		projection.ReconciliationKind != "manual" ||
		projection.ReconciliationReason != "uncertain_terminal_receipt" ||
		!projection.ReconciliationRequired || !projection.ManualReviewRequired ||
		projection.AutomaticRetry || projection.FollowUp != "reconciliation_manual" ||
		projection.SelectedTargetID != nil || !projection.PreviewOnly ||
		projection.Authority != (deviceplacement.SessionRunnerReceiptHistoryAuthority{}) {
		t.Fatalf("accepted session Runner reconciliation projection=%#v err=%v", projection, err)
	}
}

func acceptedSessionRunnerReconciliationHistoryBody(t *testing.T, owner deviceplacement.Owner) []byte {
	return acceptedSessionRunnerReconciliationHistoryBodyForRun(
		t, owner, "conversation-001", "prompt-001", "run-001",
	)
}

// acceptedSessionRunnerReconciliationHistoryBodyForRun rewrites the public
// receipt-history fixture onto the concrete owner/session IDs used by an
// accepted HTTP harness. Keeping the rewrite in the fixture-to-observation
// boundary preserves the strict reducer and gives Console API/Gate tests the
// same canonical history that Core accepted.
func acceptedSessionRunnerReconciliationHistoryBodyForRun(
	t *testing.T, owner deviceplacement.Owner, conversationID, promptID, runID string,
) []byte {
	t.Helper()
	path := os.Getenv("FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE")
	if path == "" {
		t.Skip("FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE is set by the contract script")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture deviceplacement.SessionRunnerReceiptHistoryObservation
	if err := json.Unmarshal(encoded, &fixture); err != nil {
		t.Fatal(err)
	}
	fixture.Owner = owner
	fixture.ConversationID = conversationID
	fixture.PromptID = promptID
	fixture.RunID = runID
	for index := range fixture.Receipts {
		fixture.Receipts[index].Owner = owner
		fixture.Receipts[index].ConversationID = conversationID
		fixture.Receipts[index].PromptID = promptID
		fixture.Receipts[index].RunID = runID
	}
	derived, err := deviceplacement.ObserveSessionRunnerReceiptHistory(deviceplacement.SessionRunnerReceiptHistoryRequest{
		Owner: owner, ConversationID: fixture.ConversationID, PromptID: fixture.PromptID,
		RunID: fixture.RunID, Receipts: fixture.Receipts,
	})
	if err != nil {
		t.Fatal(err)
	}
	body, err := json.Marshal(derived)
	if err != nil {
		t.Fatal(err)
	}
	return body
}

func sessionRunnerReceiptHistoryPathURL(conversationID, runID string) string {
	return "/api/v1/conversations/" + conversationID + "/runs/" + runID + "/runner-receipt-history/preview"
}

func sessionRunnerReconciliationProjectionPathURL(conversationID, runID string) string {
	return "/api/v1/conversations/" + conversationID + "/runs/" + runID + "/runner-reconciliation/preview"
}

func runForgeConsoleSessionRunnerReconciliationProjectionE2EWithToken(
	t *testing.T, apiURL, token string, historyBody []byte, conversationID, promptID, runID string,
) {
	t.Helper()
	consoleRoot := forgeConsoleE2ERoot(t)
	flutterExecutable := forgeConsoleFlutterExecutable(t, "session Runner reconciliation API E2E")
	var history deviceplacement.SessionRunnerReceiptHistoryObservation
	if err := json.Unmarshal(historyBody, &history); err != nil {
		t.Fatalf("decode Flutter session Runner reconciliation input: %v", err)
	}
	if err := history.Validate(); err != nil || history.ConversationID != conversationID ||
		history.PromptID != promptID || history.RunID != runID {
		t.Fatalf("Flutter session Runner reconciliation history binding is invalid: history=%#v err=%v", history, err)
	}
	inputJSON, err := json.Marshal(struct {
		APIURL       string                                                 `json:"api_url"`
		AccessToken  string                                                 `json:"access_token"`
		History      deviceplacement.SessionRunnerReceiptHistoryObservation `json:"history"`
		Conversation string                                                 `json:"conversation_id"`
		Prompt       string                                                 `json:"prompt_id"`
		Run          string                                                 `json:"run_id"`
	}{
		APIURL: apiURL, AccessToken: token, History: history,
		Conversation: conversationID, Prompt: promptID, Run: runID,
	})
	if err != nil {
		t.Fatalf("encode Flutter session Runner reconciliation input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "session-runner-reconciliation-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write Flutter session Runner reconciliation input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable, "test", "--no-pub",
		"test/forge_session_runner_reconciliation_projection_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter session Runner reconciliation API E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("session Runner reconciliation API Flutter output exceeded the size limit")
	}
}

func runForgeConsoleSessionRunnerReconciliationProjectionGateE2EWithToken(
	t *testing.T, apiURL, token string, historyBody []byte, conversationID, promptID, runID string,
) {
	t.Helper()
	consoleRoot := forgeConsoleE2ERoot(t)
	flutterExecutable := forgeConsoleFlutterExecutable(t, "session Runner reconciliation Gate E2E")
	var history deviceplacement.SessionRunnerReceiptHistoryObservation
	if err := json.Unmarshal(historyBody, &history); err != nil {
		t.Fatalf("decode Flutter session Runner reconciliation Gate input: %v", err)
	}
	if err := history.Validate(); err != nil || history.ConversationID != conversationID ||
		history.PromptID != promptID || history.RunID != runID {
		t.Fatalf("Flutter session Runner reconciliation Gate history binding is invalid: history=%#v err=%v", history, err)
	}
	inputJSON, err := json.Marshal(struct {
		APIURL       string                                                 `json:"api_url"`
		AccessToken  string                                                 `json:"access_token"`
		History      deviceplacement.SessionRunnerReceiptHistoryObservation `json:"history"`
		Conversation string                                                 `json:"conversation_id"`
		Prompt       string                                                 `json:"prompt_id"`
		Run          string                                                 `json:"run_id"`
	}{
		APIURL: apiURL, AccessToken: token, History: history,
		Conversation: conversationID, Prompt: promptID, Run: runID,
	})
	if err != nil {
		t.Fatalf("encode Flutter session Runner reconciliation Gate input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "session-runner-reconciliation-gate-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter session Runner reconciliation Gate input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable, "test", "--no-pub",
		"--dart-define=FORGE_CONVERSATIONS_API_ORIGIN="+apiURL,
		"test/forge_session_runner_reconciliation_projection_gate_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_GATE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter session Runner reconciliation Gate E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("session Runner reconciliation Gate Flutter output exceeded the size limit")
	}
}

func forgeConsoleE2ERoot(t *testing.T) string {
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
	return consoleRoot
}

func forgeConsoleFlutterExecutable(t *testing.T, purpose string) string {
	t.Helper()
	flutterBinary := os.Getenv("FLUTTER_BIN")
	if flutterBinary == "" {
		flutterBinary = "flutter"
	}
	flutterExecutable, err := exec.LookPath(flutterBinary)
	if err != nil {
		t.Fatalf("Flutter is required for %s: %v", purpose, err)
	}
	return flutterExecutable
}
