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

	"forgeos/forge-core/internal/auditprojection"
	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/runtimebridge"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"forgeos/forge-core/internal/runtimebridge/runmodel"
)

// TestRunAcceptedExecuteRunExecutionEvidenceAcrossClients crosses the
// accepted EXECUTE + P4 assembly with one content-free Run/receipt binding.
// Core, Runtime CLI/TUI, and optional Console Web/App/Mobile clients consume
// the same result while every execution authority bit stays disabled.
func TestRunAcceptedExecuteRunExecutionEvidenceAcrossClients(t *testing.T) {
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)

	root := privateTestParent(t)
	stateDir := filepath.Join(root, "app-state-run-execution-evidence-e2e")
	runtimeStateDir := filepath.Join(root, "runtime-state-run-execution-evidence-e2e")
	if err := os.Mkdir(runtimeStateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	runtimeExecutable := filepath.Join(root, "forge-runtime-run-execution-evidence-e2e")
	configuredRuntime := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredRuntime != "" {
		runtimeExecutable = configuredRuntime
		initializeRuntimeHubForIntegration(t, runtimeExecutable, runtimeStateDir)
	} else if err := os.WriteFile(runtimeExecutable, []byte("test runtime"), 0o700); err != nil {
		t.Fatal(err)
	}

	owner := deviceidentity.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	conversationID := "conversation-run-execution-evidence-e2e"
	promptID := "prompt-run-execution-evidence-e2e"
	runID := "run-run-execution-evidence-e2e"
	if configuredRuntime != "" {
		projectPath, projectID, _ := seedRuntimeConversationScopes(t, runtimeExecutable, runtimeStateDir)
		if err := os.WriteFile(filepath.Join(projectPath, "README.md"), []byte("deterministic Run execution evidence fixture\n"), 0o600); err != nil {
			t.Fatal(err)
		}
		bridgeStateDir := filepath.Join(root, "bridge-state-run-execution-evidence-e2e")
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
			model.ConversationScope{Kind: "project", ID: projectID}, "Run execution evidence E2E", "run-execution-evidence-e2e-create")
		if err != nil {
			t.Fatalf("create Runtime conversation for Run evidence TUI E2E: %v", err)
		}
		conversationID = conversation.ID
		prompt, _, replayed, err := bridge.AppendOwnedPrompt(context.Background(),
			model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, "observe this completed Run execution evidence", "run-execution-evidence-e2e-prompt", 1)
		if err != nil || replayed || prompt.ID == "" {
			t.Fatalf("append Runtime Prompt for Run evidence E2E: prompt=%#v replayed=%v err=%v", prompt, replayed, err)
		}
		promptID = prompt.ID
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		command := exec.CommandContext(ctx, runtimeExecutable,
			"--state-dir", runtimeStateDir, "--json", "--idempotency-key", "run-execution-evidence-e2e-start",
			"-C", projectPath, "run", "start", conversationID, promptID, "--read", "README.md")
		command.Env = []string{}
		output, err := command.CombinedOutput()
		cancel()
		if err != nil || len(output) == 0 {
			t.Fatalf("seed deterministic Run execution evidence: %v: %s", err, output)
		}
		page, err := bridge.OwnedConversationRuns(context.Background(),
			model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, nil, 25)
		if err != nil || len(page.Runs) != 1 || page.Runs[0].PromptID != promptID {
			t.Fatalf("read seeded Run execution evidence page=%#v err=%v", page, err)
		}
		runID = page.Runs[0].RunID
	}

	state := lifecycleRegistrySourceState(t, owner, "device-a", "runner-run-execution-evidence-e2e", 1)
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})
	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{
		Status: "accepted", AcceptanceID: "p4-run-execution-evidence-e2e-001", AcceptedAtUnixMS: 1,
	}
	activation.Evidence.RunnerIsolation = true
	activation.Evidence.LeaseFencing = true
	activation.Evidence.CancellationAndUncertainWork = true
	activation.Evidence.VaultArtifactAuthorization = true
	activation.Evidence.AuditOutbox = true
	config := Config{
		ListenAddress: "127.0.0.1:0", StateDir: stateDir, Build: BuildInfo{Version: "test"},
		RuntimeExecutable: runtimeExecutable, RuntimeStateDir: runtimeStateDir,
		SnaplinkIssuer: issuer, SnaplinkAudience: snaplinkForgeTestAudience, SnaplinkJWKSURL: issuer + "/.well-known/jwks.json",
		ExpectedTenantID: snaplinkForgeTestTenant, ExpectedSubjectID: snaplinkForgeTestUser, JWKSHTTPClient: ssoClient,
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
		t.Fatalf("Run evidence E2E server failed before readiness: %v", err)
	case <-time.After(5 * time.Second):
		t.Fatal("Run evidence E2E server did not announce readiness")
	}
	t.Cleanup(func() {
		cancel()
		if err := waitResult(t, result); err != nil {
			t.Error(err)
		}
	})

	receiptBody := sessionRunnerReceiptObservationPreviewBody(
		t, owner.Issuer, owner.Subject, owner.TenantID, conversationID, promptID, runID,
	)
	var receipt deviceplacement.SessionRunnerReceiptObservation
	if err := json.Unmarshal([]byte(receiptBody), &receipt); err != nil {
		t.Fatal(err)
	}
	runObserved, err := auditprojection.ProjectRunObserved(
		model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		conversationID,
		runmodel.OwnedRunSummary{RunID: runID, PromptID: promptID, CreatedAtMS: 200, LatestSequence: 5, Status: "completed"},
	)
	if err != nil {
		t.Fatal(err)
	}
	requestBody, err := json.Marshal(struct {
		RunObserved            auditprojection.RunObserved                     `json:"run_observed"`
		SessionReceiptObserved deviceplacement.SessionRunnerReceiptObservation `json:"session_receipt_observed"`
	}{RunObserved: runObserved, SessionReceiptObserved: receipt})
	if err != nil {
		t.Fatal(err)
	}
	client := &http.Client{Timeout: 5 * time.Second}
	post := func(payload []byte) (int, []byte) {
		t.Helper()
		req, err := http.NewRequest(http.MethodPost, ready.Listen+runExecutionEvidencePathURL(conversationID, runID), strings.NewReader(string(payload)))
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

	status, responseBody := post(requestBody)
	if status != http.StatusOK {
		t.Fatalf("Run execution evidence status=%d body=%q", status, responseBody)
	}
	var evidence deviceplacement.RunExecutionEvidence
	if err := json.Unmarshal(responseBody, &evidence); err != nil {
		t.Fatalf("decode Run execution evidence: %v body=%q", err, responseBody)
	}
	assertAcceptedRunExecutionEvidenceE2E(t, evidence, runObserved, receipt)
	if strings.Contains(string(responseBody), "issuer") || strings.Contains(string(responseBody), "fencing_token") || strings.Contains(string(responseBody), "prompt_content") {
		t.Fatalf("Run execution evidence leaked owner/proof/content: %q", responseBody)
	}

	if configuredRuntime != "" {
		runForgeRuntimeRunExecutionEvidenceRemoteCLI(t, runtimeExecutable, ready.Listen, token, requestBody, runObserved, receipt)
		runForgeRuntimeRunExecutionEvidenceRemoteTUI(t, runtimeExecutable, ready.Listen, token, requestBody, conversationID)
	}
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		runForgeConsoleRunExecutionEvidenceE2EWithToken(t, ready.Listen, token, requestBody, runObserved, receipt)
	}
}

