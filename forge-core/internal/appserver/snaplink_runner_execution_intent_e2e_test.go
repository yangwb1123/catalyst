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
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/runtimebridge"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// TestRunAcceptedExecuteRunnerExecutionIntentAcrossClients crosses the
// accepted EXECUTE + P4 assembly with one owner-bound Prompt/Run/Runner
// binding. Core, Runtime CLI/TUI, and the opt-in Console API consume the same
// metadata-only result. No lease, selection, command persistence, transport,
// execution, or Audit authority is enabled by this route.
func TestRunAcceptedExecuteRunnerExecutionIntentAcrossClients(t *testing.T) {
	configuredRuntime := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredRuntime == "" {
		t.Skip("Runner execution-intent E2E requires a configured Forge Runtime binary for durable Prompt/Run binding")
	}
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)

	root := privateTestParent(t)
	stateDir := filepath.Join(root, "app-state-runner-execution-intent-e2e")
	runtimeStateDir := filepath.Join(root, "runtime-state-runner-execution-intent-e2e")
	if err := os.Mkdir(runtimeStateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	runtimeExecutable := filepath.Join(root, "forge-runtime-runner-execution-intent-e2e")
	runtimeExecutable = configuredRuntime
	initializeRuntimeHubForIntegration(t, runtimeExecutable, runtimeStateDir)

	owner := deviceidentity.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	conversationID := "conversation-runner-execution-intent-e2e"
	promptID := "prompt-runner-execution-intent-e2e"
	runID := "run-runner-execution-intent-e2e"
	if configuredRuntime != "" {
		projectPath, projectID, _ := seedRuntimeConversationScopes(t, runtimeExecutable, runtimeStateDir)
		if err := os.WriteFile(filepath.Join(projectPath, "README.md"), []byte("deterministic Runner execution-intent fixture\n"), 0o600); err != nil {
			t.Fatal(err)
		}
		bridgeStateDir := filepath.Join(root, "bridge-state-runner-execution-intent-e2e")
		if err := os.Mkdir(bridgeStateDir, 0o700); err != nil {
			t.Fatal(err)
		}
		bridge, err := runtimebridge.New(runtimebridge.Config{
			Executable: runtimeExecutable, AppServerStateDir: bridgeStateDir, RuntimeStateDir: runtimeStateDir,
			Timeout: 5 * time.Second,
		})
		if err != nil {
			t.Fatal(err)
		}
		conversation, err := bridge.CreateOwnedConversation(context.Background(),
			model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			model.ConversationScope{Kind: "project", ID: projectID}, "Runner execution-intent E2E", "runner-execution-intent-e2e-create")
		if err != nil {
			t.Fatalf("create Runtime conversation for Runner execution-intent E2E: %v", err)
		}
		conversationID = conversation.ID
		prompt, _, replayed, err := bridge.AppendOwnedPrompt(context.Background(),
			model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, "review this Runner execution intent", "runner-execution-intent-e2e-prompt", 1)
		if err != nil || replayed || prompt.ID == "" {
			t.Fatalf("append Runtime Prompt for Runner execution-intent E2E: prompt=%#v replayed=%v err=%v", prompt, replayed, err)
		}
		promptID = prompt.ID
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		command := exec.CommandContext(ctx, runtimeExecutable,
			"--state-dir", runtimeStateDir, "--json", "--idempotency-key", "runner-execution-intent-e2e-start",
			"-C", projectPath, "run", "start", conversationID, prompt.ID, "--read", "README.md")
		command.Env = []string{}
		output, err := command.CombinedOutput()
		cancel()
		if err != nil || len(output) == 0 {
			t.Fatalf("seed deterministic Runner execution-intent Run: %v: %s", err, output)
		}
		bridgeRuns, err := bridge.OwnedConversationRuns(context.Background(),
			model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, nil, 25)
		if err != nil || len(bridgeRuns.Runs) != 1 || bridgeRuns.Runs[0].PromptID != prompt.ID {
			t.Fatalf("read seeded Runner execution-intent Run page=%#v err=%v", bridgeRuns, err)
		}
		runID = bridgeRuns.Runs[0].RunID
	}

	state := lifecycleRegistrySourceState(t, owner, "device-a", "runner-execution-intent-e2e", 1)
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})
	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{
		Status: "accepted", AcceptanceID: "p4-runner-execution-intent-e2e-001", AcceptedAtUnixMS: 1,
	}
	activation.Evidence.RunnerIsolation = true
	activation.Evidence.LeaseFencing = true
	activation.Evidence.CancellationAndUncertainWork = true
	activation.Evidence.VaultArtifactAuthorization = true
	activation.Evidence.AuditOutbox = true
	config := Config{
		ListenAddress: "127.0.0.1:0", StateDir: stateDir, Build: BuildInfo{Version: "test"},
		RuntimeExecutable: runtimeExecutable, RuntimeStateDir: runtimeStateDir,
		SnaplinkIssuer: issuer, SnaplinkAudience: snaplinkForgeTestAudience,
		SnaplinkJWKSURL: issuer + "/.well-known/jwks.json", ExpectedTenantID: snaplinkForgeTestTenant,
		ExpectedSubjectID: snaplinkForgeTestUser, JWKSHTTPClient: ssoClient,
		JWKSRefreshInterval: 24 * time.Hour, DeviceFabricActivation: ptrDeviceFabricRequest(activation),
		DeviceInventoryLifecycleRegistryFile: registryPath,
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	readyChannel := make(chan Ready, 1)
	result := make(chan error, 1)
	go func() {
		result <- Run(ctx, config, func(ready Ready) error { readyChannel <- ready; return nil })
	}()
	var ready Ready
	select {
	case ready = <-readyChannel:
	case err := <-result:
		t.Fatalf("Runner execution-intent E2E server failed before readiness: %v", err)
	case <-time.After(5 * time.Second):
		t.Fatal("Runner execution-intent E2E server did not announce readiness")
	}
	t.Cleanup(func() {
		cancel()
		if err := waitResult(t, result); err != nil {
			t.Error(err)
		}
	})

	request := acceptedRunnerExecutionIntentRequest(owner, conversationID, promptID, runID)
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	client := &http.Client{Timeout: 5 * time.Second}
	post := func(payload []byte) (int, []byte) {
		t.Helper()
		req, err := http.NewRequest(http.MethodPost,
			ready.Listen+runnerExecutionIntentPreviewPathURL(conversationID, runID), strings.NewReader(string(payload)))
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
		value, err := io.ReadAll(response.Body)
		if err != nil {
			t.Fatal(err)
		}
		return response.StatusCode, value
	}

	status, responseBody := post(body)
	if status != http.StatusOK {
		t.Fatalf("Runner execution-intent status=%d body=%q", status, responseBody)
	}
	var observation deviceplacement.RunnerExecutionIntentObservation
	if err := json.Unmarshal(responseBody, &observation); err != nil {
		t.Fatalf("decode Runner execution-intent: %v body=%q", err, responseBody)
	}
	assertAcceptedRunnerExecutionIntentE2E(t, observation, request, owner)
	for _, forbidden := range []string{"fencing_token", "argv", "workspace_ref"} {
		if strings.Contains(string(responseBody), forbidden) {
			t.Fatalf("Runner execution-intent leaked %q: %q", forbidden, responseBody)
		}
	}

	if configuredRuntime != "" {
		runForgeRuntimeRunnerExecutionIntentRemoteCLI(t, runtimeExecutable, ready.Listen, token, body, request, owner)
		runForgeRuntimeRunnerExecutionIntentRemoteTUI(t, runtimeExecutable, ready.Listen, token, body, request)
	}
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		runForgeConsoleRunnerExecutionIntentE2EWithToken(t, ready.Listen, token, body, request)
		if configuredRuntime != "" {
			runForgeConsoleRunnerExecutionIntentGateE2EWithToken(t, ready.Listen, token, request)
		}
	}
}

