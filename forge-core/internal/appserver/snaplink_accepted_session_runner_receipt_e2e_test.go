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
	"strconv"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/executionprofile"
	"forgeos/forge-core/internal/runtimebridge"
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// TestRunAcceptedExecuteSessionRunnerReceiptAcrossClients crosses the
// accepted EXECUTE + P4 assembly with one content-free terminal receipt
// observation. Core, Runtime CLI/TUI, and the opt-in Console API must render
// the same owner/Conversation/Prompt/Run-bound receipt without adopting it as
// durable evidence or granting Runner authority. The opt-in Console branch
// also feeds a canonical receipt history through Core's reconciliation
// projection API and Sessions Gate against the same selected Run.
func TestRunAcceptedExecuteSessionRunnerReceiptAcrossClients(t *testing.T) {
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)

	root := privateTestParent(t)
	stateDir := filepath.Join(root, "app-state-session-runner-receipt-e2e")
	runtimeStateDir := filepath.Join(root, "runtime-state-session-runner-receipt-e2e")
	if err := os.Mkdir(runtimeStateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	runtimeExecutable := filepath.Join(root, "forge-runtime-session-runner-receipt-e2e")
	configuredRuntime := os.Getenv("FORGE_RUNTIME_BIN")
	var bridge *runtimebridge.Client
	var projectID string
	var executionProfiles []executionprofile.Binding
	var executionProfile intentmodel.ServerExecutionProfile
	if configuredRuntime != "" {
		runtimeExecutable = configuredRuntime
		initializeRuntimeHubForIntegration(t, runtimeExecutable, runtimeStateDir)
	} else if err := os.WriteFile(runtimeExecutable, []byte("test runtime"), 0o700); err != nil {
		t.Fatal(err)
	}

	owner := deviceidentity.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	conversationID := "conversation-session-runner-receipt-e2e"
	promptID := "prompt-session-runner-receipt-e2e"
	runID := "run-session-runner-receipt-e2e"
	if configuredRuntime != "" {
		projectPath, seededProjectID, _ := seedRuntimeConversationScopes(t, runtimeExecutable, runtimeStateDir)
		projectID = seededProjectID
		_, executionProfile = snaplinkExecutionProfile(t, projectID)
		executionProfiles = []executionprofile.Binding{{ProjectID: projectID, Profile: executionProfile}}
		if err := os.WriteFile(filepath.Join(projectPath, "README.md"), []byte("deterministic session Runner receipt fixture\n"), 0o600); err != nil {
			t.Fatal(err)
		}
		bridgeStateDir := filepath.Join(root, "bridge-state-session-runner-receipt-e2e")
		if err := os.Mkdir(bridgeStateDir, 0o700); err != nil {
			t.Fatal(err)
		}
		var err error
		bridge, err = runtimebridge.New(runtimebridge.Config{
			Executable: runtimeExecutable, AppServerStateDir: bridgeStateDir, RuntimeStateDir: runtimeStateDir,
			Timeout: 5 * time.Second,
		})
		if err != nil {
			t.Fatal(err)
		}
		conversation, err := bridge.CreateOwnedConversation(context.Background(),
			model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			model.ConversationScope{Kind: "project", ID: projectID}, "Session Runner receipt E2E", "session-runner-receipt-e2e-create")
		if err != nil {
			t.Fatalf("create Runtime conversation for receipt E2E: %v", err)
		}
		conversationID = conversation.ID
		prompt, _, replayed, err := bridge.AppendOwnedPrompt(
			context.Background(),
			model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, "review this completed Runner receipt", "session-runner-receipt-e2e-prompt", 1,
		)
		if err != nil || replayed || prompt.ID == "" {
			t.Fatalf("append Runtime Prompt for receipt E2E: prompt=%#v replayed=%v err=%v", prompt, replayed, err)
		}
		promptID = prompt.ID
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		command := exec.CommandContext(ctx, runtimeExecutable,
			"--state-dir", runtimeStateDir, "--json", "--idempotency-key", "session-runner-receipt-e2e-start",
			"-C", projectPath, "run", "start", conversationID, prompt.ID, "--read", "README.md")
		command.Env = []string{}
		output, err := command.CombinedOutput()
		cancel()
		if err != nil || len(output) == 0 {
			t.Fatalf("seed deterministic receipt Run: %v: %s", err, output)
		}
		page, err := bridge.OwnedConversationRuns(
			context.Background(),
			model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
			conversationID, nil, 25,
		)
		if err != nil || len(page.Runs) != 1 || page.Runs[0].PromptID != prompt.ID {
			t.Fatalf("read seeded receipt Run page=%#v err=%v", page, err)
		}
		runID = page.Runs[0].RunID
	}

	state := lifecycleRegistrySourceState(t, owner, "device-a", "runner-session-receipt-e2e", 1)
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})
	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{
		Status: "accepted", AcceptanceID: "p4-session-runner-receipt-e2e-001", AcceptedAtUnixMS: 1,
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
		ExecutionProfiles:                    executionProfiles,
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
		t.Fatalf("session Runner receipt E2E server failed before readiness: %v", err)
	case <-time.After(5 * time.Second):
		t.Fatal("session Runner receipt E2E server did not announce readiness")
	}
	t.Cleanup(func() {
		cancel()
		if err := waitResult(t, result); err != nil {
			t.Error(err)
		}
	})

	body := []byte(sessionRunnerReceiptObservationPreviewBody(
		t, owner.Issuer, owner.Subject, owner.TenantID, conversationID, promptID, runID,
	))
	client := &http.Client{Timeout: 5 * time.Second}
	postPath := func(path string, payload []byte) (int, []byte) {
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
		value, err := io.ReadAll(response.Body)
		if err != nil {
			t.Fatal(err)
		}
		return response.StatusCode, value
	}
	post := func(payload []byte) (int, []byte) {
		return postPath(sessionRunnerReceiptObservationPathURL(conversationID, runID), payload)
	}

	status, responseBody := post(body)
	if status != http.StatusOK {
		t.Fatalf("session Runner receipt status=%d body=%q", status, responseBody)
	}
	var observation deviceplacement.SessionRunnerReceiptObservation
	if err := json.Unmarshal(responseBody, &observation); err != nil {
		t.Fatalf("decode session Runner receipt: %v body=%q", err, responseBody)
	}
	assertAcceptedSessionRunnerReceiptE2E(t, observation, ownerToPlacement(owner), conversationID, promptID, runID)
	if strings.Contains(string(responseBody), "fencing_token") ||
		strings.Contains(string(responseBody), "forge-task") ||
		strings.Contains(string(responseBody), "private event payload") {
		t.Fatalf("session Runner receipt leaked proof or content: %q", responseBody)
	}

	if configuredRuntime != "" {
		runForgeRuntimeSessionRunnerReceiptRemoteCLI(t, runtimeExecutable, ready.Listen, token, body,
			ownerToPlacement(owner), conversationID, promptID, runID)
		runForgeRuntimeSessionRunnerReceiptRemoteTUI(t, runtimeExecutable, ready.Listen, token, body, conversationID)
	}
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		runForgeConsoleSessionRunnerReceiptE2EWithToken(t, ready.Listen, token, body,
			ownerToPlacement(owner), conversationID, promptID, runID)

		// Reuse the same accepted EXECUTE server and durable session that back
		// the receipt Gate. The history and reconciliation calls are still
		// caller-supplied previews: they never persist a receipt or authorize a
		// retry. The API check works with the deterministic fake Runtime too;
		// the Gate check below additionally requires a real seeded Runtime so
		// the Sessions screen can select the Conversation and Run over HTTP.
		historyBody := acceptedSessionRunnerReconciliationHistoryBodyForRun(
			t, ownerToPlacement(owner), conversationID, promptID, runID,
		)
		historyStatus, canonicalHistoryBody := postPath(
			sessionRunnerReceiptHistoryPathURL(conversationID, runID), historyBody,
		)
		if historyStatus != http.StatusOK {
			t.Fatalf("session Runner reconciliation history status=%d body=%q", historyStatus, canonicalHistoryBody)
		}
		var canonicalHistory deviceplacement.SessionRunnerReceiptHistoryObservation
		if err := json.Unmarshal(canonicalHistoryBody, &canonicalHistory); err != nil {
			t.Fatalf("decode session Runner reconciliation history: %v body=%q", err, canonicalHistoryBody)
		}
		if err := canonicalHistory.Validate(); err != nil {
			t.Fatalf("validate session Runner reconciliation history: %v", err)
		}
		reconciliationStatus, projectionBody := postPath(
			sessionRunnerReconciliationProjectionPathURL(conversationID, runID),
			canonicalHistoryBody,
		)
		if reconciliationStatus != http.StatusOK {
			t.Fatalf("session Runner reconciliation projection status=%d body=%q", reconciliationStatus, projectionBody)
		}
		var projection deviceplacement.SessionRunnerReconciliationProjection
		if err := json.Unmarshal(projectionBody, &projection); err != nil {
			t.Fatalf("decode session Runner reconciliation projection: %v body=%q", err, projectionBody)
		}
		assertAcceptedSessionRunnerReconciliationProjectionE2E(
			t, projection, ownerToPlacement(owner), conversationID, promptID, runID,
		)
		if configuredRuntime != "" {
			runForgeRuntimeSessionRunnerReconciliationRemoteCLI(
				t, runtimeExecutable, ready.Listen, token, canonicalHistoryBody,
				ownerToPlacement(owner), conversationID, promptID, runID,
			)
			runForgeRuntimeSessionRunnerReconciliationRemoteTUI(
				t, runtimeExecutable, ready.Listen, token, canonicalHistoryBody, conversationID,
			)
		}
		runForgeConsoleSessionRunnerReconciliationProjectionE2EWithToken(
			t, ready.Listen, token, canonicalHistoryBody, conversationID, promptID, runID,
		)
		if configuredRuntime != "" {
			runForgeConsoleSessionRunnerReceiptGateE2EWithToken(t, ready.Listen, token, body,
				conversationID, promptID, runID)
			runForgeConsoleSessionRunnerReconciliationProjectionGateE2EWithToken(
				t, ready.Listen, token, canonicalHistoryBody, conversationID, promptID, runID,
			)
			if projectID == "" || bridge == nil {
				t.Fatal("configured Runtime pending Run-intent Gate E2E lost the project bridge")
			}
			client := &http.Client{Timeout: 5 * time.Second}
			preview := snaplinkPreviewExecutionConsent(t, client, ready.Listen, token,
				conversationID, projectID, executionProfile)
			_ = snaplinkGrantExecutionConsent(t, client, ready.Listen, token, conversationID, preview)
			ownedOwner := model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
			page, err := bridge.ListOwnedConversations(context.Background(), ownedOwner, "", 25)
			if err != nil || len(page.Conversations) != 1 || page.Conversations[0].Conversation.ID != conversationID {
				t.Fatalf("read pending Run-intent Gate aggregate version page=%#v err=%v", page, err)
			}
			expectedVersion := int(page.Conversations[0].AggregateVersion)
			beforePending, err := bridge.OwnedConversationPendingRunIntents(
				context.Background(), ownedOwner, conversationID, nil, 25,
			)
			if err != nil || len(beforePending.Intents) != 0 {
				t.Fatalf("pending Run-intent Gate expected an empty initial page=%#v err=%v", beforePending, err)
			}
			pendingContent := "review this completed Runner receipt through the Sessions Gate"
			runForgeConsolePendingRunIntentGateE2EWithToken(t, ready.Listen, token, conversationID,
				owner.Issuer, owner.Subject, owner.TenantID, expectedVersion,
				pendingContent, "session-runner-receipt-gate-pending-intent")
			afterPending, err := bridge.OwnedConversationPendingRunIntents(
				context.Background(), ownedOwner, conversationID, nil, 25,
			)
			if err != nil || len(afterPending.Intents) != 1 || afterPending.Intents[0].Status != "pending" ||
				afterPending.Intents[0].AggregateVersion != uint64(expectedVersion+1) {
				t.Fatalf("pending Run-intent Gate receipt=%#v err=%v", afterPending, err)
			}
			prompts, err := bridge.OwnedConversationPrompts(context.Background(), ownedOwner, conversationID, nil, 25)
			if err != nil {
				t.Fatalf("read pending Run-intent Gate Prompt history: %v", err)
			}
			foundPendingPrompt := false
			for _, prompt := range prompts.Prompts {
				if prompt.Content == pendingContent {
					foundPendingPrompt = true
					break
				}
			}
			if !foundPendingPrompt {
				t.Fatalf("pending Run-intent Gate Prompt was not stored: %#v", prompts)
			}
			runs, err := bridge.OwnedConversationRuns(context.Background(), ownedOwner, conversationID, nil, 25)
			if err != nil || len(runs.Runs) != 1 || runs.Runs[0].RunID != runID {
				t.Fatalf("pending Run-intent Gate changed durable Runs page=%#v err=%v", runs, err)
			}

			// Use a separate Runtime CLI process/home as a second client. Its
			// authenticated write must become visible to a fresh Sessions Gate
			// through the shared Conversation aggregate, without a local Console
			// POST and without creating another durable Run.
			externalContent := "review the completed receipt from the Runtime CLI client"
			externalKey := "session-runner-receipt-cli-external-pending"
			externalExpectedVersion := expectedVersion + 1
			cliOutput, cliStderr, err := runForgeRuntimeCLI(
				t, runtimeExecutable, ready.Listen, token, t.TempDir(), "--json",
				"--idempotency-key", externalKey, "remote", "run-intents", "submit",
				conversationID, "--expected-version", strconv.Itoa(externalExpectedVersion), externalContent,
			)
			if err != nil {
				t.Fatalf("Runtime CLI external pending Run-intent submit failed: stderr=%q stdout=%q err=%v", cliStderr, cliOutput, err)
			}
			var external intentmodel.PendingRunIntentSubmissionResult
			if err := json.Unmarshal([]byte(cliOutput), &external); err != nil ||
				external.Replayed || external.Prompt.Content != externalContent ||
				external.Intent.Status != "pending" ||
				external.Intent.AggregateVersion != uint64(externalExpectedVersion+1) ||
				external.InitialEvent.Type != "submitted" {
				t.Fatalf("Runtime CLI external pending Run-intent=%#v stdout=%q decode=%v", external, cliOutput, err)
			}
			externalPending, err := bridge.OwnedConversationPendingRunIntents(
				context.Background(), ownedOwner, conversationID, nil, 25,
			)
			if err != nil || len(externalPending.Intents) != 2 {
				t.Fatalf("read external Runtime CLI pending Run-intents page=%#v err=%v", externalPending, err)
			}
			var externalIntentPresent bool
			for _, intent := range externalPending.Intents {
				if intent.IntentID == external.Intent.IntentID && intent.Status == "pending" &&
					intent.AggregateVersion == external.Intent.AggregateVersion {
					externalIntentPresent = true
					break
				}
			}
			if !externalIntentPresent {
				t.Fatalf("Runtime CLI external pending Run-intent was not durable: %#v", externalPending)
			}
			externalPrompts, err := bridge.OwnedConversationPrompts(
				context.Background(), ownedOwner, conversationID, nil, 25,
			)
			if err != nil {
				t.Fatalf("read Runtime CLI external Prompt history: %v", err)
			}
			var externalPromptPresent bool
			for _, prompt := range externalPrompts.Prompts {
				if prompt.Content == externalContent {
					externalPromptPresent = true
					break
				}
			}
			if !externalPromptPresent {
				t.Fatalf("Runtime CLI external Prompt was not stored: %#v", externalPrompts)
			}
			externalRuns, err := bridge.OwnedConversationRuns(
				context.Background(), ownedOwner, conversationID, nil, 25,
			)
			if err != nil || len(externalRuns.Runs) != 1 || externalRuns.Runs[0].RunID != runID {
				t.Fatalf("Runtime CLI external pending Run-intent changed durable Runs page=%#v err=%v", externalRuns, err)
			}
			runForgeConsolePendingRunIntentGateReadE2EWithToken(
				t, ready.Listen, token, conversationID, owner.Issuer, owner.Subject,
				owner.TenantID, externalExpectedVersion, externalContent, external.Intent.IntentID,
			)
			finalPending, err := bridge.OwnedConversationPendingRunIntents(
				context.Background(), ownedOwner, conversationID, nil, 25,
			)
			if err != nil || len(finalPending.Intents) != 2 {
				t.Fatalf("read pending Run-intents after read-only Gate=%#v err=%v", finalPending, err)
			}
			finalRuns, err := bridge.OwnedConversationRuns(
				context.Background(), ownedOwner, conversationID, nil, 25,
			)
			if err != nil || len(finalRuns.Runs) != 1 || finalRuns.Runs[0].RunID != runID {
				t.Fatalf("read-only Gate changed durable Runs page=%#v err=%v", finalRuns, err)
			}

			// Keep a third Gate open while another Runtime CLI process writes. The
			// live Sessions Gate must consume the owner change feed and refresh its
			// Prompt/pending metadata without issuing a local scheduling-review POST.
			liveContent := "observe the live change-feed convergence from Runtime CLI"
			liveKey := "session-runner-receipt-cli-live-convergence"
			liveExpectedVersion := int(external.Intent.AggregateVersion)
			runForgeConsolePendingRunIntentGateLiveConvergenceE2EWithToken(
				t, ready.Listen, token, conversationID, owner.Issuer, owner.Subject,
				owner.TenantID, runtimeExecutable, liveExpectedVersion, liveContent, liveKey, 2,
			)
			livePending, err := bridge.OwnedConversationPendingRunIntents(
				context.Background(), ownedOwner, conversationID, nil, 25,
			)
			if err != nil || len(livePending.Intents) != 3 {
				t.Fatalf("read pending Run-intents after live Gate convergence=%#v err=%v", livePending, err)
			}
			livePrompts, err := bridge.OwnedConversationPrompts(
				context.Background(), ownedOwner, conversationID, nil, 25,
			)
			if err != nil {
				t.Fatalf("read live-convergence Prompt history: %v", err)
			}
			var livePromptPresent bool
			for _, prompt := range livePrompts.Prompts {
				if prompt.Content == liveContent {
					livePromptPresent = true
					break
				}
			}
			if !livePromptPresent {
				t.Fatalf("live-convergence Prompt was not stored: %#v", livePrompts)
			}
			liveRuns, err := bridge.OwnedConversationRuns(
				context.Background(), ownedOwner, conversationID, nil, 25,
			)
			if err != nil || len(liveRuns.Runs) != 1 || liveRuns.Runs[0].RunID != runID {
				t.Fatalf("live Gate convergence changed durable Runs page=%#v err=%v", liveRuns, err)
			}
		}
	}
}

