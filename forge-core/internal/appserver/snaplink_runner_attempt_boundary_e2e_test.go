//go:build linux && !android

package appserver

import (
	"bytes"
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
	"forgeos/forge-core/internal/executionattempt"
	"forgeos/forge-core/internal/executionlease"
	"forgeos/forge-core/internal/runtimebridge"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// TestRunAcceptedExecuteRunnerAttemptBoundaryAcrossClients crosses one
// accepted EXECUTE + P4 Attempt edge through the authenticated Core route,
// Runtime CLI/TUI, and the shared Console Web/App/Mobile API/Gate. Every
// consumer receives the same redacted projection; the lease image is byte
// stable before and after the read-only previews.
func TestRunAcceptedExecuteRunnerAttemptBoundaryAcrossClients(t *testing.T) {
	configuredRuntime := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredRuntime == "" {
		t.Skip("Runner Attempt boundary E2E requires a configured Forge Runtime binary for durable Run binding")
	}
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)

	root := privateTestParent(t)
	stateDir := filepath.Join(root, "app-state-runner-attempt-boundary-e2e")
	runtimeStateDir := filepath.Join(root, "runtime-state-runner-attempt-boundary-e2e")
	if err := os.Mkdir(runtimeStateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	runtimeExecutable := filepath.Join(root, "forge-runtime-runner-attempt-boundary-e2e")
	if configuredRuntime != "" {
		runtimeExecutable = configuredRuntime
		initializeRuntimeHubForIntegration(t, runtimeExecutable, runtimeStateDir)
	} else if err := os.WriteFile(runtimeExecutable, []byte("test runtime"), 0o700); err != nil {
		t.Fatal(err)
	}

	owner := deviceidentity.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	conversationID := "conversation-runner-attempt-boundary-e2e"
	runID := "run-runner-attempt-boundary-e2e"
	if configuredRuntime != "" {
		projectPath, projectID, _ := seedRuntimeConversationScopes(t, runtimeExecutable, runtimeStateDir)
		if err := os.WriteFile(filepath.Join(projectPath, "README.md"), []byte("deterministic Runner Attempt boundary fixture\n"), 0o600); err != nil {
			t.Fatal(err)
		}
		bridgeStateDir := filepath.Join(root, "bridge-state-runner-attempt-boundary-e2e")
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
			model.ConversationScope{Kind: "project", ID: projectID}, "Runner Attempt boundary E2E", "runner-attempt-boundary-e2e-create")
		if err != nil {
			t.Fatalf("create Runtime Conversation for Attempt boundary E2E: %v", err)
		}
		conversationID = conversation.ID
		prompt, _, replayed, err := bridge.AppendOwnedPrompt(
			context.Background(), model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, "review this Runner Attempt boundary", "runner-attempt-boundary-e2e-prompt", 1,
		)
		if err != nil || replayed || prompt.ID == "" {
			t.Fatalf("append Runtime Prompt for Attempt boundary E2E: prompt=%#v replayed=%v err=%v", prompt, replayed, err)
		}
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		command := exec.CommandContext(ctx, runtimeExecutable,
			"--state-dir", runtimeStateDir, "--json", "--idempotency-key", "runner-attempt-boundary-e2e-start",
			"-C", projectPath, "run", "start", conversationID, prompt.ID, "--read", "README.md")
		command.Env = []string{}
		output, err := command.CombinedOutput()
		cancel()
		if err != nil || len(output) == 0 {
			t.Fatalf("seed deterministic Attempt boundary Run: %v: %s", err, output)
		}
		page, err := bridge.OwnedConversationRuns(
			context.Background(), model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, nil, 25,
		)
		if err != nil || len(page.Runs) != 1 || page.Runs[0].PromptID != prompt.ID {
			t.Fatalf("read seeded Attempt boundary Run page=%#v err=%v", page, err)
		}
		runID = page.Runs[0].RunID
	}

	state := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})
	leasePath := filepath.Join(filepath.Dir(registryPath), "leases.json")
	leaseAdapter, err := deviceinventory.NewPersistedExecutionLeaseRegistryFileSetWriteAdapterWithTokenSource(
		leasePath, owner, func() (string, error) { return "token-runner-attempt-boundary-a", nil },
	)
	if err != nil {
		t.Fatal(err)
	}
	entry, replayed, err := leaseAdapter.Claim(context.Background(), executionlease.ClaimRequest{
		ConversationID: conversationID, RunID: runID, AttemptID: "attempt-runner-attempt-boundary-e2e",
		IdempotencyKey: "runner-attempt-boundary-claim-key-00001", RequestSHA256: executionlease.RequestDigest([]byte("runner-attempt-boundary-claim")),
		IssuedAtMS: uint64(time.Now().UnixMilli()) - 1_000, TTLMS: 30_000,
	}, []executionlease.ClaimCandidate{{DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1, HeartbeatSequence: 1}})
	if err != nil || replayed {
		t.Fatalf("Attempt boundary lease entry=%#v replayed=%v err=%v", entry, replayed, err)
	}
	leaseBefore, err := os.ReadFile(leasePath)
	if err != nil {
		t.Fatal(err)
	}

	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{Status: "accepted", AcceptanceID: "p4-runner-attempt-boundary-e2e-001", AcceptedAtUnixMS: 1}
	activation.Evidence.RunnerIsolation = true
	activation.Evidence.LeaseFencing = true
	activation.Evidence.CancellationAndUncertainWork = true
	activation.Evidence.VaultArtifactAuthorization = true
	activation.Evidence.AuditOutbox = true
	authority := devicefabricgate.RunnerAuthorityConfig{
		Enabled: true, AuthorityID: "runner-authority-attempt-boundary-e2e",
		Decision: devicefabricgate.Decision{Status: "accepted", AcceptanceID: "runner-authority-attempt-boundary-e2e-001", AcceptedAtUnixMS: 1},
	}
	config := Config{
		ListenAddress: "127.0.0.1:0", StateDir: stateDir, Build: BuildInfo{Version: "test"},
		RuntimeExecutable: runtimeExecutable, RuntimeStateDir: runtimeStateDir,
		SnaplinkIssuer: issuer, SnaplinkAudience: snaplinkForgeTestAudience,
		SnaplinkJWKSURL: issuer + "/.well-known/jwks.json", ExpectedTenantID: snaplinkForgeTestTenant,
		ExpectedSubjectID: snaplinkForgeTestUser, JWKSHTTPClient: ssoClient,
		JWKSRefreshInterval: 24 * time.Hour, DeviceFabricActivation: ptrDeviceFabricRequest(activation),
		RunnerExecutionAuthority: &authority, DeviceInventoryLifecycleRegistryFile: registryPath,
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
		t.Fatalf("Attempt boundary E2E server failed before readiness: %v", err)
	case <-time.After(5 * time.Second):
		t.Fatal("Attempt boundary E2E server did not announce readiness")
	}
	t.Cleanup(func() {
		cancel()
		if err := waitResult(t, result); err != nil {
			t.Error(err)
		}
	})

	transportRequest := runnerTransportAdmissionE2ERequest(t, owner, entry)
	request := deviceplacement.RunnerAttemptBoundaryPreviewRequest{
		RunnerExecutionBoundaryPreviewRequest: deviceplacement.RunnerExecutionBoundaryPreviewRequest{
			Owner: ownerToPlacement(owner), ConversationID: entry.ConversationID, RunID: entry.RunID,
			AttemptID: entry.AttemptID, AttemptState: "accepted", Command: transportRequest.Command,
			Transport: transportRequest.Transport, ExpectedPayloadSHA256: transportRequest.ExpectedPayloadSHA256,
			Controls: deviceplacement.RunnerExecutionBoundaryControls{EffectState: "not_started"},
		},
		Transition: executionattempt.BeginStarting,
	}
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	client := &http.Client{Timeout: 5 * time.Second}
	post := func(payload []byte) (int, []byte) {
		t.Helper()
		req, err := http.NewRequest(http.MethodPost, ready.Listen+runnerAttemptBoundaryPathIDsURL(request.ConversationID, request.RunID), strings.NewReader(string(payload)))
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
		t.Fatalf("Runner Attempt boundary status=%d body=%q", status, responseBody)
	}
	var observation deviceplacement.RunnerAttemptBoundaryObservation
	if err := json.Unmarshal(responseBody, &observation); err != nil {
		t.Fatalf("decode Runner Attempt boundary: %v body=%q", err, responseBody)
	}
	assertRunnerAttemptBoundaryE2EReady(t, observation, request, entry, owner)
	for _, forbidden := range []string{"fencing_token", "argv", "workspace", "payload_body", "Runner output"} {
		if strings.Contains(string(responseBody), forbidden) {
			t.Fatalf("Runner Attempt boundary leaked %q: %q", forbidden, responseBody)
		}
	}

	if configuredRuntime != "" {
		runForgeRuntimeRunnerAttemptBoundaryRemoteCLI(t, runtimeExecutable, ready.Listen, token, body, request, entry, owner)
		runForgeRuntimeRunnerAttemptBoundaryRemoteTUI(t, runtimeExecutable, ready.Listen, token, body, request, entry)
	}
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		runForgeConsoleRunnerAttemptBoundaryE2EWithToken(t, ready.Listen, token, request)
		if configuredRuntime != "" {
			runForgeConsoleRunnerAttemptBoundaryGateE2EWithToken(t, ready.Listen, token, request)
		}
	}
	leaseAfter, err := os.ReadFile(leasePath)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(leaseBefore, leaseAfter) {
		t.Fatal("Runner Attempt boundary preview changed the durable lease image")
	}
}

