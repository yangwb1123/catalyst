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
	"forgeos/forge-core/internal/runtimebridge"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// TestRunAcceptedExecuteRunnerDispatchAdmissionAcrossClients crosses the
// accepted EXECUTE + P4 production boundary with one already fenced lease.
// Core, Runtime, and the opt-in Console clients all consume the same
// owner/Run/Attempt/command proof. The response is a metadata-only admission
// recheck; releasing the proof makes the same request fail closed without
// contacting a Runner or authorizing command execution.
func TestRunAcceptedExecuteRunnerDispatchAdmissionAcrossClients(t *testing.T) {
	if os.Getenv("FORGE_RUNTIME_BIN") == "" {
		t.Skip("Runner dispatch admission E2E requires a configured Forge Runtime binary for durable Run binding")
	}
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)

	root := privateTestParent(t)
	stateDir := filepath.Join(root, "app-state-runner-admission-e2e")
	runtimeStateDir := filepath.Join(root, "runtime-state-runner-admission-e2e")
	if err := os.Mkdir(runtimeStateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	runtimeExecutable := filepath.Join(root, "forge-runtime-runner-admission-e2e")
	configuredRuntime := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredRuntime != "" {
		runtimeExecutable = configuredRuntime
		initializeRuntimeHubForIntegration(t, runtimeExecutable, runtimeStateDir)
	} else if err := os.WriteFile(runtimeExecutable, []byte("test runtime"), 0o700); err != nil {
		t.Fatal(err)
	}

	owner := deviceidentity.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	conversationID := "conversation-admission-e2e"
	runID := "run-admission-e2e"
	if configuredRuntime != "" {
		projectPath, projectID, _ := seedRuntimeConversationScopes(t, runtimeExecutable, runtimeStateDir)
		if err := os.WriteFile(filepath.Join(projectPath, "README.md"), []byte("deterministic Runner admission fixture\n"), 0o600); err != nil {
			t.Fatal(err)
		}
		bridgeStateDir := filepath.Join(root, "bridge-state-runner-admission-e2e")
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
			model.ConversationScope{Kind: "project", ID: projectID}, "Runner admission E2E", "runner-admission-e2e-create")
		if err != nil {
			t.Fatalf("create Runtime conversation for TUI admission E2E: %v", err)
		}
		conversationID = conversation.ID
		prompt, _, replayed, err := bridge.AppendOwnedPrompt(
			context.Background(),
			model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, "review this Runner dispatch admission", "runner-admission-e2e-prompt", 1,
		)
		if err != nil || replayed || prompt.ID == "" {
			t.Fatalf("append Runtime Prompt for Runner admission E2E: prompt=%#v replayed=%v err=%v", prompt, replayed, err)
		}
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		command := exec.CommandContext(ctx, runtimeExecutable,
			"--state-dir", runtimeStateDir, "--json", "--idempotency-key", "runner-admission-e2e-start",
			"-C", projectPath, "run", "start", conversationID, prompt.ID, "--read", "README.md")
		command.Env = []string{}
		output, err := command.CombinedOutput()
		cancel()
		if err != nil || len(output) == 0 {
			t.Fatalf("seed deterministic Runner admission Run: %v: %s", err, output)
		}
		page, err := bridge.OwnedConversationRuns(
			context.Background(),
			model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, nil, 25,
		)
		if err != nil || len(page.Runs) != 1 || page.Runs[0].PromptID != prompt.ID {
			t.Fatalf("read seeded Runner admission Run page=%#v err=%v", page, err)
		}
		runID = page.Runs[0].RunID
	}
	state := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})
	leasePath := filepath.Join(filepath.Dir(registryPath), "leases.json")
	leaseAdapter, err := deviceinventory.NewPersistedExecutionLeaseRegistryFileSetWriteAdapterWithTokenSource(
		leasePath, owner, func() (string, error) { return "token-admission-a", nil },
	)
	if err != nil {
		t.Fatal(err)
	}
	now := uint64(time.Now().UnixMilli())
	entry, replayed, err := leaseAdapter.Claim(context.Background(), executionlease.ClaimRequest{
		ConversationID: conversationID, RunID: runID, AttemptID: "attempt-admission-e2e",
		IdempotencyKey: "admission-claim-key-00000001", RequestSHA256: executionlease.RequestDigest([]byte("admission-claim")),
		IssuedAtMS: now - 1_000, TTLMS: 30_000,
	}, []executionlease.ClaimCandidate{{DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1, HeartbeatSequence: 1}})
	if err != nil || replayed {
		t.Fatalf("admission lease entry=%#v replayed=%v err=%v", entry, replayed, err)
	}

	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{Status: "accepted", AcceptanceID: "p4-runner-admission-e2e-001", AcceptedAtUnixMS: 1}
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
		DeviceExecutionLeaseRegistryFile:     leasePath,
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
		t.Fatalf("runner admission E2E server failed before readiness: %v", err)
	case <-time.After(5 * time.Second):
		t.Fatal("runner admission E2E server did not announce readiness")
	}
	t.Cleanup(func() {
		cancel()
		if err := waitResult(t, result); err != nil {
			t.Error(err)
		}
	})

	request := runnerDispatchAdmissionE2ERequest(owner, entry)
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	client := &http.Client{Timeout: 5 * time.Second}
	post := func(payload []byte) (int, []byte) {
		t.Helper()
		req, err := http.NewRequest(http.MethodPost,
			ready.Listen+runnerDispatchAdmissionPathIDsURL(request.ConversationID, request.RunID),
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
		t.Fatalf("runner admission status=%d body=%q", status, responseBody)
	}
	var observation deviceplacement.RunnerDispatchAdmissionObservation
	if err := json.Unmarshal(responseBody, &observation); err != nil {
		t.Fatalf("decode runner admission: %v body=%q", err, responseBody)
	}
	assertRunnerDispatchAdmissionE2EReady(t, observation, request, entry, owner)
	if strings.Contains(string(responseBody), "fencing_token") || strings.Contains(string(responseBody), "argv") || strings.Contains(string(responseBody), "workspace") {
		t.Fatalf("runner admission leaked command proof material: %q", responseBody)
	}

	if executable := os.Getenv("FORGE_RUNTIME_BIN"); executable != "" {
		runForgeRuntimeRunnerDispatchAdmissionRemoteCLI(t, executable, ready.Listen, token, body, request, entry, owner)
		runForgeRuntimeRunnerDispatchAdmissionRemoteTUI(t, executable, ready.Listen, token, body, request, entry)
	}
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		runForgeConsoleRunnerDispatchAdmissionE2EWithToken(t, ready.Listen, token, request, owner)
		if configuredRuntime != "" {
			runForgeConsoleRunnerDispatchAdmissionGateE2EWithToken(t, ready.Listen, token, request)
		}
	}

	releaseBody, err := json.Marshal(schedulerSelectionLeaseReleaseRequest{
		ConversationID: entry.ConversationID, RunID: entry.RunID, AttemptID: entry.AttemptID,
		TargetID: entry.Grant.TargetID, Epoch: entry.Grant.Epoch, FencingToken: entry.Grant.FencingToken,
	})
	if err != nil {
		t.Fatal(err)
	}
	releaseRequest, err := http.NewRequest(http.MethodPost, ready.Listen+schedulerSelectionLeaseReleasePath, strings.NewReader(string(releaseBody)))
	if err != nil {
		t.Fatal(err)
	}
	releaseRequest.Header.Set("Authorization", "Bearer "+token)
	releaseRequest.Header.Set("Content-Type", "application/json")
	releaseRequest.Header.Set("Idempotency-Key", "admission-release-key-00000001")
	releaseResponse, err := client.Do(releaseRequest)
	if err != nil {
		t.Fatal(err)
	}
	releasePayload, err := io.ReadAll(releaseResponse.Body)
	_ = releaseResponse.Body.Close()
	if err != nil {
		t.Fatal(err)
	}
	if releaseResponse.StatusCode != http.StatusOK {
		t.Fatalf("runner admission release status=%d body=%q", releaseResponse.StatusCode, releasePayload)
	}

	status, responseBody = post(body)
	if status != http.StatusOK {
		t.Fatalf("released runner admission status=%d body=%q", status, responseBody)
	}
	if err := json.Unmarshal(responseBody, &observation); err != nil {
		t.Fatal(err)
	}
	if err := observation.Validate(); err != nil || observation.AdmissionReady || observation.LeaseActive ||
		!containsAdmissionReason(observation.RejectionReasons, "lease_inactive_at_evaluated_time") ||
		observation.Authority != (deviceplacement.RunnerDispatchAdmissionAuthority{}) {
		t.Fatalf("released runner admission=%#v err=%v", observation, err)
	}
}

