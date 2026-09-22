package appserver

// This test proves that an authenticated Snaplink owner can observe a real
// completed Run from independent CLI and TUI processes. The Run is seeded by
// the local deterministic runtime fixture; the remote surface remains
// read-only and exposes only owner-scoped metadata/timeline markers.

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/auditprojection"
	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/runtimebridge"
	runtimebridgemodel "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
)

func TestSnaplinkAuthenticatedRunObservationToRustHubWhenConfigured(t *testing.T) {
	executable := os.Getenv("FORGE_RUNTIME_BIN")
	if executable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for Snaplink-authenticated Run observation integration")
	}

	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	if err := os.Mkdir(runtimeState, 0o700); err != nil {
		t.Fatal(err)
	}
	initializeRuntimeHubForIntegration(t, executable, runtimeState)
	projectPath, projectID, _ := seedRuntimeConversationScopes(t, executable, runtimeState)
	if err := os.WriteFile(filepath.Join(projectPath, "README.md"), []byte("deterministic Run observation fixture\n"), 0o600); err != nil {
		t.Fatal(err)
	}

	appState := filepath.Join(t.TempDir(), "app-server-state")
	if err := os.Mkdir(appState, 0o700); err != nil {
		t.Fatal(err)
	}
	bridge, err := runtimebridge.New(runtimebridge.Config{
		Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState,
		Timeout: 5 * time.Second,
	})
	if err != nil {
		t.Fatal(err)
	}

	issuer, ssoClient, tokenA, _, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)
	authenticator, err := authn.New(authn.Config{
		Issuer: issuer, Audience: snaplinkForgeTestAudience,
		JWKSURL:          issuer + "/.well-known/jwks.json",
		ExpectedTenantID: snaplinkForgeTestTenant, ExpectedSubjectID: snaplinkForgeTestUser,
		JWKSHTTPClient: ssoClient, JWKSRefreshInterval: 24 * time.Hour,
	})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(authenticator.Close)

	recorder := &conversationHTTPRecorder{}
	api := recorder.wrap(authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(bridge, nil)))
	serverHandler := http.Handler(api)
	if os.Getenv("FORGE_BROWSER_E2E") == "1" {
		webBuildDir := os.Getenv("FORGE_WEB_BUILD_DIR")
		if webBuildDir == "" {
			t.Fatal("FORGE_WEB_BUILD_DIR is required when FORGE_BROWSER_E2E=1")
		}
		serverHandler = serveForgeConsoleWebAssets(webBuildDir, api)
	}
	server := httptest.NewServer(serverHandler)
	t.Cleanup(server.Close)
	client := server.Client()
	conversation := snaplinkCreateProjectConversation(t, client, server.URL, tokenA, projectID)
	promptID := appendSnaplinkRunObservationPrompt(t, client, server.URL, tokenA, conversation.ID)
	seedSnaplinkDeterministicRun(t, executable, runtimeState, projectPath, conversation.ID, promptID)
	placementBody := multiInstanceDevicePlacementPreviewBody(
		t, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant,
	)

	firstCLIRequest := len(recorder.snapshot())
	output, stderr, err := runForgeRuntimeCLI(t, executable, server.URL, tokenA, t.TempDir(),
		"--json", "remote", "runs", "list", conversation.ID, "--limit", "25")
	if err != nil {
		t.Fatalf("authenticated CLI Run list failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var page runmodel.OwnedRunPage
	if err := json.Unmarshal([]byte(output), &page); err != nil || page.ConversationID != conversation.ID ||
		len(page.Runs) != 1 || page.HasMore || page.Runs[0].PromptID != promptID ||
		page.Runs[0].RunID == "" || page.Runs[0].LatestSequence == 0 || page.Runs[0].Status != "completed" {
		t.Fatalf("authenticated CLI Run page=%#v stdout=%q decode=%v", page, output, err)
	}
	runID := page.Runs[0].RunID
	runObserved, err := auditprojection.ProjectRunObserved(
		runtimebridgemodel.Owner{
			Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant,
		}, conversation.ID, page.Runs[0],
	)
	if err != nil {
		t.Fatalf("project populated Run metadata for Flutter: %v", err)
	}
	runObservedPath := conversationCollectionPath + "/" + conversation.ID + "/runs/" + runID + "/observation"
	runObservedResponse := snaplinkConversationRequest(t, client, server.URL, tokenA,
		http.MethodGet, runObservedPath, "", "")
	if runObservedResponse.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(runObservedResponse.Body)
		_ = runObservedResponse.Body.Close()
		t.Fatalf("authenticated Run observation status=%d body=%q", runObservedResponse.StatusCode, body)
	}
	var transportedRunObserved auditprojection.RunObserved
	if !decodeSnaplinkResponse(t, runObservedResponse, &transportedRunObserved) ||
		transportedRunObserved != runObserved {
		t.Fatalf("transported Run observation=%#v want=%#v", transportedRunObserved, runObserved)
	}
	cliObservedOutput, cliObservedStderr, err := runForgeRuntimeCLI(t, executable, server.URL, tokenA, t.TempDir(),
		"--json", "remote", "runs", "observed", conversation.ID, runID)
	if err != nil {
		t.Fatalf("authenticated CLI Run observation failed: stderr=%q stdout=%q err=%v", cliObservedStderr, cliObservedOutput, err)
	}
	var cliObserved auditprojection.RunObserved
	if err := json.Unmarshal([]byte(cliObservedOutput), &cliObserved); err != nil || cliObserved != runObserved {
		t.Fatalf("authenticated CLI Run observation=%#v want=%#v stdout=%q decode=%v", cliObserved, runObserved, cliObservedOutput, err)
	}
	runObservedJSON, err := json.Marshal(runObserved)
	if err != nil {
		t.Fatalf("encode populated Run metadata for Flutter: %v", err)
	}
	sessionObservationBody := multiInstanceSessionDeviceObservationPreviewBody(
		t, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant, conversation.ID, runID,
	)
	sessionObservationInputPath := filepath.Join(t.TempDir(), "session-device-observation-request.json")
	if err := os.WriteFile(sessionObservationInputPath, []byte(sessionObservationBody), 0o600); err != nil {
		t.Fatal(err)
	}
	sessionObservationPath := conversationCollectionPath + "/" + conversation.ID + "/runs/" + runID + "/device-observation/preview"
	sessionObservationOutput, sessionObservationStderr, err := runForgeRuntimeCLI(t, executable, server.URL, tokenA, t.TempDir(),
		"--json", "remote", "session-observation", "preview", "--input", sessionObservationInputPath)
	if err != nil {
		t.Fatalf("authenticated CLI session observation failed: stderr=%q stdout=%q err=%v", sessionObservationStderr, sessionObservationOutput, err)
	}
	sessionObservation, err := deviceplacement.DecodeSessionDeviceObservation(strings.NewReader(sessionObservationOutput))
	if err != nil || sessionObservation.Owner != (deviceplacement.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}) ||
		sessionObservation.ConversationID != conversation.ID || sessionObservation.RunID != runID ||
		sessionObservation.SelectedDeviceID != nil || sessionObservation.SelectedInstanceID != nil ||
		sessionObservation.Authority != (deviceplacement.SessionPlacementAuthority{}) ||
		len(sessionObservation.Inventory.Devices) != 9 ||
		len(sessionObservation.PlacementObservation.Decisions) != 9 ||
		sessionObservation.ResourceSummary.DeviceCount != 9 ||
		sessionObservation.ResourceSummary.RunnerInstanceCount != 9 ||
		sessionObservation.ResourceSummary.AvailableCPUCores != 66 ||
		sessionObservation.ResourceSummary.AvailableMemoryBytes != 135168 ||
		sessionObservation.ResourceSummary.AvailableStorageBytes != 67584 ||
		sessionObservation.ResourceSummary.EligibleDeviceCount != 2 ||
		sessionObservation.ResourceSummary.EligibleInstanceCount != 2 {
		t.Fatalf("authenticated CLI session observation=%#v stdout=%q decode=%v", sessionObservation, sessionObservationOutput, err)
	}

	// Bind the same authenticated owner, Conversation, and observed Run to a
	// caller-supplied P3a placement declaration. This is a pure observation
	// bridge: it selects no device and grants no execution authority.
	firstPlacementRequest := len(recorder.snapshot())
	placementResponse := snaplinkConversationRequest(t, client, server.URL, tokenA, http.MethodPost,
		devicePlacementPreviewPath, "", placementBody)
	var placementResult deviceplacement.Result
	if placementResponse.StatusCode != http.StatusOK || !decodeSnaplinkResponse(t, placementResponse, &placementResult) ||
		placementResult.SchemaVersion != deviceplacement.ResultSchemaVersion ||
		placementResult.EvaluationMode != deviceplacement.EvaluationMode ||
		placementResult.Owner != (deviceplacement.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}) ||
		!placementResult.OwnerDeclarationUnverified || !placementResult.DeviceAttributesUnverified ||
		placementResult.ExecutionAuthorized || placementResult.ReservationCreated || placementResult.DispatchPerformed ||
		len(placementResult.DeviceResults) != 9 || placementResult.DeviceResults[0].DeviceID != "candidate-a" {
		t.Fatalf("authenticated placement observation status=%d result=%#v", placementResponse.StatusCode, placementResult)
	}
	placementRequests := recorder.snapshot()[firstPlacementRequest:]
	wantPlacement := []recordedConversationRequest{{method: http.MethodPost, path: devicePlacementPreviewPath}}
	if len(placementRequests) != len(wantPlacement) || placementRequests[0] != wantPlacement[0] {
		t.Fatalf("placement observation requests=%#v want=%#v", placementRequests, wantPlacement)
	}

	var placementInput deviceplacement.Request
	if err := json.Unmarshal([]byte(placementBody), &placementInput); err != nil {
		t.Fatalf("decode placement observation input: %v", err)
	}
	var sessionObservationInput struct {
		Candidates []deviceplacement.SessionPlacementCandidate `json:"candidates"`
	}
	if err := json.Unmarshal([]byte(sessionObservationBody), &sessionObservationInput); err != nil {
		t.Fatalf("decode session observation input: %v", err)
	}
	placementObservation, err := deviceplacement.ObserveSessionPlacement(deviceplacement.SessionPlacementObservationRequest{
		Owner: placementInput.Owner, ConversationID: conversation.ID, RunID: runID,
		Placement:  placementInput,
		Candidates: sessionObservationInput.Candidates,
	})
	if err != nil {
		t.Fatalf("bind placement observation to Run: %v", err)
	}
	runSummary := page.Runs[0]
	runIntentObservation, err := deviceplacement.ObserveRunIntent(deviceplacement.RunIntentObservationRequest{
		Owner: placementInput.Owner, ConversationID: conversation.ID,
		Prompt: deviceplacement.RunIntentPromptReceipt{
			PromptID: promptID, ConversationID: conversation.ID, Role: "user",
			AcceptedAtMS: int64(runSummary.CreatedAtMS), IntentID: "fixture-intent",
			InitialEventID: "fixture-event", InitialEventSequence: 1,
			InitialEventType: "submitted", Replayed: false,
		},
		Run: deviceplacement.RunIntentRunReference{
			RunID: runID, ConversationID: conversation.ID, PromptID: promptID,
			CreatedAtMS: int64(runSummary.CreatedAtMS), LatestSequence: runSummary.LatestSequence,
			Status: runSummary.Status,
		},
		Placement: placementObservation,
	})
	if err != nil || !runIntentObservation.PromptRunBindingValid || !runIntentObservation.PlacementObservationBound ||
		!runIntentObservation.PreviewOnly || runIntentObservation.Owner != placementInput.Owner ||
		runIntentObservation.ConversationID != conversation.ID || runIntentObservation.PromptID != promptID ||
		runIntentObservation.RunID != runID || runIntentObservation.SelectedDeviceID != nil ||
		runIntentObservation.SelectedInstanceID != nil || runIntentObservation.Authority != (deviceplacement.SessionPlacementAuthority{}) {
		t.Fatalf("Run/placement observation=%#v err=%v", runIntentObservation, err)
	}
	runIntentObservationJSON, err := json.Marshal(runIntentObservation)
	if err != nil {
		t.Fatalf("encode Run-intent observation for Flutter: %v", err)
	}
	const (
		runnerAttemptID = "attempt-1"
		runnerCommandID = "command-1"
		runnerTargetID  = "runner-1"
	)
	runnerIdempotencyKey := runID + ":" + runnerAttemptID + ":" + runnerCommandID
	runnerLeaseProof := deviceplacement.RunnerTerminalLeaseProof{
		AttemptID: runnerAttemptID, TargetID: runnerTargetID, Epoch: 1, FencingToken: "fence-001",
	}
	runnerTerminalCommand := deviceplacement.RunnerTerminalCommand{
		V: 1, CommandID: runnerCommandID, LeaseProof: runnerLeaseProof,
		IdempotencyKey: runnerIdempotencyKey, WorkspaceRef: "workspace-001",
		Argv: []string{"forge-task", "--prompt-ref", promptID}, TimeoutMS: 5000, MaxOutputBytes: 65536,
	}
	runnerCommandSHA256, err := runnerTerminalCommand.CommandSHA256()
	if err != nil {
		t.Fatalf("compute Runner execution intent command digest: %v", err)
	}
	runnerExecutionObservation, err := deviceplacement.ObserveRunnerExecutionIntent(deviceplacement.RunnerExecutionIntentRequest{
		Owner: placementInput.Owner, ConversationID: conversation.ID,
		Prompt: deviceplacement.RunIntentPromptReceipt{
			PromptID: promptID, ConversationID: conversation.ID, Role: "user",
			AcceptedAtMS: int64(runSummary.CreatedAtMS), IntentID: "fixture-intent",
			InitialEventID: "fixture-event", InitialEventSequence: 1,
			InitialEventType: "submitted", Replayed: false,
		},
		Run: deviceplacement.RunIntentRunReference{
			RunID: runID, ConversationID: conversation.ID, PromptID: promptID,
			CreatedAtMS: int64(runSummary.CreatedAtMS), LatestSequence: runSummary.LatestSequence,
			Status: runSummary.Status,
		},
		Binding: deviceplacement.RunnerExecutionIntentBinding{
			ConversationID: conversation.ID, PromptID: promptID, RunID: runID,
			AttemptID: runnerAttemptID, CommandID: runnerCommandID, TargetID: runnerTargetID,
			CommandSHA256: runnerCommandSHA256, IdempotencyKey: runnerIdempotencyKey,
		},
		Command: deviceplacement.RunnerExecutionCommand{
			V: runnerTerminalCommand.V, CommandID: runnerCommandID,
			LeaseProof: deviceplacement.RunnerExecutionLeaseProof{
				AttemptID: runnerLeaseProof.AttemptID, TargetID: runnerLeaseProof.TargetID,
				Epoch: runnerLeaseProof.Epoch, FencingToken: runnerLeaseProof.FencingToken,
			},
			IdempotencyKey: runnerIdempotencyKey, WorkspaceRef: runnerTerminalCommand.WorkspaceRef,
			Argv: runnerTerminalCommand.Argv, TimeoutMS: runnerTerminalCommand.TimeoutMS,
			MaxOutputBytes: runnerTerminalCommand.MaxOutputBytes,
		},
	})
	if err != nil || !runnerExecutionObservation.PromptRunBindingValid ||
		!runnerExecutionObservation.RunnerCommandBindingValid ||
		!runnerExecutionObservation.PreviewOnly || runnerExecutionObservation.SelectedTargetID != nil ||
		runnerExecutionObservation.Authority != (deviceplacement.RunnerExecutionIntentAuthority{}) {
		t.Fatalf("Runner execution observation=%#v err=%v", runnerExecutionObservation, err)
	}
	runnerExecutionObservationJSON, err := json.Marshal(runnerExecutionObservation)
	if err != nil {
		t.Fatalf("encode Runner execution observation for Flutter: %v", err)
	}
	sessionRunnerReceiptObservationJSON, err := marshalSessionRunnerReceiptObservation(
		t, placementInput.Owner, conversation.ID, promptID, runID,
		runnerExecutionObservation, runnerTerminalCommand, runnerLeaseProof,
		runnerCommandSHA256,
		deviceplacement.RunnerTerminalDisposition{
			Kind: "completed", ReceiptSHA256: strings.Repeat("a", 64),
		},
	)
	if err != nil {
		t.Fatalf("encode session Runner receipt observation for Flutter: %v", err)
	}
	timelineOutput, timelineStderr, err := runForgeRuntimeCLI(t, executable, server.URL, tokenA, t.TempDir(),
		"--json", "remote", "runs", "timeline", conversation.ID, runID)
	if err != nil {
		t.Fatalf("authenticated CLI Run timeline failed: stderr=%q stdout=%q err=%v", timelineStderr, timelineOutput, err)
	}
	var timeline runmodel.OwnedRunTimelinePage
	if err := json.Unmarshal([]byte(timelineOutput), &timeline); err != nil ||
		timeline.ConversationID != conversation.ID || timeline.RunID != runID || timeline.AfterSequence != 0 ||
		len(timeline.Events) < 2 || timeline.Events[0].Sequence != 1 || timeline.Events[0].Type != "run_started" ||
		timeline.Events[len(timeline.Events)-1].Type != "run_finished" || timeline.HasMore {
		t.Fatalf("authenticated CLI Run timeline=%#v stdout=%q decode=%v", timeline, timelineOutput, err)
	}
	if strings.Contains(timelineOutput, "deterministic Run observation fixture") ||
		strings.Contains(timelineOutput, "content") || strings.Contains(timelineOutput, "output") {
		t.Fatalf("Run timeline leaked payload fields: %q", timelineOutput)
	}
	cliRequests := recorder.snapshot()[firstCLIRequest:]
	wantCLI := []recordedConversationRequest{
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversation.ID + "/runs", query: "limit=25"},
		{method: http.MethodGet, path: runObservedPath},
		{method: http.MethodGet, path: runObservedPath},
		{method: http.MethodPost, path: sessionObservationPath},
		{method: http.MethodPost, path: devicePlacementPreviewPath},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversation.ID + "/runs/" + runID + "/timeline", query: "after_sequence=0&limit=128"},
	}
	if len(cliRequests) != len(wantCLI) {
		t.Fatalf("Run observation CLI issued %d requests; want %#v got %#v", len(cliRequests), wantCLI, cliRequests)
	}
	for index, request := range cliRequests {
		if request != wantCLI[index] {
			t.Fatalf("Run observation CLI request[%d]=%#v want=%#v", index, request, wantCLI[index])
		}
	}

	sessionRunnerReceiptPath := conversationCollectionPath + "/" + conversation.ID + "/runs/" + runID + "/runner-receipt-observation/preview"
	runSessionRunnerReceiptPreviewE2E(t, executable, server.URL, tokenA, conversation.ID, promptID, runID,
		placementInput.Owner, runnerCommandID, runnerCommandSHA256, sessionRunnerReceiptObservationJSON,
		recorder, sessionRunnerReceiptPath, "completed", false)
	uncertainSessionRunnerReceiptObservationJSON, err := marshalSessionRunnerReceiptObservation(
		t, placementInput.Owner, conversation.ID, promptID, runID,
		runnerExecutionObservation, runnerTerminalCommand, runnerLeaseProof,
		runnerCommandSHA256,
		deviceplacement.RunnerTerminalDisposition{
			Kind:   "uncertain",
			Reason: "transport ended after effect boundary",
		},
	)
	if err != nil {
		t.Fatalf("encode uncertain session Runner receipt observation: %v", err)
	}
	runSessionRunnerReceiptPreviewE2E(t, executable, server.URL, tokenA, conversation.ID, promptID, runID,
		placementInput.Owner, runnerCommandID, runnerCommandSHA256, uncertainSessionRunnerReceiptObservationJSON,
		recorder, sessionRunnerReceiptPath, "uncertain", true)

	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		firstFlutterRequest := len(recorder.snapshot())
		runForgeConsoleRunObservationAPITest(
			t, server.URL, tokenA, issuer, conversation.ID, runID, sessionObservationBody,
			json.RawMessage(sessionObservationOutput), runObservedJSON,
			sessionRunnerReceiptObservationJSON,
		)
		flutterRequests := recorder.snapshot()[firstFlutterRequest:]
		sessionObservationPath := conversationCollectionPath + "/" + conversation.ID + "/runs/" + runID + "/device-observation/preview"
		wantFlutter := []recordedConversationRequest{
			{method: http.MethodGet, path: conversationCollectionPath + "/" + conversation.ID + "/runs", query: "limit=25"},
			{method: http.MethodGet, path: runObservedPath},
			{method: http.MethodGet, path: conversationCollectionPath + "/" + conversation.ID + "/runs/" + runID + "/timeline", query: "after_sequence=0&limit=128"},
			{method: http.MethodPost, path: sessionObservationPath},
			{method: http.MethodPost, path: sessionRunnerReceiptPath},
		}
		if len(flutterRequests) != len(wantFlutter) {
			t.Fatalf("Run observation Flutter API issued %d requests; want %#v got %#v",
				len(flutterRequests), wantFlutter, flutterRequests)
		}
		for index, request := range flutterRequests {
			if request != wantFlutter[index] {
				t.Fatalf("Run observation Flutter request[%d]=%#v want=%#v", index, request, wantFlutter[index])
			}
		}

		assertNoDeviceOrDispatchRequests(t, flutterRequests)

		firstUncertainFlutterRequest := len(recorder.snapshot())
		runForgeConsoleRunObservationAPITest(
			t, server.URL, tokenA, issuer, conversation.ID, runID, sessionObservationBody,
			json.RawMessage(sessionObservationOutput), runObservedJSON,
			uncertainSessionRunnerReceiptObservationJSON,
		)
		uncertainFlutterRequests := recorder.snapshot()[firstUncertainFlutterRequest:]
		if len(uncertainFlutterRequests) != len(wantFlutter) {
			t.Fatalf("uncertain Run observation Flutter API issued %d requests; want %#v got %#v",
				len(uncertainFlutterRequests), wantFlutter, uncertainFlutterRequests)
		}
		for index, request := range uncertainFlutterRequests {
			if request != wantFlutter[index] {
				t.Fatalf("uncertain Run observation Flutter request[%d]=%#v want=%#v", index, request, wantFlutter[index])
			}
		}
		assertNoDeviceOrDispatchRequests(t, uncertainFlutterRequests)

		firstFlutterWidgetRequest := len(recorder.snapshot())
		runForgeConsoleRunObservationWidgetE2ETest(
			t, server.URL, tokenA, conversation.ID, runID, runObservedJSON, sessionObservationBody,
			runIntentObservationJSON, runnerExecutionObservationJSON,
			sessionRunnerReceiptObservationJSON,
		)
		widgetRequests := recorder.snapshot()[firstFlutterWidgetRequest:]
		assertForgeConsoleNativeRunObservationRequests(t, widgetRequests, conversation.ID, runID)
		assertNoDeviceOrDispatchRequests(t, widgetRequests)

		firstUncertainFlutterWidgetRequest := len(recorder.snapshot())
		runForgeConsoleRunObservationWidgetE2ETest(
			t, server.URL, tokenA, conversation.ID, runID, runObservedJSON, sessionObservationBody,
			runIntentObservationJSON, runnerExecutionObservationJSON,
			uncertainSessionRunnerReceiptObservationJSON,
		)
		uncertainWidgetRequests := recorder.snapshot()[firstUncertainFlutterWidgetRequest:]
		assertForgeConsoleNativeRunObservationRequests(t, uncertainWidgetRequests, conversation.ID, runID)
		assertNoDeviceOrDispatchRequests(t, uncertainWidgetRequests)
	}

	if os.Getenv("FORGE_BROWSER_E2E") == "1" {
		firstBrowserRequest := len(recorder.snapshot())
		runForgeConsoleBrowserRunObservationE2EWithToken(
			t, server.URL, tokenA, conversation.ID, runID,
			runObservedJSON, []byte(sessionObservationBody), nil,
			sessionRunnerReceiptObservationJSON,
		)
		browserRequests := recorder.snapshot()[firstBrowserRequest:]
		assertForgeConsoleBrowserRunObservationRequests(
			t, browserRequests, conversation.ID, runID,
			true, sessionRunnerReceiptObservationJSON != nil,
		)
		assertNoDeviceOrDispatchRequests(t, browserRequests)

		firstUncertainBrowserRequest := len(recorder.snapshot())
		runForgeConsoleBrowserRunObservationE2EWithToken(
			t, server.URL, tokenA, conversation.ID, runID,
			runObservedJSON, []byte(sessionObservationBody), nil,
			uncertainSessionRunnerReceiptObservationJSON,
		)
		uncertainBrowserRequests := recorder.snapshot()[firstUncertainBrowserRequest:]
		assertForgeConsoleBrowserRunObservationRequests(
			t, uncertainBrowserRequests, conversation.ID, runID,
			true, uncertainSessionRunnerReceiptObservationJSON != nil,
		)
		assertNoDeviceOrDispatchRequests(t, uncertainBrowserRequests)
	}

	firstTUIRequest := len(recorder.snapshot())
	runObservationTUI(t, executable, server.URL, tokenA, conversation.ID, runID, sessionObservationInputPath)
	tuiRequests := recorder.snapshot()[firstTUIRequest:]
	wantTUI := []recordedConversationRequest{
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversation.ID + "/prompts", query: "limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversation.ID + "/runs", query: "limit=25"},
		{method: http.MethodGet, path: runObservedPath},
		{method: http.MethodPost, path: sessionObservationPath},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversation.ID + "/runs/" + runID + "/timeline", query: "after_sequence=0&limit=128"},
	}
	if len(tuiRequests) != len(wantTUI) {
		t.Fatalf("Run observation TUI issued %d requests; want %#v got %#v", len(tuiRequests), wantTUI, tuiRequests)
	}
	for index, request := range tuiRequests {
		if request != wantTUI[index] {
			t.Fatalf("Run observation TUI request[%d]=%#v want=%#v", index, request, wantTUI[index])
		}
	}

	firstTUIResumeRequest := len(recorder.snapshot())
	runRunTimelineResumeTUI(t, executable, server.URL, issuer, snaplinkForgeTestUser,
		snaplinkForgeTestTenant, tokenA, conversation.ID, runID)
	assertRunTimelineResumeTUIRequests(t, recorder.snapshot()[firstTUIResumeRequest:], conversation.ID, runID)

}

