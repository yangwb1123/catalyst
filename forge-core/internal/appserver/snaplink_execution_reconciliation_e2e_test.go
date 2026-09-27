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
	"forgeos/forge-core/internal/executionlease"
	"forgeos/forge-core/internal/executionreconcile"
	"forgeos/forge-core/internal/runtimebridge"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// TestRunAcceptedExecuteExecutionReconciliationAcrossClients crosses the
// accepted EXECUTE + P4 production assembly with one caller-supplied terminal
// uncertainty image. Core, Runtime CLI/TUI, and the opt-in Console API must
// classify the same image as manual reconciliation while keeping all effect
// authority false and withholding proof/reason/output material.
func TestRunAcceptedExecuteExecutionReconciliationAcrossClients(t *testing.T) {
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)

	root := privateTestParent(t)
	stateDir := filepath.Join(root, "app-state-execution-reconciliation-e2e")
	runtimeStateDir := filepath.Join(root, "runtime-state-execution-reconciliation-e2e")
	if err := os.Mkdir(runtimeStateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	runtimeExecutable := filepath.Join(root, "forge-runtime-execution-reconciliation-e2e")
	configuredRuntime := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredRuntime != "" {
		runtimeExecutable = configuredRuntime
		initializeRuntimeHubForIntegration(t, runtimeExecutable, runtimeStateDir)
	} else if err := os.WriteFile(runtimeExecutable, []byte("test runtime"), 0o700); err != nil {
		t.Fatal(err)
	}

	owner := deviceidentity.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	conversationID := "conversation-execution-reconciliation-e2e"
	runID := "run-execution-reconciliation-e2e"
	if configuredRuntime != "" {
		projectPath, projectID, _ := seedRuntimeConversationScopes(t, runtimeExecutable, runtimeStateDir)
		if err := os.WriteFile(filepath.Join(projectPath, "README.md"), []byte("deterministic execution reconciliation fixture\n"), 0o600); err != nil {
			t.Fatal(err)
		}
		bridgeStateDir := filepath.Join(root, "bridge-state-execution-reconciliation-e2e")
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
			model.ConversationScope{Kind: "project", ID: projectID}, "Execution reconciliation E2E", "execution-reconciliation-e2e-create")
		if err != nil {
			t.Fatalf("create Runtime conversation for reconciliation E2E: %v", err)
		}
		conversationID = conversation.ID
		prompt, _, replayed, err := bridge.AppendOwnedPrompt(
			context.Background(),
			model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, "review this uncertain execution", "execution-reconciliation-e2e-prompt", 1,
		)
		if err != nil || replayed || prompt.ID == "" {
			t.Fatalf("append Runtime Prompt for reconciliation E2E: prompt=%#v replayed=%v err=%v", prompt, replayed, err)
		}
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		command := exec.CommandContext(ctx, runtimeExecutable,
			"--state-dir", runtimeStateDir, "--json", "--idempotency-key", "execution-reconciliation-e2e-start",
			"-C", projectPath, "run", "start", conversationID, prompt.ID, "--read", "README.md")
		command.Env = []string{}
		output, err := command.CombinedOutput()
		cancel()
		if err != nil || len(output) == 0 {
			t.Fatalf("seed deterministic reconciliation Run: %v: %s", err, output)
		}
		page, err := bridge.OwnedConversationRuns(
			context.Background(),
			model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, nil, 25,
		)
		if err != nil || len(page.Runs) != 1 || page.Runs[0].PromptID != prompt.ID {
			t.Fatalf("read seeded reconciliation Run page=%#v err=%v", page, err)
		}
		runID = page.Runs[0].RunID
	}

	state := lifecycleRegistrySourceState(t, owner, "device-a", "runner-reconciliation-e2e", 1)
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})

	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{
		Status: "accepted", AcceptanceID: "p4-execution-reconciliation-e2e-001", AcceptedAtUnixMS: 1,
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
		t.Fatalf("execution reconciliation E2E server failed before readiness: %v", err)
	case <-time.After(5 * time.Second):
		t.Fatal("execution reconciliation E2E server did not announce readiness")
	}
	t.Cleanup(func() {
		cancel()
		if err := waitResult(t, result); err != nil {
			t.Error(err)
		}
	})

	input := acceptedExecutionReconciliationInput(t, ownerToPlacement(owner), conversationID, runID)
	body, err := json.Marshal(input)
	if err != nil {
		t.Fatal(err)
	}
	client := &http.Client{Timeout: 5 * time.Second}
	post := func(payload []byte) (int, []byte) {
		t.Helper()
		req, err := http.NewRequest(http.MethodPost,
			ready.Listen+executionReconciliationPathIDsURL(input.ConversationID, input.RunID),
			strings.NewReader(string(payload)))
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
		t.Fatalf("execution reconciliation status=%d body=%q", status, responseBody)
	}
	var observation executionreconcile.Observation
	if err := json.Unmarshal(responseBody, &observation); err != nil {
		t.Fatalf("decode execution reconciliation: %v body=%q", err, responseBody)
	}
	assertExecutionReconciliationE2E(t, observation, input, ownerToPlacement(owner))
	for _, forbidden := range []string{"fencing_token", "transport ended after effect boundary", "receipt_sha256", "argv", "workspace"} {
		if strings.Contains(string(responseBody), forbidden) {
			t.Fatalf("execution reconciliation leaked %q: %q", forbidden, responseBody)
		}
	}

	if configuredRuntime != "" {
		runForgeRuntimeExecutionReconciliationRemoteCLI(t, runtimeExecutable, ready.Listen, token, body, input, ownerToPlacement(owner))
		runForgeRuntimeExecutionReconciliationRemoteTUI(t, runtimeExecutable, ready.Listen, token, body, input)
	}
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		runForgeConsoleExecutionReconciliationE2EWithToken(t, ready.Listen, token, input, ownerToPlacement(owner))
		if configuredRuntime != "" {
			runForgeConsoleExecutionReconciliationGateE2EWithToken(t, ready.Listen, token, input)
		}
	}
}