func runnerDispatchAdmissionPathIDsURL(conversationID, runID string) string {
	return "/api/v1/conversations/" + conversationID + "/runs/" + runID + "/runner-dispatch-admission/preview"
}

func runnerDispatchAdmissionE2ERequest(owner deviceidentity.Owner, entry executionlease.RegistryEntry) deviceplacement.RunnerDispatchAdmissionRequest {
	return deviceplacement.RunnerDispatchAdmissionRequest{
		Owner: ownerToPlacement(owner), ConversationID: entry.ConversationID, RunID: entry.RunID, AttemptID: entry.AttemptID,
		AttemptState: "accepted", EvaluatedAtMS: 1,
		Command: deviceplacement.RunnerExecutionCommand{
			V: 1, CommandID: "command-admission-e2e-1",
			LeaseProof: deviceplacement.RunnerExecutionLeaseProof{
				AttemptID: entry.AttemptID, TargetID: entry.Grant.TargetID, Epoch: entry.Grant.Epoch, FencingToken: entry.Grant.FencingToken,
			},
			IdempotencyKey: entry.RunID + ":" + entry.AttemptID + ":command-admission-e2e-1",
			WorkspaceRef:   "workspace-admission-e2e", Argv: []string{"forge-task", "--prompt-ref", "prompt-admission-e2e"},
			TimeoutMS: 5_000, MaxOutputBytes: 65_536,
		},
	}
}