func sessionRunnerReceiptObservationPathURL(conversationID, runID string) string {
	return "/api/v1/conversations/" + conversationID + "/runs/" + runID + "/runner-receipt-observation/preview"
}

func assertAcceptedSessionRunnerReceiptE2E(
	t *testing.T, observation deviceplacement.SessionRunnerReceiptObservation,
	owner deviceplacement.Owner, conversationID, promptID, runID string,
) {
	t.Helper()
	if err := observation.Validate(); err != nil || observation.Owner != owner ||
		observation.ConversationID != conversationID || observation.PromptID != promptID ||
		observation.RunID != runID || observation.SelectedTargetID != nil ||
		!observation.PromptRunBindingValid || !observation.ReceiptBindingValid ||
		observation.ReceiptObservation.DispositionKind != "completed" ||
		!observation.ReceiptObservation.ReceiptValid || observation.ReceiptObservation.Uncertain ||
		observation.ReceiptObservation.ReconciliationRequired ||
		observation.ReceiptObservation.ManualReviewRequired || observation.ReceiptObservation.AutomaticRetry ||
		observation.ReceiptObservation.FollowUp != "none" ||
		observation.Authority != (deviceplacement.SessionRunnerReceiptAuthority{}) ||
		observation.ReceiptObservation.Authority != (deviceplacement.RunnerTerminalReceiptAuthority{}) {
		t.Fatalf("accepted session Runner receipt=%#v err=%v", observation, err)
	}
}