func runnerExecutionIntentPreviewPathURL(conversationID, runID string) string {
	return "/api/v1/conversations/" + conversationID + "/runs/" + runID + "/runner-execution-intent/preview"
}

func acceptedRunnerExecutionIntentRequest(owner deviceidentity.Owner, conversationID, promptID, runID string) deviceplacement.RunnerExecutionIntentRequest {
	attemptID := "attempt-runner-execution-intent-e2e"
	commandID := "command-runner-execution-intent-e2e"
	targetID := "runner-execution-intent-e2e"
	idempotencyKey := runID + ":" + attemptID + ":" + commandID
	command := deviceplacement.RunnerExecutionCommand{
		V: 1, CommandID: commandID,
		// secret-scan:ignore — deterministic lease-proof fixture, never used for execution.
		LeaseProof:     deviceplacement.RunnerExecutionLeaseProof{AttemptID: attemptID, TargetID: targetID, Epoch: 1, FencingToken: "fence-runner-execution-intent-e2e"},
		IdempotencyKey: idempotencyKey, WorkspaceRef: "workspace-runner-execution-intent-e2e",
		Argv: []string{"forge-task", "--prompt-ref", promptID}, TimeoutMS: 5_000, MaxOutputBytes: 65_536,
	}
	digest, err := (deviceplacement.RunnerTerminalCommand{
		V: command.V, CommandID: command.CommandID,
		LeaseProof: deviceplacement.RunnerTerminalLeaseProof{
			AttemptID: command.LeaseProof.AttemptID, TargetID: command.LeaseProof.TargetID,
			Epoch: command.LeaseProof.Epoch, FencingToken: command.LeaseProof.FencingToken,
		}, IdempotencyKey: command.IdempotencyKey, WorkspaceRef: command.WorkspaceRef,
		Argv: command.Argv, TimeoutMS: command.TimeoutMS, MaxOutputBytes: command.MaxOutputBytes,
	}).CommandSHA256()
	if err != nil {
		panic("Runner execution-intent E2E command digest: " + err.Error())
	}
	return deviceplacement.RunnerExecutionIntentRequest{
		Owner: ownerToPlacement(owner), ConversationID: conversationID,
		Prompt: deviceplacement.RunIntentPromptReceipt{
			PromptID: promptID, ConversationID: conversationID, Role: "user", AcceptedAtMS: 200,
			IntentID: "intent-runner-execution-intent-e2e", InitialEventID: "event-runner-execution-intent-e2e", InitialEventSequence: 1, InitialEventType: "submitted",
		},
		Run: deviceplacement.RunIntentRunReference{
			RunID: runID, ConversationID: conversationID, PromptID: promptID, CreatedAtMS: 200,
			LatestSequence: 5, Status: "completed",
		},
		Binding: deviceplacement.RunnerExecutionIntentBinding{
			ConversationID: conversationID, PromptID: promptID, RunID: runID, AttemptID: attemptID,
			CommandID: commandID, TargetID: targetID, CommandSHA256: digest, IdempotencyKey: idempotencyKey,
		},
		Command: command,
	}
}

