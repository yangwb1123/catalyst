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

// TestRunAcceptedExecuteRunnerExecutionBoundaryAcrossClients crosses the
// accepted EXECUTE + P4 boundary with one fenced lease and one verified
// transport observation. Core, and when configured Runtime/Console clients,
// consume the same display-only value. Releasing the lease makes the exact
// request non-ready without opening a Runner or authorizing execution.
func TestRunAcceptedExecuteRunnerExecutionBoundaryAcrossClients(t *testing.T) {
	configuredRuntime := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredRuntime == "" {
		t.Skip("Runner execution boundary E2E requires a configured Forge Runtime binary for durable Run binding")
	}
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)

	root := privateTestParent(t)
	stateDir := filepath.Join(root, "app-state-runner-execution-boundary-e2e")
	runtimeStateDir := filepath.Join(root, "runtime-state-runner-execution-boundary-e2e")
	if err := os.Mkdir(runtimeStateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	runtimeExecutable := filepath.Join(root, "forge-runtime-runner-execution-boundary-e2e")
	if configuredRuntime != "" {
		runtimeExecutable = configuredRuntime
		initializeRuntimeHubForIntegration(t, runtimeExecutable, runtimeStateDir)
	} else if err := os.WriteFile(runtimeExecutable, []byte("test runtime"), 0o700); err != nil {
		t.Fatal(err)
	}

	owner := deviceidentity.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	conversationID := "conversation-execution-boundary-e2e"
	runID := "run-execution-boundary-e2e"
	if configuredRuntime != "" {
		projectPath, projectID, _ := seedRuntimeConversationScopes(t, runtimeExecutable, runtimeStateDir)
		if err := os.WriteFile(filepath.Join(projectPath, "README.md"), []byte("deterministic Runner execution boundary fixture\n"), 0o600); err != nil {
			t.Fatal(err)
		}
		bridgeStateDir := filepath.Join(root, "bridge-state-runner-execution-boundary-e2e")
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
			model.ConversationScope{Kind: "project", ID: projectID}, "Runner execution boundary E2E", "runner-execution-boundary-e2e-create")
		if err != nil {
			t.Fatalf("create Runtime conversation for execution-boundary E2E: %v", err)
		}
		conversationID = conversation.ID
		prompt, _, replayed, err := bridge.AppendOwnedPrompt(
			context.Background(),
			model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, "review this Runner execution boundary", "runner-execution-boundary-e2e-prompt", 1,
		)
		if err != nil || replayed || prompt.ID == "" {
			t.Fatalf("append Runtime Prompt for execution-boundary E2E: prompt=%#v replayed=%v err=%v", prompt, replayed, err)
		}
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		command := exec.CommandContext(ctx, runtimeExecutable,
			"--state-dir", runtimeStateDir, "--json", "--idempotency-key", "runner-execution-boundary-e2e-start",
			"-C", projectPath, "run", "start", conversationID, prompt.ID, "--read", "README.md")
		command.Env = []string{}
		output, err := command.CombinedOutput()
		cancel()
		if err != nil || len(output) == 0 {
			t.Fatalf("seed deterministic execution-boundary Run: %v: %s", err, output)
		}
		page, err := bridge.OwnedConversationRuns(
			context.Background(),
			model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, nil, 25,
		)
		if err != nil || len(page.Runs) != 1 || page.Runs[0].PromptID != prompt.ID {
			t.Fatalf("read seeded execution-boundary Run page=%#v err=%v", page, err)
		}
		runID = page.Runs[0].RunID
	}
	clientInstancePath := filepath.Join(t.TempDir(), "client-instance-session-view.json")
	writeClientInstanceSessionViewSourceFileAtPath(t, clientInstancePath, owner, []deviceplacement.ClientInstanceSessionViewInstance{
		{InstanceID: "client-web-001", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{conversationID}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-app-001", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{conversationID}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-mobile-001", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{conversationID}, ObservedAtMS: 200_500, Status: "active"},
	})

	state := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})
	leasePath := filepath.Join(filepath.Dir(registryPath), "leases.json")
	leaseAdapter, err := deviceinventory.NewPersistedExecutionLeaseRegistryFileSetWriteAdapterWithTokenSource(
		leasePath, owner, func() (string, error) { return "token-execution-boundary-a", nil },
	)
	if err != nil {
		t.Fatal(err)
	}
	now := uint64(time.Now().UnixMilli())
	entry, replayed, err := leaseAdapter.Claim(context.Background(), executionlease.ClaimRequest{
		ConversationID: conversationID, RunID: runID, AttemptID: "attempt-transport-admission-e2e",
		IdempotencyKey: "execution-boundary-claim-key-00001", RequestSHA256: executionlease.RequestDigest([]byte("execution-boundary-claim")),
		IssuedAtMS: now - 1_000, TTLMS: 30_000,
	}, []executionlease.ClaimCandidate{{DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1, HeartbeatSequence: 1}})
	if err != nil || replayed {
		t.Fatalf("execution boundary lease entry=%#v replayed=%v err=%v", entry, replayed, err)
	}

	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{Status: "accepted", AcceptanceID: "p4-runner-execution-boundary-e2e-001", AcceptedAtUnixMS: 1}
	activation.Evidence.RunnerIsolation = true
	activation.Evidence.LeaseFencing = true
	activation.Evidence.CancellationAndUncertainWork = true
	activation.Evidence.VaultArtifactAuthorization = true
	activation.Evidence.AuditOutbox = true
	authority := devicefabricgate.RunnerAuthorityConfig{
		Enabled: true, AuthorityID: "runner-authority-execution-boundary-e2e",
		Decision: devicefabricgate.Decision{Status: "accepted", AcceptanceID: "runner-authority-execution-boundary-e2e-001", AcceptedAtUnixMS: 1},
	}
	config := Config{
		ListenAddress: "127.0.0.1:0", StateDir: stateDir, Build: BuildInfo{Version: "test"},
		RuntimeExecutable: runtimeExecutable, RuntimeStateDir: runtimeStateDir,
		SnaplinkIssuer: issuer, SnaplinkAudience: snaplinkForgeTestAudience,
		SnaplinkJWKSURL: issuer + "/.well-known/jwks.json", ExpectedTenantID: snaplinkForgeTestTenant,
		ExpectedSubjectID: snaplinkForgeTestUser, JWKSHTTPClient: ssoClient,
		JWKSRefreshInterval: 24 * time.Hour, DeviceFabricActivation: ptrDeviceFabricRequest(activation),
		RunnerExecutionAuthority:             &authority,
		DeviceInventoryLifecycleRegistryFile: registryPath, DeviceClientInstanceSessionViewFile: clientInstancePath,
		DeviceExecutionLeaseRegistryFile: leasePath,
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
		t.Fatalf("execution boundary E2E server failed before readiness: %v", err)
	case <-time.After(5 * time.Second):
		t.Fatal("execution boundary E2E server did not announce readiness")
	}
	t.Cleanup(func() {
		cancel()
		if err := waitResult(t, result); err != nil {
			t.Error(err)
		}
	})

	transportRequest := runnerTransportAdmissionE2ERequest(t, owner, entry)
	request := deviceplacement.RunnerExecutionBoundaryPreviewRequest{
		Owner: ownerToPlacement(owner), ConversationID: entry.ConversationID, RunID: entry.RunID,
		AttemptID: entry.AttemptID, AttemptState: "accepted", Command: transportRequest.Command,
		Transport: transportRequest.Transport, ExpectedPayloadSHA256: transportRequest.ExpectedPayloadSHA256,
		Controls: deviceplacement.RunnerExecutionBoundaryControls{EffectState: "not_started"},
	}
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	client := &http.Client{Timeout: 5 * time.Second}
	post := func(payload []byte) (int, []byte) {
		t.Helper()
		req, err := http.NewRequest(http.MethodPost,
			ready.Listen+runnerExecutionBoundaryPathIDsURL(request.ConversationID, request.RunID),
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
		t.Fatalf("execution boundary status=%d body=%q", status, responseBody)
	}
	var observation deviceplacement.RunnerExecutionBoundaryObservation
	if err := json.Unmarshal(responseBody, &observation); err != nil {
		t.Fatalf("decode execution boundary: %v body=%q", err, responseBody)
	}
	assertRunnerExecutionBoundaryE2EReady(t, observation, request, entry, owner)
	for _, forbidden := range []string{"fencing_token", "argv", "workspace", "payload_body", "Runner output"} {
		if strings.Contains(string(responseBody), forbidden) {
			t.Fatalf("execution boundary leaked %q: %q", forbidden, responseBody)
		}
	}

	if configuredRuntime != "" {
		runForgeRuntimeRunnerExecutionBoundaryRemoteCLI(t, runtimeExecutable, ready.Listen, token, body, request, entry, owner)
		runForgeRuntimeRunnerExecutionBoundaryRemoteTUI(t, runtimeExecutable, ready.Listen, token, body, request, entry)
	}
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		runForgeConsoleRunnerExecutionBoundaryE2EWithToken(t, ready.Listen, token, request, owner)
		if configuredRuntime != "" {
			runForgeConsoleRunnerExecutionBoundaryGateE2EWithToken(t, ready.Listen, token, request)
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
	releaseRequest.Header.Set("Idempotency-Key", "execution-boundary-release-key-00001")
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
		t.Fatalf("execution boundary release status=%d body=%q", releaseResponse.StatusCode, releasePayload)
	}

	status, responseBody = post(body)
	if status != http.StatusOK {
		t.Fatalf("released execution boundary status=%d body=%q", status, responseBody)
	}
	if err := json.Unmarshal(responseBody, &observation); err != nil {
		t.Fatal(err)
	}
	if err := observation.Validate(); err != nil || observation.ExecutionBoundaryReady ||
		!containsAdmissionReason(observation.RejectionReasons, "lease_inactive_at_evaluated_time") ||
		observation.Authority != (deviceplacement.RunnerExecutionBoundaryAuthority{}) {
		t.Fatalf("released execution boundary=%#v err=%v", observation, err)
	}
}

