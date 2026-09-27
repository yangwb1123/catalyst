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
	"forgeos/forge-core/internal/runnertransport"
	"forgeos/forge-core/internal/runtimebridge"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// TestRunAcceptedExecuteRunnerTransportAdmissionAcrossClients crosses the
// accepted EXECUTE + P4 boundary with one already fenced lease and one D3
// transport observation. Core, Runtime CLI, and Runtime TUI consume the same
// display-only value; releasing the proof turns the same request inactive.
func TestRunAcceptedExecuteRunnerTransportAdmissionAcrossClients(t *testing.T) {
	if os.Getenv("FORGE_RUNTIME_BIN") == "" {
		t.Skip("Runner transport admission E2E requires a configured Forge Runtime binary for durable Run binding")
	}
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)

	root := privateTestParent(t)
	stateDir := filepath.Join(root, "app-state-runner-transport-admission-e2e")
	runtimeStateDir := filepath.Join(root, "runtime-state-runner-transport-admission-e2e")
	if err := os.Mkdir(runtimeStateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	runtimeExecutable := filepath.Join(root, "forge-runtime-runner-transport-admission-e2e")
	configuredRuntime := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredRuntime != "" {
		runtimeExecutable = configuredRuntime
		initializeRuntimeHubForIntegration(t, runtimeExecutable, runtimeStateDir)
	} else if err := os.WriteFile(runtimeExecutable, []byte("test runtime"), 0o700); err != nil {
		t.Fatal(err)
	}

	owner := deviceidentity.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	conversationID := "conversation-transport-admission-e2e"
	runID := "run-transport-admission-e2e"
	if configuredRuntime != "" {
		projectPath, projectID, _ := seedRuntimeConversationScopes(t, runtimeExecutable, runtimeStateDir)
		if err := os.WriteFile(filepath.Join(projectPath, "README.md"), []byte("deterministic Runner transport admission fixture\n"), 0o600); err != nil {
			t.Fatal(err)
		}
		bridgeStateDir := filepath.Join(root, "bridge-state-runner-transport-admission-e2e")
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
			model.ConversationScope{Kind: "project", ID: projectID}, "Runner transport admission E2E", "runner-transport-admission-e2e-create")
		if err != nil {
			t.Fatalf("create Runtime conversation for transport admission E2E: %v", err)
		}
		conversationID = conversation.ID
		prompt, _, replayed, err := bridge.AppendOwnedPrompt(
			context.Background(),
			model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, "review this Runner transport admission", "runner-transport-admission-e2e-prompt", 1,
		)
		if err != nil || replayed || prompt.ID == "" {
			t.Fatalf("append Runtime Prompt for transport admission E2E: prompt=%#v replayed=%v err=%v", prompt, replayed, err)
		}
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		command := exec.CommandContext(ctx, runtimeExecutable,
			"--state-dir", runtimeStateDir, "--json", "--idempotency-key", "runner-transport-admission-e2e-start",
			"-C", projectPath, "run", "start", conversationID, prompt.ID, "--read", "README.md")
		command.Env = []string{}
		output, err := command.CombinedOutput()
		cancel()
		if err != nil || len(output) == 0 {
			t.Fatalf("seed deterministic transport admission Run: %v: %s", err, output)
		}
		page, err := bridge.OwnedConversationRuns(
			context.Background(),
			model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, nil, 25,
		)
		if err != nil || len(page.Runs) != 1 || page.Runs[0].PromptID != prompt.ID {
			t.Fatalf("read seeded transport admission Run page=%#v err=%v", page, err)
		}
		runID = page.Runs[0].RunID
	}

	state := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})
	leasePath := filepath.Join(filepath.Dir(registryPath), "leases.json")
	leaseAdapter, err := deviceinventory.NewPersistedExecutionLeaseRegistryFileSetWriteAdapterWithTokenSource(
		leasePath, owner, func() (string, error) { return "token-transport-admission-a", nil },
	)
	if err != nil {
		t.Fatal(err)
	}
	now := uint64(time.Now().UnixMilli())
	entry, replayed, err := leaseAdapter.Claim(context.Background(), executionlease.ClaimRequest{
		ConversationID: conversationID, RunID: runID, AttemptID: "attempt-transport-admission-e2e",
		IdempotencyKey: "transport-admission-claim-key-00001", RequestSHA256: executionlease.RequestDigest([]byte("transport-admission-claim")),
		IssuedAtMS: now - 1_000, TTLMS: 30_000,
	}, []executionlease.ClaimCandidate{{DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1, HeartbeatSequence: 1}})
	if err != nil || replayed {
		t.Fatalf("transport admission lease entry=%#v replayed=%v err=%v", entry, replayed, err)
	}

	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{Status: "accepted", AcceptanceID: "p4-runner-transport-admission-e2e-001", AcceptedAtUnixMS: 1}
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
		DeviceInventoryLifecycleRegistryFile: registryPath, DeviceExecutionLeaseRegistryFile: leasePath,
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
		t.Fatalf("runner transport admission E2E server failed before readiness: %v", err)
	case <-time.After(5 * time.Second):
		t.Fatal("runner transport admission E2E server did not announce readiness")
	}
	t.Cleanup(func() {
		cancel()
		if err := waitResult(t, result); err != nil {
			t.Error(err)
		}
	})

	request := runnerTransportAdmissionE2ERequest(t, owner, entry)
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	client := &http.Client{Timeout: 5 * time.Second}
	post := func(payload []byte) (int, []byte) {
		t.Helper()
		req, err := http.NewRequest(http.MethodPost,
			ready.Listen+runnerTransportAdmissionPathIDsURL(request.ConversationID, request.RunID),
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
		t.Fatalf("runner transport admission status=%d body=%q", status, responseBody)
	}
	var observation deviceplacement.RunnerTransportAdmissionObservation
	if err := json.Unmarshal(responseBody, &observation); err != nil {
		t.Fatalf("decode runner transport admission: %v body=%q", err, responseBody)
	}
	assertRunnerTransportAdmissionE2EReady(t, observation, request, entry, owner)
	for _, forbidden := range []string{"fencing_token", "argv", "workspace", "payload_body"} {
		if strings.Contains(string(responseBody), forbidden) {
			t.Fatalf("runner transport admission leaked %q: %q", forbidden, responseBody)
		}
	}

	if configuredRuntime != "" {
		runForgeRuntimeRunnerTransportAdmissionRemoteCLI(t, runtimeExecutable, ready.Listen, token, body, request, entry, owner)
		runForgeRuntimeRunnerTransportAdmissionRemoteTUI(t, runtimeExecutable, ready.Listen, token, body, request, entry)
	}
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		runForgeConsoleRunnerTransportAdmissionE2EWithToken(t, ready.Listen, token, request, owner)
		if configuredRuntime != "" {
			runForgeConsoleRunnerTransportAdmissionGateE2EWithToken(t, ready.Listen, token, request)
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
	releaseRequest.Header.Set("Idempotency-Key", "transport-admission-release-key-00001")
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
		t.Fatalf("runner transport admission release status=%d body=%q", releaseResponse.StatusCode, releasePayload)
	}

	status, responseBody = post(body)
	if status != http.StatusOK {
		t.Fatalf("released runner transport admission status=%d body=%q", status, responseBody)
	}
	if err := json.Unmarshal(responseBody, &observation); err != nil {
		t.Fatal(err)
	}
	if err := observation.Validate(); err != nil || observation.AdmissionReady || observation.LeaseActive ||
		!containsAdmissionReason(observation.RejectionReasons, "lease_inactive_at_evaluated_time") ||
		observation.Authority != (deviceplacement.RunnerTransportAdmissionAuthority{}) {
		t.Fatalf("released runner transport admission=%#v err=%v", observation, err)
	}
}