func assertAcceptedRunnerExecutionIntentE2E(t *testing.T, observation deviceplacement.RunnerExecutionIntentObservation, request deviceplacement.RunnerExecutionIntentRequest, owner deviceidentity.Owner) {
	t.Helper()
	expected, err := deviceplacement.ObserveRunnerExecutionIntent(request)
	if err != nil || observation != expected || observation.Owner != ownerToPlacement(owner) {
		t.Fatalf("unexpected Runner execution-intent observation=%#v expected=%#v err=%v", observation, expected, err)
	}
}

func runForgeRuntimeRunnerExecutionIntentRemoteCLI(t *testing.T, executable, apiURL, accessToken string, body []byte, request deviceplacement.RunnerExecutionIntentRequest, owner deviceidentity.Owner) {
	t.Helper()
	inputPath := filepath.Join(t.TempDir(), "runner-execution-intent-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatal(err)
	}
	output, stderr, err := runForgeRuntimeCLI(t, executable, apiURL, accessToken, t.TempDir(), "--json", "remote", "runner-execution-intent-preview", "--input", inputPath)
	if err != nil {
		t.Fatalf("authenticated Runner execution-intent Runtime CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var observation deviceplacement.RunnerExecutionIntentObservation
	if err := json.Unmarshal([]byte(output), &observation); err != nil {
		t.Fatalf("decode Runner execution-intent Runtime CLI: %v stdout=%q", err, output)
	}
	assertAcceptedRunnerExecutionIntentE2E(t, observation, request, owner)
}

func runForgeRuntimeRunnerExecutionIntentRemoteTUI(t *testing.T, executable, apiURL, accessToken string, body []byte, request deviceplacement.RunnerExecutionIntentRequest) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("Runner execution-intent TUI E2E requires script: %v", err)
	}
	inputPath := filepath.Join(t.TempDir(), "runner-execution-intent-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatal(err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript, "-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("open " + request.ConversationID + "\nrunner-execution-intent-remote-preview --input " + inputPath + "\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated Runner execution-intent TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Runner execution-intent TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{"remote Runner execution intent preview", "conversation=" + request.ConversationID, "prompt_run_binding_valid=true runner_command_binding_valid=true preview_only=true selected_target=none", "execution_authorized=false", "dispatch_performed=false"} {
		if !strings.Contains(output, want) {
			t.Fatalf("Runner execution-intent TUI omitted %q: %q", want, output)
		}
	}
	if strings.Contains(output, request.Command.LeaseProof.FencingToken) || strings.Contains(output, request.Command.WorkspaceRef) || strings.Contains(output, request.Command.Argv[0]) {
		t.Fatalf("Runner execution-intent TUI leaked command proof: %q", output)
	}
}

func runForgeConsoleRunnerExecutionIntentE2EWithToken(t *testing.T, apiURL, token string, body []byte, request deviceplacement.RunnerExecutionIntentRequest) {
	t.Helper()
	consoleRoot := os.Getenv("SNAPLINK_CONSOLE_ROOT")
	if consoleRoot == "" {
		workingDirectory, err := os.Getwd()
		if err != nil {
			t.Fatal(err)
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
		t.Fatalf("Flutter is required for Runner execution-intent E2E: %v", err)
	}
	inputJSON, err := json.Marshal(map[string]any{
		"api_url": apiURL, "access_token": token, "request": request,
	})
	if err != nil {
		t.Fatal(err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "runner-execution-intent-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatal(err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable, "test", "--no-pub", "--dart-define=FORGE_CONVERSATIONS_API_ORIGIN="+apiURL, "test/forge_runner_execution_intent_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(forgeConsoleTestEnvironment(inputPath, flutterHome), "FORGE_RUNNER_EXECUTION_INTENT_E2E_INPUT="+inputPath)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter Runner execution-intent E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Runner execution-intent Flutter output exceeded the size limit")
	}
}

func runForgeConsoleRunnerExecutionIntentGateE2EWithToken(t *testing.T, apiURL, token string, request deviceplacement.RunnerExecutionIntentRequest) {
	t.Helper()
	consoleRoot := os.Getenv("SNAPLINK_CONSOLE_ROOT")
	if consoleRoot == "" {
		workingDirectory, err := os.Getwd()
		if err != nil {
			t.Fatal(err)
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
		t.Fatalf("Flutter is required for Runner execution-intent Gate E2E: %v", err)
	}
	inputJSON, err := json.Marshal(map[string]any{
		"api_url": apiURL, "access_token": token, "request": request,
	})
	if err != nil {
		t.Fatal(err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "runner-execution-intent-gate-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatal(err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable, "test", "--no-pub", "--dart-define=FORGE_CONVERSATIONS_API_ORIGIN="+apiURL, "test/forge_runner_execution_intent_gate_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(forgeConsoleTestEnvironment(inputPath, flutterHome), "FORGE_RUNNER_EXECUTION_INTENT_GATE_E2E_INPUT="+inputPath)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter Runner execution-intent Gate E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Runner execution-intent Gate Flutter output exceeded the size limit")
	}
}