func runExecutionEvidencePathURL(conversationID, runID string) string {
	return "/api/v1/conversations/" + conversationID + "/runs/" + runID + "/execution-evidence/preview"
}

func assertAcceptedRunExecutionEvidenceE2E(t *testing.T, evidence deviceplacement.RunExecutionEvidence, run auditprojection.RunObserved, receipt deviceplacement.SessionRunnerReceiptObservation) {
	t.Helper()
	if evidence.SchemaVersion != deviceplacement.RunExecutionEvidenceSchemaVersion || evidence.ConversationID != run.ConversationID || evidence.RunID != run.RunID || evidence.PromptID != run.PromptID || evidence.OwnerRef != run.OwnerRef || evidence.RunStatus != run.Status || evidence.AttemptID != receipt.ReceiptObservation.AttemptID || evidence.TargetID != receipt.ReceiptObservation.TargetID || evidence.CommandID != receipt.ReceiptObservation.CommandID || evidence.CommandSHA256 != receipt.ReceiptObservation.CommandSHA256 || evidence.DispositionKind != receipt.ReceiptObservation.DispositionKind || evidence.ReceiptObservedAtMS != uint64(receipt.ReceiptObservation.ObservedAtMS) || !evidence.MetadataObserved || evidence.ContentIncluded || evidence.Authority != (deviceplacement.RunExecutionEvidenceAuthority{}) {
		t.Fatalf("unexpected Run execution evidence=%#v", evidence)
	}
}