func runForgeRuntimeSessionRunnerReceiptRemoteCLI(
	t *testing.T, executable, apiURL, accessToken string, body []byte,
	owner deviceplacement.Owner, conversationID, promptID, runID string,
) {
	t.Helper()
	inputPath := filepath.Join(t.TempDir(), "session-runner-receipt-observation.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatalf("write session Runner receipt request for Runtime CLI: %v", err)
	}
	output, stderr, err := runForgeRuntimeCLI(t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "remote", "session-runner-receipt", "preview", "--input", inputPath)
	if err != nil {
		t.Fatalf("authenticated session Runner receipt Runtime CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var value deviceplacement.SessionRunnerReceiptObservation
	if err := json.Unmarshal([]byte(output), &value); err != nil {
		t.Fatalf("decode session Runner receipt Runtime CLI: %v stdout=%q", err, output)
	}
	assertAcceptedSessionRunnerReceiptE2E(t, value, owner, conversationID, promptID, runID)
}

func runForgeRuntimeSessionRunnerReceiptRemoteTUI(
	t *testing.T, executable, apiURL, accessToken string, body []byte, conversationID string,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("session Runner receipt TUI E2E requires script: %v", err)
	}
	inputPath := filepath.Join(t.TempDir(), "session-runner-receipt-observation.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatalf("write session Runner receipt request for TUI: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript, "-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("open " + conversationID + "\nsession-runner-receipt-preview --input " + inputPath + "\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated session Runner receipt TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("session Runner receipt TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"authenticated session Runner terminal receipt observation [forge.session-runner-receipt-observation/v1]",
		"disposition=completed", "receipt_valid=true uncertain=false",
		"follow_up=none reconciliation_required=false manual_review_required=false automatic_retry=false",
		"selected_target=none", "receipt_persisted=false execution_authorized=false",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("session Runner receipt TUI output omitted %q: %q", want, output)
		}
	}
	if strings.Contains(output, "fence-001") || strings.Contains(output, "forge-task") || strings.Contains(output, "/api/v1/devices") {
		t.Fatalf("session Runner receipt TUI leaked proof or device request: %q", output)
	}
}

func runForgeConsoleSessionRunnerReceiptE2EWithToken(
	t *testing.T, apiURL, token string, body []byte, owner deviceplacement.Owner,
	conversationID, promptID, runID string,
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
		t.Fatalf("Flutter is required for session Runner receipt E2E: %v", err)
	}
	var observation deviceplacement.SessionRunnerReceiptObservation
	if err := json.Unmarshal(body, &observation); err != nil {
		t.Fatalf("decode Flutter session Runner receipt input: %v", err)
	}
	inputJSON, err := json.Marshal(struct {
		APIURL       string                                          `json:"api_url"`
		AccessToken  string                                          `json:"access_token"`
		Owner        model.Owner                                     `json:"owner"`
		Observation  deviceplacement.SessionRunnerReceiptObservation `json:"observation"`
		Conversation string                                          `json:"conversation_id"`
		Prompt       string                                          `json:"prompt_id"`
		Run          string                                          `json:"run_id"`
	}{
		APIURL: apiURL, AccessToken: token,
		Owner:       model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		Observation: observation, Conversation: conversationID, Prompt: promptID, Run: runID,
	})
	if err != nil {
		t.Fatalf("encode Flutter session Runner receipt input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "session-runner-receipt-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write Flutter session Runner receipt input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable, "test", "--no-pub", "test/forge_session_runner_receipt_observation_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(forgeConsoleTestEnvironment(inputPath, flutterHome), "FORGE_SESSION_RUNNER_RECEIPT_E2E_INPUT="+inputPath)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter session Runner receipt E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("session Runner receipt Flutter output exceeded the size limit")
	}
}

func runForgeConsoleSessionRunnerReceiptGateE2EWithToken(
	t *testing.T, apiURL, token string, body []byte,
	conversationID, promptID, runID string,
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
		t.Fatalf("Flutter is required for session Runner receipt Gate E2E: %v", err)
	}
	var observation deviceplacement.SessionRunnerReceiptObservation
	if err := json.Unmarshal(body, &observation); err != nil {
		t.Fatalf("decode Flutter session Runner receipt Gate input: %v", err)
	}
	inputJSON, err := json.Marshal(struct {
		APIURL       string                                          `json:"api_url"`
		AccessToken  string                                          `json:"access_token"`
		Observation  deviceplacement.SessionRunnerReceiptObservation `json:"observation"`
		Conversation string                                          `json:"conversation_id"`
		Prompt       string                                          `json:"prompt_id"`
		Run          string                                          `json:"run_id"`
	}{
		APIURL: apiURL, AccessToken: token, Observation: observation,
		Conversation: conversationID, Prompt: promptID, Run: runID,
	})
	if err != nil {
		t.Fatalf("encode Flutter session Runner receipt Gate input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "session-runner-receipt-gate-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter session Runner receipt Gate input: %v", err)
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
		"test/forge_session_runner_receipt_observation_gate_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_SESSION_RUNNER_RECEIPT_GATE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter session Runner receipt Gate E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("session Runner receipt Gate Flutter output exceeded the size limit")
	}
}