func assertForgeConsoleBrowserRunObservationRequests(
	t *testing.T, requests []recordedConversationRequest, conversationID, runID string,
	expectSessionObservation, expectReceiptPreview bool,
) {
	t.Helper()
	want := map[recordedConversationRequest]bool{
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=50"}:                                         true,
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/prompts", query: "limit=100"}:    true,
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/runs", query: "limit=25"}:        true,
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/runs/" + runID + "/observation"}: true,
	}
	timelinePath := conversationCollectionPath + "/" + conversationID + "/runs/" + runID + "/timeline"
	receiptPreviewRequest := recordedConversationRequest{
		method: http.MethodPost,
		path:   conversationCollectionPath + "/" + conversationID + "/runs/" + runID + "/runner-receipt-observation/preview",
	}
	sessionObservationRequest := recordedConversationRequest{
		method: http.MethodPost,
		path:   conversationCollectionPath + "/" + conversationID + "/runs/" + runID + "/device-observation/preview",
	}
	seen := make(map[recordedConversationRequest]bool, len(want))
	sessionObservationCount := 0
	receiptPreviewCount := 0
	var timelineAfter []uint64
	for _, request := range requests {
		if request.path == conversationChangesPath {
			if request.method != http.MethodGet || request.query != "after_cursor=0&limit=128" {
				t.Fatalf("unexpected Forge browser change-feed request %#v", request)
			}
			continue
		}
		if request.path == timelinePath {
			if request.method != http.MethodGet {
				t.Fatalf("unexpected Forge browser Run timeline request %#v", request)
			}
			sequence, ok := parseCursorQuery(request.query, "after_sequence")
			if !ok {
				t.Fatalf("unexpected Forge browser Run timeline cursor %#v", request)
			}
			timelineAfter = append(timelineAfter, sequence)
			continue
		}
		if expectSessionObservation && request == sessionObservationRequest {
			sessionObservationCount++
			continue
		}
		if expectReceiptPreview && request == receiptPreviewRequest {
			receiptPreviewCount++
			continue
		}
		if request.method != http.MethodGet || !want[request] {
			t.Fatalf("unexpected Forge browser Run observation request %#v", request)
		}
		seen[request] = true
	}
	if len(timelineAfter) < 2 || timelineAfter[0] != 0 || timelineAfter[1] == 0 {
		t.Fatalf("Forge browser Run timeline resume cursors=%#v; want [0, positive, ...]", timelineAfter)
	}
	for _, sequence := range timelineAfter[1:] {
		if sequence == 0 {
			t.Fatalf("Forge browser Run timeline replayed a zero cursor after resume: %#v", timelineAfter)
		}
	}
	for request := range want {
		if !seen[request] {
			t.Fatalf("Forge browser omitted Run observation request %#v; observed %#v", request, requests)
		}
	}
	if expectSessionObservation && sessionObservationCount != 1 {
		t.Fatalf("Forge browser session device observation POST count=%d want=1; observed %#v", sessionObservationCount, requests)
	}
	if expectReceiptPreview && receiptPreviewCount != 1 {
		t.Fatalf("Forge browser receipt preview POST count=%d want=1; observed %#v", receiptPreviewCount, requests)
	}
}