func assertRunnerDispatchAdmissionE2EReady(
	t *testing.T, observation deviceplacement.RunnerDispatchAdmissionObservation,
	request deviceplacement.RunnerDispatchAdmissionRequest, entry executionlease.RegistryEntry, owner deviceidentity.Owner,
) {
	t.Helper()
	if err := observation.Validate(); err != nil || !observation.AdmissionReady || !observation.LeaseProofCurrent || !observation.LeaseActive ||
		!observation.CommandBindingValid || !observation.AttemptStateAdmissible || observation.ConversationID != request.ConversationID ||
		observation.RunID != request.RunID || observation.AttemptID != request.AttemptID || observation.TargetID != entry.Grant.TargetID ||
		observation.LeaseEpoch != entry.Grant.Epoch || observation.Owner != ownerToPlacement(owner) ||
		observation.Authority != (deviceplacement.RunnerDispatchAdmissionAuthority{}) || len(observation.RejectionReasons) != 0 {
		t.Fatalf("runner admission observation=%#v err=%v", observation, err)
	}
}

func runForgeRuntimeRunnerDispatchAdmissionRemoteCLI(
	t *testing.T, executable, apiURL, accessToken string, body []byte,
	request deviceplacement.RunnerDispatchAdmissionRequest, entry executionlease.RegistryEntry, owner deviceidentity.Owner,
) {
	t.Helper()
	inputPath := filepath.Join(t.TempDir(), "runner-dispatch-admission-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatalf("write runner admission request for Runtime CLI: %v", err)
	}
	output, stderr, err := runForgeRuntimeCLI(t, executable, apiURL, accessToken, t.TempDir(), "--json", "remote", "placement", "runner-dispatch-admission-preview", "--input", inputPath)
	if err != nil {
		t.Fatalf("authenticated Runner admission Runtime CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var value deviceplacement.RunnerDispatchAdmissionObservation
	if err := json.Unmarshal([]byte(output), &value); err != nil {
		t.Fatalf("decode Runner admission Runtime CLI: %v stdout=%q", err, output)
	}
	assertRunnerDispatchAdmissionE2EReady(t, value, request, entry, owner)
}

func runForgeRuntimeRunnerDispatchAdmissionRemoteTUI(
	t *testing.T, executable, apiURL, accessToken string, body []byte,
	request deviceplacement.RunnerDispatchAdmissionRequest, entry executionlease.RegistryEntry,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("Runner admission TUI E2E requires script: %v", err)
	}
	inputPath := filepath.Join(t.TempDir(), "runner-dispatch-admission-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatalf("write Runner admission request for TUI: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript, "-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("open " + request.ConversationID + "\nrunner-dispatch-admission-remote-preview --input " + inputPath + "\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated Runner admission TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Runner admission TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{"Runner dispatch admission", "target=" + entry.Grant.TargetID, "ready=true", "fencing token, argv, workspace, and output withheld"} {
		if !strings.Contains(output, want) {
			t.Fatalf("Runner admission TUI output omitted %q: %q", want, output)
		}
	}
	if strings.Contains(output, entry.Grant.FencingToken) || strings.Contains(output, "fencing_token") || strings.Contains(output, "prompt-admission-e2e") {
		t.Fatalf("Runner admission TUI leaked proof or command material: %q", output)
	}
	_ = request
}

func runForgeConsoleRunnerDispatchAdmissionE2EWithToken(
	t *testing.T, apiURL, token string, request deviceplacement.RunnerDispatchAdmissionRequest, owner deviceidentity.Owner,
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
		t.Fatalf("Flutter is required for Runner admission E2E: %v", err)
	}
	input := struct {
		APIURL      string                                         `json:"api_url"`
		AccessToken string                                         `json:"access_token"`
		Owner       model.Owner                                    `json:"owner"`
		Request     deviceplacement.RunnerDispatchAdmissionRequest `json:"request"`
	}{
		APIURL: apiURL, AccessToken: token,
		Owner: model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}, Request: request,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter Runner admission input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "runner-dispatch-admission-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write Flutter Runner admission input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable, "test", "--no-pub", "test/forge_runner_dispatch_admission_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(forgeConsoleTestEnvironment(inputPath, flutterHome), "FORGE_RUNNER_DISPATCH_ADMISSION_E2E_INPUT="+inputPath)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter Runner admission E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Runner admission Flutter output exceeded the size limit")
	}
}

func runForgeConsoleRunnerDispatchAdmissionGateE2EWithToken(
	t *testing.T, apiURL, token string, request deviceplacement.RunnerDispatchAdmissionRequest,
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
		t.Fatalf("Flutter is required for Runner admission Gate E2E: %v", err)
	}
	input := struct {
		APIURL      string                                         `json:"api_url"`
		AccessToken string                                         `json:"access_token"`
		Request     deviceplacement.RunnerDispatchAdmissionRequest `json:"request"`
	}{APIURL: apiURL, AccessToken: token, Request: request}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter Runner admission Gate input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "runner-dispatch-admission-gate-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter Runner admission Gate input: %v", err)
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
		"test/forge_runner_dispatch_admission_gate_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_RUNNER_DISPATCH_ADMISSION_GATE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter Runner admission Gate E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Runner admission Gate Flutter output exceeded the size limit")
	}
}