func runnerAttemptBoundaryPathIDsURL(conversationID, runID string) string {
	return "/api/v1/conversations/" + conversationID + "/runs/" + runID + "/runner-attempt-boundary/preview"
}

func runnerAttemptBoundaryE2ERequest(t *testing.T, owner deviceidentity.Owner, entry executionlease.RegistryEntry) deviceplacement.RunnerAttemptBoundaryPreviewRequest {
	t.Helper()
	transport := runnerTransportAdmissionE2ERequest(t, owner, entry)
	return deviceplacement.RunnerAttemptBoundaryPreviewRequest{
		RunnerExecutionBoundaryPreviewRequest: deviceplacement.RunnerExecutionBoundaryPreviewRequest{
			Owner: ownerToPlacement(owner), ConversationID: entry.ConversationID, RunID: entry.RunID,
			AttemptID: entry.AttemptID, AttemptState: "accepted", Command: transport.Command,
			Transport: transport.Transport, ExpectedPayloadSHA256: transport.ExpectedPayloadSHA256,
			Controls: deviceplacement.RunnerExecutionBoundaryControls{EffectState: "not_started"},
		},
		Transition: executionattempt.BeginStarting,
	}
}

func assertRunnerAttemptBoundaryE2EReady(t *testing.T, observation deviceplacement.RunnerAttemptBoundaryObservation, request deviceplacement.RunnerAttemptBoundaryPreviewRequest, entry executionlease.RegistryEntry, owner deviceidentity.Owner) {
	t.Helper()
	if err := observation.Validate(); err != nil || !observation.ExecutionBoundaryReady || !observation.AttemptTransitionValid || !observation.AttemptTransitionDispatch || !observation.AttemptBoundaryReady ||
		observation.Owner != ownerToPlacement(owner) || observation.ConversationID != request.ConversationID || observation.RunID != request.RunID || observation.AttemptID != request.AttemptID ||
		observation.CommandID != request.Command.CommandID || observation.TargetID != entry.Grant.TargetID || observation.LeaseEpoch != entry.Grant.Epoch ||
		observation.CurrentAttemptState != request.AttemptState || observation.NextAttemptState != "starting" || observation.Transition != executionattempt.BeginStarting ||
		observation.Authority != (deviceplacement.RunnerAttemptBoundaryAuthority{}) || len(observation.RejectionReasons) != 0 {
		t.Fatalf("Runner Attempt boundary observation=%#v err=%v", observation, err)
	}
}