func appendSnaplinkRunObservationPrompt(
	t *testing.T, client *http.Client, baseURL, token, conversationID string,
) string {
	t.Helper()
	response := snaplinkConversationRequest(t, client, baseURL, token, http.MethodPost,
		conversationCollectionPath+"/"+conversationID+"/prompts", "run-observation-prompt",
		`{"content":"observe this completed Run","expected_version":1}`)
	var result struct {
		Prompt struct {
			ID             string `json:"id"`
			ConversationID string `json:"conversation_id"`
			Role           string `json:"role"`
		} `json:"prompt"`
		AggregateVersion uint64 `json:"aggregate_version"`
		Replayed         bool   `json:"replayed"`
	}
	if response.StatusCode != http.StatusCreated || !decodeSnaplinkResponse(t, response, &result) ||
		result.Prompt.ID == "" || result.Prompt.ConversationID != conversationID ||
		result.Prompt.Role != "user" || result.AggregateVersion != 2 || result.Replayed {
		t.Fatalf("Run observation Prompt status=%d result=%#v", response.StatusCode, result)
	}
	return result.Prompt.ID
}

func seedSnaplinkDeterministicRun(
	t *testing.T, executable, runtimeState, projectPath, conversationID, promptID string,
) {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, executable,
		"--state-dir", runtimeState, "--json", "--idempotency-key", "snaplink-run-observation",
		"-C", projectPath, "run", "start", conversationID, promptID, "--read", "README.md")
	command.Env = []string{}
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("seed deterministic Run: %v: %s", err, output)
	}
	if len(output) == 0 {
		t.Fatal("seed deterministic Run returned no runtime evidence")
	}
}