func runnerTransportAdmissionPathIDsURL(conversationID, runID string) string {
	return "/api/v1/conversations/" + conversationID + "/runs/" + runID + "/runner-transport-admission/preview"
}

func runnerTransportAdmissionE2ERequest(t *testing.T, owner deviceidentity.Owner, entry executionlease.RegistryEntry) deviceplacement.RunnerTransportAdmissionRequest {
	t.Helper()
	path := deviceplacement.TransportPayloadBindingPath(entry.Grant.TargetID)
	payload := []byte(`{"attempt_id":"attempt-transport-admission-e2e","command_id":"command-transport-admission-e2e-1","target_id":"runner-a"}`)
	timestamp := time.Now().Unix()
	// secret-scan:ignore — deterministic HMAC fixture, never used outside this test.
	const secret = "runner-transport-admission-e2e-secret"
	signature, err := runnertransport.Sign(secret, http.MethodPost, path, timestamp, "transport-admission-e2e-nonce", payload)
	if err != nil {
		t.Fatal(err)
	}
	transport, err := runnertransport.Verify(secret, http.MethodPost, path, runnertransport.Envelope{
		TS: timestamp, Nonce: "transport-admission-e2e-nonce", Sig: signature, Payload: json.RawMessage(payload),
	}, timestamp, runnertransport.NewReplayCache(4))
	if err != nil {
		t.Fatal(err)
	}
	command := deviceplacement.RunnerExecutionCommand{
		V: 1, CommandID: "command-transport-admission-e2e-1",
		LeaseProof: deviceplacement.RunnerExecutionLeaseProof{
			AttemptID: entry.AttemptID, TargetID: entry.Grant.TargetID, Epoch: entry.Grant.Epoch, FencingToken: entry.Grant.FencingToken,
		},
		IdempotencyKey: entry.RunID + ":" + entry.AttemptID + ":command-transport-admission-e2e-1",
		WorkspaceRef:   "workspace-transport-admission-e2e", Argv: []string{"forge-task", "--prompt-ref", "prompt-transport-admission-e2e"},
		TimeoutMS: 5_000, MaxOutputBytes: 65_536,
	}
	return deviceplacement.RunnerTransportAdmissionRequest{
		Owner: ownerToPlacement(owner), ConversationID: entry.ConversationID, RunID: entry.RunID, AttemptID: entry.AttemptID,
		AttemptState: "accepted", Command: command,
		Lease:     deviceplacement.RunnerDispatchAdmissionLease{TargetID: entry.InstanceID, Epoch: entry.Grant.Epoch, IssuedAtMS: entry.Grant.IssuedAtMS, ExpiresAtMS: entry.Grant.ExpiresAtMS, Current: true, Active: true},
		Transport: transport, ExpectedPayloadSHA256: transport.PayloadSHA256, EvaluatedAtMS: 1,
	}
}