func executionReconciliationPathIDsURL(conversationID, runID string) string {
	return "/api/v1/conversations/" + conversationID + "/runs/" + runID + "/execution-reconciliation/preview"
}

func acceptedExecutionReconciliationInput(
	t *testing.T, owner deviceplacement.Owner, conversationID, runID string,
) executionreconcile.Input {
	t.Helper()
	lease, err := executionlease.Issue(
		"attempt-execution-reconciliation-e2e", "runner-reconciliation-e2e", 1,
		"token-execution-reconciliation-e2e", 100, 10_000,
	)
	if err != nil {
		t.Fatal(err)
	}
	return executionreconcile.Input{
		Owner: owner, ConversationID: conversationID, RunID: runID,
		AttemptID: "attempt-execution-reconciliation-e2e", CommandID: "command-execution-reconciliation-e2e",
		TargetID: "runner-reconciliation-e2e", RunStatus: "nonterminal", AttemptState: "uncertain",
		Lease: lease, ObservedAtMS: 400,
		Terminal: &executionlease.TerminalReceipt{
			V: executionlease.ExecutionLeaseABIVersion, Proof: lease.Proof(),
			Disposition:  executionlease.TerminalDisposition{Kind: "uncertain", Reason: "transport ended after effect boundary"},
			ObservedAtMS: 300,
		},
	}
}

func assertExecutionReconciliationE2E(
	t *testing.T, observation executionreconcile.Observation,
	input executionreconcile.Input, owner deviceplacement.Owner,
) {
	t.Helper()
	if err := observation.Validate(); err != nil || observation.Owner != owner ||
		observation.ConversationID != input.ConversationID || observation.RunID != input.RunID ||
		observation.AttemptID != input.AttemptID || observation.CommandID != input.CommandID ||
		observation.TargetID != input.TargetID || observation.NextObservation != "terminal_uncertain" ||
		!observation.TerminalObserved || observation.TerminalDisposition != "uncertain" ||
		!observation.TerminalStateAligned || !observation.ReconciliationRequired ||
		!observation.ManualReviewRequired || observation.AutomaticRetry || !observation.PreviewOnly ||
		observation.Authority != (executionreconcile.Authority{}) {
		t.Fatalf("execution reconciliation observation=%#v err=%v", observation, err)
	}
}