func runnerExecutionBoundaryPathIDsURL(conversationID, runID string) string {
	return "/api/v1/conversations/" + conversationID + "/runs/" + runID + "/runner-execution-boundary/preview"
}

func assertRunnerExecutionBoundaryE2EReady(
	t *testing.T, observation deviceplacement.RunnerExecutionBoundaryObservation,
	request deviceplacement.RunnerExecutionBoundaryPreviewRequest, entry executionlease.RegistryEntry, owner deviceidentity.Owner,
) {
	t.Helper()
	if err := observation.Validate(); err != nil || !observation.ExecutionBoundaryReady || !observation.ActivationAllowed ||
		!observation.RunnerAuthorityAccepted || !observation.DispatchAdmissionReady || !observation.TransportAdmissionReady ||
		observation.ConversationID != request.ConversationID || observation.RunID != request.RunID ||
		observation.AttemptID != request.AttemptID || observation.TargetID != entry.Grant.TargetID ||
		observation.LeaseEpoch != entry.Grant.Epoch || observation.Owner != ownerToPlacement(owner) ||
		observation.Authority != (deviceplacement.RunnerExecutionBoundaryAuthority{}) || len(observation.RejectionReasons) != 0 {
		t.Fatalf("execution boundary observation=%#v err=%v", observation, err)
	}
}