func runForgeRuntimeRunnerAttemptBoundaryRemoteCLI(t *testing.T, executable, apiURL, accessToken string, body []byte, request deviceplacement.RunnerAttemptBoundaryPreviewRequest, entry executionlease.RegistryEntry, owner deviceidentity.Owner) {
	t.Helper()
	inputPath := filepath.Join(t.TempDir(), "runner-attempt-boundary-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatalf("write Runner Attempt boundary request for Runtime CLI: %v", err)
	}
	output, stderr, err := runForgeRuntimeCLI(t, executable, apiURL, accessToken, t.TempDir(), "--json", "remote", "placement", "runner-attempt-boundary-preview", "--input", inputPath)
	if err != nil {
		t.Fatalf("authenticated Runner Attempt boundary Runtime CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var value deviceplacement.RunnerAttemptBoundaryObservation
	if err := json.Unmarshal([]byte(output), &value); err != nil {
		t.Fatalf("decode Runner Attempt boundary Runtime CLI: %v stdout=%q", err, output)
	}
	assertRunnerAttemptBoundaryE2EReady(t, value, request, entry, owner)
}

func runForgeRuntimeRunnerAttemptBoundaryRemoteTUI(t *testing.T, executable, apiURL, accessToken string, body []byte, request deviceplacement.RunnerAttemptBoundaryPreviewRequest, entry executionlease.RegistryEntry) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("Runner Attempt boundary TUI E2E requires script: %v", err)
	}
	inputPath := filepath.Join(t.TempDir(), "runner-attempt-boundary-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatalf("write Runner Attempt boundary request for TUI: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript, "-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("open " + request.ConversationID + "\nrunner-attempt-boundary-remote-preview --input " + inputPath + "\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated Runner Attempt boundary TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Runner Attempt boundary TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{"Runner Attempt boundary", "target=" + entry.Grant.TargetID, "attempt_boundary_ready=true", "fencing token, argv, workspace, transport payload, and Runner output withheld"} {
		if !strings.Contains(output, want) {
			t.Fatalf("Runner Attempt boundary TUI output omitted %q: %q", want, output)
		}
	}
	if strings.Contains(output, entry.Grant.FencingToken) || strings.Contains(output, "fencing_token") || strings.Contains(output, "forge-task") {
		t.Fatalf("Runner Attempt boundary TUI leaked proof or command material: %q", output)
	}
}

func runForgeConsoleRunnerAttemptBoundaryE2EWithToken(t *testing.T, apiURL, token string, request deviceplacement.RunnerAttemptBoundaryPreviewRequest) {
	t.Helper()
	consoleRoot := snaplinkConsoleRootForAttemptBoundaryE2E(t)
	flutterExecutable := flutterForAttemptBoundaryE2E(t, "Runner Attempt boundary E2E")
	input := struct {
		APIURL  string                                              `json:"api_url"`
		Token   string                                              `json:"access_token"`
		Request deviceplacement.RunnerAttemptBoundaryPreviewRequest `json:"request"`
	}{apiURL, token, request}
	runFlutterAttemptBoundaryE2E(t, consoleRoot, flutterExecutable, input, "test/forge_runner_attempt_boundary_api_e2e_test.dart", "FORGE_RUNNER_ATTEMPT_BOUNDARY_E2E_INPUT", apiURL)
}

func runForgeConsoleRunnerAttemptBoundaryGateE2EWithToken(t *testing.T, apiURL, token string, request deviceplacement.RunnerAttemptBoundaryPreviewRequest) {
	t.Helper()
	consoleRoot := snaplinkConsoleRootForAttemptBoundaryE2E(t)
	flutterExecutable := flutterForAttemptBoundaryE2E(t, "Runner Attempt boundary Gate E2E")
	input := struct {
		APIURL  string                                              `json:"api_url"`
		Token   string                                              `json:"access_token"`
		Request deviceplacement.RunnerAttemptBoundaryPreviewRequest `json:"request"`
	}{apiURL, token, request}
	runFlutterAttemptBoundaryE2E(t, consoleRoot, flutterExecutable, input, "test/forge_runner_attempt_boundary_gate_e2e_test.dart", "FORGE_RUNNER_ATTEMPT_BOUNDARY_GATE_E2E_INPUT", apiURL)
}

func snaplinkConsoleRootForAttemptBoundaryE2E(t *testing.T) string {
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

func flutterForAttemptBoundaryE2E(t *testing.T, purpose string) string {
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

func runFlutterAttemptBoundaryE2E(t *testing.T, consoleRoot, flutterExecutable string, input any, testFile, inputEnv, apiURL string) {
	t.Helper()
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter Runner Attempt boundary input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "runner-attempt-boundary-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write Flutter Runner Attempt boundary input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable, "test", "--no-pub", testFile)
	command.Dir = consoleRoot
	command.Env = append(forgeConsoleTestEnvironment(inputPath, flutterHome), inputEnv+"="+inputPath)
	if strings.Contains(testFile, "_gate_") {
		command.Args = append(command.Args[:len(command.Args)-1], "--dart-define=FORGE_CONVERSATIONS_API_ORIGIN="+apiURL, testFile)
	}
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter Runner Attempt boundary E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Runner Attempt boundary Flutter output exceeded the size limit")
	}
}