func assertRunnerTransportAdmissionE2EReady(t *testing.T, observation deviceplacement.RunnerTransportAdmissionObservation, request deviceplacement.RunnerTransportAdmissionRequest, entry executionlease.RegistryEntry, owner deviceidentity.Owner) {
	t.Helper()
	if err := observation.Validate(); err != nil || !observation.AdmissionReady || !observation.TransportBindingValid || !observation.LeaseProofCurrent || !observation.LeaseActive ||
		!observation.CommandBindingValid || !observation.AttemptStateAdmissible || observation.ConversationID != request.ConversationID || observation.RunID != request.RunID || observation.AttemptID != request.AttemptID ||
		observation.TargetID != entry.Grant.TargetID || observation.LeaseEpoch != entry.Grant.Epoch || observation.Owner != ownerToPlacement(owner) || observation.TransportPath != deviceplacement.TransportPayloadBindingPath(entry.Grant.TargetID) ||
		observation.Authority != (deviceplacement.RunnerTransportAdmissionAuthority{}) || len(observation.RejectionReasons) != 0 {
		t.Fatalf("runner transport admission observation=%#v err=%v", observation, err)
	}
}

func runForgeRuntimeRunnerTransportAdmissionRemoteCLI(t *testing.T, executable, apiURL, accessToken string, body []byte, request deviceplacement.RunnerTransportAdmissionRequest, entry executionlease.RegistryEntry, owner deviceidentity.Owner) {
	t.Helper()
	inputPath := filepath.Join(t.TempDir(), "runner-transport-admission-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatalf("write Runner transport admission request for Runtime CLI: %v", err)
	}
	output, stderr, err := runForgeRuntimeCLI(t, executable, apiURL, accessToken, t.TempDir(), "--json", "remote", "placement", "runner-transport-admission-preview", "--input", inputPath)
	if err != nil {
		t.Fatalf("authenticated Runner transport admission Runtime CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var value deviceplacement.RunnerTransportAdmissionObservation
	if err := json.Unmarshal([]byte(output), &value); err != nil {
		t.Fatalf("decode Runner transport admission Runtime CLI: %v stdout=%q", err, output)
	}
	assertRunnerTransportAdmissionE2EReady(t, value, request, entry, owner)
}

func runForgeRuntimeRunnerTransportAdmissionRemoteTUI(t *testing.T, executable, apiURL, accessToken string, body []byte, request deviceplacement.RunnerTransportAdmissionRequest, entry executionlease.RegistryEntry) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("Runner transport admission TUI E2E requires script: %v", err)
	}
	inputPath := filepath.Join(t.TempDir(), "runner-transport-admission-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatalf("write Runner transport admission request for TUI: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript, "-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("open " + request.ConversationID + "\nrunner-transport-admission-remote-preview --input " + inputPath + "\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated Runner transport admission TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Runner transport admission TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{"Runner transport admission", "target=" + entry.Grant.TargetID, "ready=true", "transport payload, fencing token, argv, workspace, and output withheld"} {
		if !strings.Contains(output, want) {
			t.Fatalf("Runner transport admission TUI output omitted %q: %q", want, output)
		}
	}
	if strings.Contains(output, entry.Grant.FencingToken) || strings.Contains(output, "fencing_token") || strings.Contains(output, "prompt-transport-admission-e2e") {
		t.Fatalf("Runner transport admission TUI leaked proof or command material: %q", output)
	}
}

func runForgeConsoleRunnerTransportAdmissionE2EWithToken(t *testing.T, apiURL, token string, request deviceplacement.RunnerTransportAdmissionRequest, owner deviceidentity.Owner) {
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
		t.Fatalf("Flutter is required for Runner transport admission E2E: %v", err)
	}
	input := struct {
		APIURL      string                                          `json:"api_url"`
		AccessToken string                                          `json:"access_token"`
		Owner       model.Owner                                     `json:"owner"`
		Request     deviceplacement.RunnerTransportAdmissionRequest `json:"request"`
	}{
		APIURL: apiURL, AccessToken: token,
		Owner: model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}, Request: request,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter Runner transport admission input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "runner-transport-admission-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write Flutter Runner transport admission input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable, "test", "--no-pub", "test/forge_runner_transport_admission_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(forgeConsoleTestEnvironment(inputPath, flutterHome), "FORGE_RUNNER_TRANSPORT_ADMISSION_E2E_INPUT="+inputPath)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter Runner transport admission E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Runner transport admission Flutter output exceeded the size limit")
	}
}

func runForgeConsoleRunnerTransportAdmissionGateE2EWithToken(
	t *testing.T, apiURL, token string, request deviceplacement.RunnerTransportAdmissionRequest,
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
		t.Fatalf("Flutter is required for Runner transport admission Gate E2E: %v", err)
	}
	input := struct {
		APIURL      string                                          `json:"api_url"`
		AccessToken string                                          `json:"access_token"`
		Request     deviceplacement.RunnerTransportAdmissionRequest `json:"request"`
	}{APIURL: apiURL, AccessToken: token, Request: request}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter Runner transport admission Gate input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "runner-transport-admission-gate-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter Runner transport admission Gate input: %v", err)
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
		"test/forge_runner_transport_admission_gate_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_RUNNER_TRANSPORT_ADMISSION_GATE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter Runner transport admission Gate E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Runner transport admission Gate Flutter output exceeded the size limit")
	}
}