func runForgeRuntimeRunnerExecutionBoundaryRemoteCLI(
	t *testing.T, executable, apiURL, accessToken string, body []byte,
	request deviceplacement.RunnerExecutionBoundaryPreviewRequest, entry executionlease.RegistryEntry, owner deviceidentity.Owner,
) {
	t.Helper()
	inputPath := filepath.Join(t.TempDir(), "runner-execution-boundary-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatalf("write execution boundary request for Runtime CLI: %v", err)
	}
	output, stderr, err := runForgeRuntimeCLI(t, executable, apiURL, accessToken, t.TempDir(), "--json", "remote", "placement", "runner-execution-boundary-preview", "--input", inputPath)
	if err != nil {
		t.Fatalf("authenticated execution boundary Runtime CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var value deviceplacement.RunnerExecutionBoundaryObservation
	if err := json.Unmarshal([]byte(output), &value); err != nil {
		t.Fatalf("decode execution boundary Runtime CLI: %v stdout=%q", err, output)
	}
	assertRunnerExecutionBoundaryE2EReady(t, value, request, entry, owner)
}

func runForgeRuntimeRunnerExecutionBoundaryRemoteTUI(
	t *testing.T, executable, apiURL, accessToken string, body []byte,
	request deviceplacement.RunnerExecutionBoundaryPreviewRequest, entry executionlease.RegistryEntry,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("execution boundary TUI E2E requires script: %v", err)
	}
	inputPath := filepath.Join(t.TempDir(), "runner-execution-boundary-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatalf("write execution boundary request for TUI: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript, "-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("open " + request.ConversationID + "\nrunner-execution-boundary-remote-preview --input " + inputPath + "\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated execution boundary TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("execution boundary TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{"Runner execution boundary", "target=" + entry.Grant.TargetID, "ready=true", "fencing token, argv, workspace, transport payload, and Runner output withheld"} {
		if !strings.Contains(output, want) {
			t.Fatalf("execution boundary TUI output omitted %q: %q", want, output)
		}
	}
	if strings.Contains(output, entry.Grant.FencingToken) || strings.Contains(output, "fencing_token") || strings.Contains(output, "prompt-transport-admission-e2e") {
		t.Fatalf("execution boundary TUI leaked proof or command material: %q", output)
	}
}

func runForgeConsoleRunnerExecutionBoundaryE2EWithToken(
	t *testing.T, apiURL, token string, request deviceplacement.RunnerExecutionBoundaryPreviewRequest, owner deviceidentity.Owner,
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
		t.Fatalf("Flutter is required for execution boundary E2E: %v", err)
	}
	input := struct {
		APIURL      string                                                `json:"api_url"`
		AccessToken string                                                `json:"access_token"`
		Owner       model.Owner                                           `json:"owner"`
		Request     deviceplacement.RunnerExecutionBoundaryPreviewRequest `json:"request"`
	}{
		APIURL: apiURL, AccessToken: token,
		Owner: model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}, Request: request,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter execution boundary input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "runner-execution-boundary-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write Flutter execution boundary input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable, "test", "--no-pub", "test/forge_runner_execution_boundary_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(forgeConsoleTestEnvironment(inputPath, flutterHome), "FORGE_RUNNER_EXECUTION_BOUNDARY_E2E_INPUT="+inputPath)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter execution boundary E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("execution boundary Flutter output exceeded the size limit")
	}
}

func runForgeConsoleRunnerExecutionBoundaryGateE2EWithToken(
	t *testing.T, apiURL, token string, request deviceplacement.RunnerExecutionBoundaryPreviewRequest,
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
		t.Fatalf("Flutter is required for Runner execution boundary Gate E2E: %v", err)
	}
	input := struct {
		APIURL      string                                                `json:"api_url"`
		AccessToken string                                                `json:"access_token"`
		Request     deviceplacement.RunnerExecutionBoundaryPreviewRequest `json:"request"`
	}{APIURL: apiURL, AccessToken: token, Request: request}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter Runner execution boundary Gate input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "runner-execution-boundary-gate-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter Runner execution boundary Gate input: %v", err)
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
		"test/forge_runner_execution_boundary_gate_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_RUNNER_EXECUTION_BOUNDARY_GATE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter Runner execution boundary Gate E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Runner execution boundary Gate Flutter output exceeded the size limit")
	}
}