func runForgeRuntimeExecutionReconciliationRemoteCLI(
	t *testing.T, executable, apiURL, accessToken string, body []byte,
	input executionreconcile.Input, owner deviceplacement.Owner,
) {
	t.Helper()
	inputPath := filepath.Join(t.TempDir(), "execution-reconciliation-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatalf("write execution reconciliation request for Runtime CLI: %v", err)
	}
	output, stderr, err := runForgeRuntimeCLI(t, executable, apiURL, accessToken, t.TempDir(), "--json", "remote", "execution-reconciliation", "preview", "--input", inputPath)
	if err != nil {
		t.Fatalf("authenticated execution reconciliation Runtime CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var value executionreconcile.Observation
	if err := json.Unmarshal([]byte(output), &value); err != nil {
		t.Fatalf("decode execution reconciliation Runtime CLI: %v stdout=%q", err, output)
	}
	assertExecutionReconciliationE2E(t, value, input, owner)
}

func runForgeRuntimeExecutionReconciliationRemoteTUI(
	t *testing.T, executable, apiURL, accessToken string, body []byte,
	input executionreconcile.Input,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("execution reconciliation TUI E2E requires script: %v", err)
	}
	inputPath := filepath.Join(t.TempDir(), "execution-reconciliation-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatalf("write execution reconciliation request for TUI: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript, "-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("open " + input.ConversationID + "\nexecution-reconciliation-preview --input " + inputPath + "\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated execution reconciliation TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("execution reconciliation TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"execution reconciliation preview [forge.execution-reconciliation-observation/v1]",
		"next_observation=terminal_uncertain", "reconciliation_required=true", "automatic_retry=false",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("execution reconciliation TUI output omitted %q: %q", want, output)
		}
	}
	if strings.Contains(output, "token-execution-reconciliation-e2e") || strings.Contains(output, "transport ended after effect boundary") || strings.Contains(output, "fencing_token") {
		t.Fatalf("execution reconciliation TUI leaked proof or terminal detail: %q", output)
	}
}

func runForgeConsoleExecutionReconciliationE2EWithToken(
	t *testing.T, apiURL, token string, input executionreconcile.Input, owner deviceplacement.Owner,
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
		t.Fatalf("Flutter is required for execution reconciliation E2E: %v", err)
	}
	inputJSON, err := json.Marshal(struct {
		APIURL      string                   `json:"api_url"`
		AccessToken string                   `json:"access_token"`
		Owner       model.Owner              `json:"owner"`
		Request     executionreconcile.Input `json:"request"`
	}{
		APIURL: apiURL, AccessToken: token,
		Owner:   model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		Request: input,
	})
	if err != nil {
		t.Fatalf("encode Flutter execution reconciliation input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "execution-reconciliation-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write Flutter execution reconciliation input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable, "test", "--no-pub", "test/forge_execution_reconciliation_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(forgeConsoleTestEnvironment(inputPath, flutterHome), "FORGE_EXECUTION_RECONCILIATION_E2E_INPUT="+inputPath)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter execution reconciliation E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("execution reconciliation Flutter output exceeded the size limit")
	}
}

func runForgeConsoleExecutionReconciliationGateE2EWithToken(
	t *testing.T, apiURL, token string, input executionreconcile.Input,
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
		t.Fatalf("Flutter is required for execution reconciliation Gate E2E: %v", err)
	}
	inputJSON, err := json.Marshal(struct {
		APIURL      string                   `json:"api_url"`
		AccessToken string                   `json:"access_token"`
		Request     executionreconcile.Input `json:"request"`
	}{APIURL: apiURL, AccessToken: token, Request: input})
	if err != nil {
		t.Fatalf("encode Flutter execution reconciliation Gate input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "execution-reconciliation-gate-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter execution reconciliation Gate input: %v", err)
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
		"test/forge_execution_reconciliation_gate_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_EXECUTION_RECONCILIATION_GATE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter execution reconciliation Gate E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("execution reconciliation Gate Flutter output exceeded the size limit")
	}
}