func runForgeRuntimeRunExecutionEvidenceRemoteCLI(t *testing.T, executable, apiURL, accessToken string, body []byte, run auditprojection.RunObserved, receipt deviceplacement.SessionRunnerReceiptObservation) {
	t.Helper()
	inputPath := filepath.Join(t.TempDir(), "run-execution-evidence-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatal(err)
	}
	output, stderr, err := runForgeRuntimeCLI(t, executable, apiURL, accessToken, t.TempDir(), "--json", "remote", "run-execution-evidence", "preview", "--input", inputPath)
	if err != nil {
		t.Fatalf("Run execution evidence Runtime CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var evidence deviceplacement.RunExecutionEvidence
	if err := json.Unmarshal([]byte(output), &evidence); err != nil {
		t.Fatal(err)
	}
	assertAcceptedRunExecutionEvidenceE2E(t, evidence, run, receipt)
}

func runForgeRuntimeRunExecutionEvidenceRemoteTUI(t *testing.T, executable, apiURL, accessToken string, body []byte, conversationID string) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("Run execution evidence TUI E2E requires script: %v", err)
	}
	inputPath := filepath.Join(t.TempDir(), "run-execution-evidence-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatal(err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript, "-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("open " + conversationID + "\nrun-execution-evidence-remote-preview --input " + inputPath + "\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Run execution evidence TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	output := stdoutBuffer.String()
	for _, want := range []string{"authenticated Run execution evidence [forge.run.execution-evidence.v1]", "disposition=completed", "content_included=false uncertain=false reconciliation_required=false", "execution_authorized=false dispatch_performed=false"} {
		if !strings.Contains(output, want) {
			t.Fatalf("Run execution evidence TUI omitted %q: %q", want, output)
		}
	}
	if strings.Contains(output, "issuer") || strings.Contains(output, "fencing_token") || strings.Contains(output, "/api/v1/devices") {
		t.Fatalf("Run execution evidence TUI leaked proof or device request: %q", output)
	}
}

func runForgeConsoleRunExecutionEvidenceE2EWithToken(t *testing.T, apiURL, token string, body []byte, run auditprojection.RunObserved, receipt deviceplacement.SessionRunnerReceiptObservation) {
	t.Helper()
	consoleRoot := os.Getenv("SNAPLINK_CONSOLE_ROOT")
	if consoleRoot == "" {
		workingDirectory, err := os.Getwd()
		if err != nil {
			t.Fatal(err)
		}
		consoleRoot = filepath.Join(filepath.Dir(filepath.Dir(filepath.Dir(workingDirectory))), "workspace", "demo", "snaplink-console")
	}
	if _, err := os.Stat(filepath.Join(consoleRoot, "pubspec.yaml")); err != nil {
		t.Fatal(err)
	}
	flutterBinary := os.Getenv("FLUTTER_BIN")
	if flutterBinary == "" {
		flutterBinary = "flutter"
	}
	flutterExecutable, err := exec.LookPath(flutterBinary)
	if err != nil {
		t.Fatal(err)
	}
	inputJSON, err := json.Marshal(map[string]any{
		"api_url": apiURL, "access_token": token, "conversation_id": run.ConversationID, "run_id": run.RunID,
		"run_observed": run, "session_receipt_observed": receipt,
	})
	if err != nil {
		t.Fatal(err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "run-execution-evidence-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatal(err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	flutterArgs := []string{"test", "--no-pub", "--dart-define=FORGE_CONVERSATIONS_API_ORIGIN=" + apiURL, "test/forge_run_execution_evidence_api_e2e_test.dart"}
	if os.Getenv("FORGE_RUNTIME_BIN") != "" {
		flutterArgs = append(flutterArgs, "test/forge_run_execution_evidence_gate_e2e_test.dart")
	}
	command := exec.CommandContext(ctx, flutterExecutable, flutterArgs...)
	command.Dir = consoleRoot
	command.Env = append(forgeConsoleTestEnvironment(inputPath, flutterHome), "FORGE_RUN_EXECUTION_EVIDENCE_E2E_INPUT="+inputPath)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter Run execution evidence E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
}
