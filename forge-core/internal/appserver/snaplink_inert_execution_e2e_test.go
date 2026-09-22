package appserver

// This test exercises the private, inert execution surface with real
// Snaplink-issued access tokens and a real Rust Hub. It deliberately remains
// test-only: the running app-server route surface does not expose consent,
// pending-intent, live device, or Runner authority. The test-only surface may
// expose the stateless caller-declaration placement preview for client E2E.

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/yangwb1123/snaplink/domains/authenticators"
	"github.com/yangwb1123/snaplink/infrastructure/defaultimpl"
	"github.com/yangwb1123/snaplink/interfaces/sso"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/executionprofile"
	"forgeos/forge-core/internal/runtimebridge"
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
)

func TestSnaplinkAuthenticatedInertExecutionToRustHubWhenConfigured(t *testing.T) {
	executable := os.Getenv("FORGE_RUNTIME_BIN")
	if executable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for Snaplink-authenticated Go HTTP to Rust Hub execution integration")
	}

	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	if err := os.Mkdir(runtimeState, 0o700); err != nil {
		t.Fatal(err)
	}
	initializeRuntimeHubForIntegration(t, executable, runtimeState)
	_, projectID, _ := seedRuntimeConversationScopes(t, executable, runtimeState)

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

	profiles, profile := snaplinkExecutionProfile(t, projectID)
	issuer, ssoClient, tokenA, tokenB, closeSnaplink := startSnaplinkForgeTestIssuer(t)
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
	inertRoutes := newConversationRoutesWithInertExecutionAPI(bridge, profiles)
	// The inventory route is mounted only in this test process. The production
	// route constructor remains unchanged and therefore still returns 404.
	deviceInventorySource := &fixtureDeviceInventoryReadSource{
		value: fixtureDeviceInventoryReadValueMultiple(deviceplacement.Owner{
			Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant,
		}),
	}
	testRoutes := http.NewServeMux()
	testRoutes.Handle(deviceInventoryReadCandidatePath, newDeviceInventoryReadCandidateRoutes(&deviceInventoryReadCandidateConfig{
		Enabled: true, Source: deviceInventorySource,
	}))
	testRoutes.Handle("/", inertRoutes)
	apiHandler := recorder.wrap(authenticator.Handler(testRoutes))
	serverHandler := http.Handler(apiHandler)
	if os.Getenv("FORGE_BROWSER_E2E") == "1" {
		webBuildDir := os.Getenv("FORGE_WEB_BUILD_DIR")
		if webBuildDir == "" {
			t.Fatal("FORGE_WEB_BUILD_DIR is required when FORGE_BROWSER_E2E=1")
		}
		serverHandler = serveForgeConsoleWebAssets(webBuildDir, apiHandler)
	}
	inert := httptest.NewServer(serverHandler)
	t.Cleanup(inert.Close)
	client := inert.Client()

	conversation := snaplinkCreateProjectConversation(t, client, inert.URL, tokenA, projectID)
	preview := snaplinkPreviewExecutionConsent(t, client, inert.URL, tokenA, conversation.ID, projectID, profile)
	grant := snaplinkGrantExecutionConsent(t, client, inert.URL, tokenA, conversation.ID, preview)
	first := snaplinkSubmitPendingIntent(t, client, inert.URL, tokenB, conversation.ID, "snaplink-execution-submit", 1,
		"prompt sent from client B", projectID, profile)
	snaplinkAssertPendingIntentViews(t, client, inert.URL, tokenB, conversation.ID, first)
	firstCLIRequest := len(recorder.snapshot())
	freshCLIIntent := assertSnaplinkRustCLISharedSession(t, executable, inert.URL, issuer, tokenA, tokenB, conversation.ID)
	assertSnaplinkRustCLIRequestsHaveNoExecutionOrDeviceEffects(t, recorder.snapshot()[firstCLIRequest:], conversation.ID, first.Intent.IntentID, freshCLIIntent.Intent.IntentID)
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		firstConsoleRequest := len(recorder.snapshot())
		runForgeConsoleLiveAPITestWithToken(
			t, recorder, inert.URL, tokenB, conversation.ID, 4,
			"Prompt submitted by Snaplink Flutter API client",
			"Prompt submitted from Snaplink Flutter screen",
			"prompt sent from client B", 4, "snaplink-flutter-client-prompt",
			devicePlacementPreviewBody(issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant),
			first.Intent.IntentID, true,
		)
		assertForgeConsoleAPIAndWidgetRequestsHaveNoExecutionOrDeviceEffects(
			t, recorder.snapshot()[firstConsoleRequest:], conversation.ID, first.Intent.IntentID, true,
		)
		firstPendingWidgetRequest := len(recorder.snapshot())
		runForgeConsolePendingIntentWidgetE2EWithToken(
			t, inert.URL, tokenB, conversation.ID, first.Intent.IntentID,
		)
		pendingWidgetRequests := recorder.snapshot()[firstPendingWidgetRequest:]
		var pendingMetadataReads []recordedConversationRequest
		var pendingTimelineReads []recordedConversationRequest
		for _, request := range pendingWidgetRequests {
			if request.method == http.MethodGet &&
				request.path == conversationCollectionPath+"/"+conversation.ID+"/run-intents" {
				pendingMetadataReads = append(pendingMetadataReads, request)
			}
			if request.method == http.MethodGet &&
				request.path == conversationCollectionPath+"/"+conversation.ID+"/run-intents/"+first.Intent.IntentID+"/timeline" {
				pendingTimelineReads = append(pendingTimelineReads, request)
			}
		}
		if len(pendingMetadataReads) != 1 || pendingMetadataReads[0].query != "limit=25" {
			t.Fatalf("Flutter pending metadata widget requests=%#v; expected one bounded owner-scoped list GET", pendingWidgetRequests)
		}
		if len(pendingTimelineReads) != 1 || pendingTimelineReads[0].query != "after_sequence=0&limit=25" {
			t.Fatalf("Flutter pending metadata widget requests=%#v; expected one expanded payload-free timeline GET", pendingWidgetRequests)
		}
		for _, request := range pendingWidgetRequests {
			if request.method == http.MethodPost &&
				strings.Contains(request.path, "/run-intents") {
				t.Fatalf("pending metadata widget performed a Run-intent write: %#v", request)
			}
		}
		firstInventoryRequest := len(recorder.snapshot())
		runForgeConsoleDeviceInventoryCandidateE2EWithToken(
			t, inert.URL, tokenB, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant,
		)
		inventoryRequests := recorder.snapshot()[firstInventoryRequest:]
		var deviceRequests []recordedConversationRequest
		for _, request := range inventoryRequests {
			if request.path == deviceInventoryReadCandidatePath {
				deviceRequests = append(deviceRequests, request)
			}
		}
		if len(deviceRequests) != 1 || deviceRequests[0] != (recordedConversationRequest{
			method: http.MethodGet, path: deviceInventoryReadCandidatePath,
		}) {
			t.Fatalf("Flutter inventory candidate requests=%#v; expected one owner-bound GET among Sessions reads", inventoryRequests)
		}
		// The Rust CLI has already exercised the same candidate before this
		// Flutter read. Keep the source count explicit so a hidden retry or
		// extra device request cannot pass unnoticed.
		if deviceInventorySource.calls != 2 || deviceInventorySource.owner != (model.Owner{
			Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant,
		}) {
			t.Fatalf("Flutter inventory candidate source=%#v; expected one CLI and one Flutter verified-owner read", deviceInventorySource)
		}
	}
	firstTUIRequest := len(recorder.snapshot())
	tuiAggregateVersion := first.Intent.AggregateVersion + 1
	tuiChangeCursor := uint64(3)
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		// Flutter submits one fresh pending intent and two ordinary Prompts
		// before the TUI starts. Each write advances the aggregate and change
		// feed, while the first fresh receipt remains owner-visible.
		tuiAggregateVersion += 3
		tuiChangeCursor += 3
	}
	assertSnaplinkRustTUISharedSession(
		t, executable, inert.URL, issuer, tokenB, conversation.ID, first.Intent.IntentID,
		tuiAggregateVersion, tuiChangeCursor,
	)
	assertSnaplinkRustTUIRequestsHaveNoExecutionOrDeviceEffects(t, recorder.snapshot()[firstTUIRequest:], conversation.ID, first.Intent.IntentID)
	snaplinkAssertFreshPendingIntentReadback(t, client, inert.URL, tokenB, conversation.ID, first)
	if os.Getenv("FORGE_BROWSER_E2E") == "1" {
		runForgeConsoleBrowserE2EWithToken(
			t, inert.URL, tokenB, conversation.ID,
			"Prompt submitted from Snaplink Flutter Web",
			false,
		)
	}

	revocation := snaplinkConversationRequest(t, client, inert.URL, tokenA, http.MethodDelete,
		executionConsentCollectionPath+"/"+grant.Grant.GrantID, "snaplink-execution-revoke", "")
	if revocation.StatusCode != http.StatusOK {
		t.Fatalf("Snaplink consent revoke status=%d body=%q", revocation.StatusCode, readConversationClientBody(t, revocation))
	}

	afterRevoke := snaplinkConversationRequest(t, client, inert.URL, tokenB, http.MethodPost,
		conversationCollectionPath+"/"+conversation.ID+"/run-intents", "snaplink-execution-after-revoke",
		`{"content":"new task after revoke","expected_version":2}`)
	if afterRevoke.StatusCode != http.StatusConflict {
		t.Fatalf("pending intent after consent revocation status=%d body=%q", afterRevoke.StatusCode, readConversationClientBody(t, afterRevoke))
	}

	replay := snaplinkConversationRequest(t, client, inert.URL, tokenB, http.MethodPost,
		conversationCollectionPath+"/"+conversation.ID+"/run-intents", "snaplink-execution-submit",
		`{"content":"prompt sent from client B","expected_version":1}`)
	var replayResult intentmodel.PendingRunIntentSubmissionResult
	if replay.StatusCode != http.StatusOK || !decodeSnaplinkResponse(t, replay, &replayResult) ||
		!replayResult.Replayed || replayResult.Intent.IntentID != first.Intent.IntentID ||
		replayResult.Prompt.ID != first.Prompt.ID {
		t.Fatalf("original pending intent replay after revoke status=%d result=%#v", replay.StatusCode, replayResult)
	}

	production := authenticator.Handler(newConversationRoutes(bridge))
	for _, path := range []string{
		conversationCollectionPath + "/" + conversation.ID + "/execution-consents",
		conversationCollectionPath + "/" + conversation.ID + "/run-intents",
	} {
		request, err := http.NewRequest(http.MethodGet, "http://forge.test"+path, nil)
		if err != nil {
			t.Fatal(err)
		}
		request.Header.Set("Authorization", "Bearer "+tokenB)
		recorder := httptest.NewRecorder()
		production.ServeHTTP(recorder, request)
		if recorder.Code != http.StatusNotFound {
			t.Fatalf("production execution path %s status=%d body=%q", path, recorder.Code, recorder.Body.String())
		}
	}
}

func assertSnaplinkRustCLISharedSession(
	t *testing.T,
	executable, apiURL, issuer, tokenA, tokenB, conversationID string,
) intentmodel.PendingRunIntentSubmissionResult {
	t.Helper()
	listOutput, listStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, tokenA, t.TempDir(), "--json", "remote", "sessions", "list",
	)
	if err != nil {
		t.Fatalf("Snaplink JWT Rust CLI session list failed: stderr=%q stdout=%q err=%v", listStderr, listOutput, err)
	}
	var page model.OwnedConversationPage
	if err := json.Unmarshal([]byte(listOutput), &page); err != nil || len(page.Conversations) != 1 ||
		page.Conversations[0].Conversation.ID != conversationID || page.HasMore {
		t.Fatalf("Snaplink JWT Rust CLI session page=%#v stdout=%q decode=%v", page, listOutput, err)
	}

	promptOutput, promptStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, tokenB, t.TempDir(), "--json", "remote", "prompts", "list", conversationID,
	)
	if err != nil {
		t.Fatalf("Snaplink JWT Rust CLI Prompt list failed: stderr=%q stdout=%q err=%v", promptStderr, promptOutput, err)
	}
	var history model.ConversationPromptPage
	if err := json.Unmarshal([]byte(promptOutput), &history); err != nil || history.ConversationID != conversationID ||
		len(history.Prompts) != 1 || history.Prompts[0].Content != "prompt sent from client B" {
		t.Fatalf("Snaplink JWT Rust CLI Prompt history=%#v stdout=%q decode=%v", history, promptOutput, err)
	}

	changeOutput, changeStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, tokenA, t.TempDir(), "--json", "remote", "changes", "list", "--after-cursor", "0",
	)
	if err != nil {
		t.Fatalf("Snaplink JWT Rust CLI change feed failed: stderr=%q stdout=%q err=%v", changeStderr, changeOutput, err)
	}
	var changes model.OwnedConversationChangePage
	if err := json.Unmarshal([]byte(changeOutput), &changes); err != nil || changes.AfterCursor != 0 ||
		changes.ScannedThroughCursor != 2 || changes.HasMore || len(changes.Changes) != 2 ||
		changes.Changes[0].ConversationID != conversationID ||
		changes.Changes[0].Kind != "conversation_created" ||
		changes.Changes[1].ConversationID != conversationID ||
		changes.Changes[1].Kind != "prompt_appended" {
		t.Fatalf("Snaplink JWT Rust CLI change feed=%#v stdout=%q decode=%v", changes, changeOutput, err)
	}

	submitOutput, submitStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, tokenB, t.TempDir(), "--json", "--idempotency-key", "snaplink-execution-submit",
		"remote", "run-intents", "submit", conversationID, "--expected-version", "1", "prompt sent from client B",
	)
	if err != nil {
		t.Fatalf("Snaplink JWT Rust CLI pending Run-intent submit failed: stderr=%q stdout=%q err=%v", submitStderr, submitOutput, err)
	}
	var submitted intentmodel.PendingRunIntentSubmissionResult
	if err := json.Unmarshal([]byte(submitOutput), &submitted); err != nil ||
		!submitted.Replayed || submitted.Prompt.Content != "prompt sent from client B" ||
		submitted.Intent.Status != "pending" {
		t.Fatalf("Snaplink JWT Rust CLI pending Run-intent submit=%#v stdout=%q decode=%v", submitted, submitOutput, err)
	}

	freshSubmitOutput, freshSubmitStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, tokenB, t.TempDir(), "--json", "--idempotency-key", "snaplink-cli-pending-fresh",
		"remote", "run-intents", "submit", conversationID, "--expected-version", "2", "pending intent from Rust CLI",
	)
	if err != nil {
		t.Fatalf("Snaplink JWT Rust CLI fresh pending Run-intent submit failed: stderr=%q stdout=%q err=%v", freshSubmitStderr, freshSubmitOutput, err)
	}
	var freshSubmitted intentmodel.PendingRunIntentSubmissionResult
	if err := json.Unmarshal([]byte(freshSubmitOutput), &freshSubmitted); err != nil ||
		freshSubmitted.Replayed || freshSubmitted.Prompt.Content != "pending intent from Rust CLI" ||
		freshSubmitted.Intent.Status != "pending" || freshSubmitted.Intent.IntentID == submitted.Intent.IntentID {
		t.Fatalf("Snaplink JWT Rust CLI fresh pending Run-intent submit=%#v stdout=%q decode=%v", freshSubmitted, freshSubmitOutput, err)
	}

	runOutput, runStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, tokenA, t.TempDir(), "--json", "remote", "runs", "list", conversationID, "--limit", "25",
	)
	if err != nil {
		t.Fatalf("Snaplink JWT Rust CLI Run observation failed: stderr=%q stdout=%q err=%v", runStderr, runOutput, err)
	}
	var runs runmodel.OwnedRunPage
	if err := json.Unmarshal([]byte(runOutput), &runs); err != nil || runs.ConversationID != conversationID ||
		runs.Runs == nil || len(runs.Runs) != 0 || runs.HasMore {
		t.Fatalf("Snaplink JWT Rust CLI observed an ordinary Run: page=%#v stdout=%q decode=%v", runs, runOutput, err)
	}

	intentOutput, intentStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, tokenB, t.TempDir(), "--json", "remote", "run-intents", "list", conversationID,
	)
	if err != nil {
		t.Fatalf("Snaplink JWT Rust CLI pending Run-intent list failed: stderr=%q stdout=%q err=%v", intentStderr, intentOutput, err)
	}
	var intents intentmodel.OwnedPendingRunIntentPage
	if err := json.Unmarshal([]byte(intentOutput), &intents); err != nil || intents.ConversationID != conversationID ||
		len(intents.Intents) != 2 || intents.Intents[0].Status != "pending" || intents.Intents[0].IntentID == "" || intents.HasMore {
		t.Fatalf("Snaplink JWT Rust CLI pending Run-intent page=%#v stdout=%q decode=%v", intents, intentOutput, err)
	}
	var originalIntentPresent bool
	var freshIntentPresent bool
	for _, intent := range intents.Intents {
		if intent.IntentID == submitted.Intent.IntentID {
			originalIntentPresent = true
		}
		if intent.IntentID == freshSubmitted.Intent.IntentID &&
			intent.Status == "pending" && intent.AggregateVersion == freshSubmitted.Intent.AggregateVersion &&
			intent.PromptID == freshSubmitted.Prompt.ID && intent.LatestSequence == 1 {
			freshIntentPresent = true
		}
	}
	if !originalIntentPresent {
		t.Fatalf("Snaplink JWT Rust CLI pending Run-intent page omitted original receipt: %#v", intents)
	}
	if !freshIntentPresent {
		t.Fatalf("Snaplink JWT Rust CLI pending Run-intent page omitted fresh receipt: page=%#v fresh=%#v", intents, freshSubmitted)
	}
	timelineOutput, timelineStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, tokenB, t.TempDir(), "--json", "remote", "run-intents", "timeline", conversationID, submitted.Intent.IntentID,
	)
	if err != nil {
		t.Fatalf("Snaplink JWT Rust CLI pending Run-intent timeline failed: stderr=%q stdout=%q err=%v", timelineStderr, timelineOutput, err)
	}
	var timeline intentmodel.OwnedPendingRunIntentTimelinePage
	if err := json.Unmarshal([]byte(timelineOutput), &timeline); err != nil || timeline.ConversationID != conversationID ||
		timeline.IntentID != submitted.Intent.IntentID || len(timeline.Events) != 1 || timeline.Events[0].Type != "submitted" || timeline.HasMore {
		t.Fatalf("Snaplink JWT Rust CLI pending Run-intent timeline=%#v stdout=%q decode=%v", timeline, timelineOutput, err)
	}
	freshTimelineOutput, freshTimelineStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, tokenB, t.TempDir(), "--json", "remote", "run-intents", "timeline", conversationID, freshSubmitted.Intent.IntentID,
	)
	if err != nil {
		t.Fatalf("Snaplink JWT Rust CLI fresh pending Run-intent timeline failed: stderr=%q stdout=%q err=%v", freshTimelineStderr, freshTimelineOutput, err)
	}
	var freshTimeline intentmodel.OwnedPendingRunIntentTimelinePage
	if err := json.Unmarshal([]byte(freshTimelineOutput), &freshTimeline); err != nil ||
		freshTimeline.ConversationID != conversationID || freshTimeline.IntentID != freshSubmitted.Intent.IntentID ||
		len(freshTimeline.Events) != 1 || freshTimeline.Events[0].Type != "submitted" || freshTimeline.HasMore {
		t.Fatalf("Snaplink JWT Rust CLI fresh pending Run-intent timeline=%#v stdout=%q decode=%v", freshTimeline, freshTimelineOutput, err)
	}

	inventoryOutput, inventoryStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, tokenB, t.TempDir(), "--json", "remote", "inventory", "show",
	)
	if err != nil {
		t.Fatalf("Snaplink JWT Rust CLI inventory candidate failed: stderr=%q stdout=%q err=%v", inventoryStderr, inventoryOutput, err)
	}
	var inventory deviceplacement.SessionDeviceObservationInventory
	if err := json.Unmarshal([]byte(inventoryOutput), &inventory); err != nil ||
		inventory.Owner.Subject != snaplinkForgeTestUser || inventory.Owner.TenantID != snaplinkForgeTestTenant ||
		len(inventory.Devices) != 2 || inventory.Devices[0].Device.DeviceID != "device-a" ||
		inventory.Devices[0].InstanceID != "runner-a" || inventory.Devices[1].Device.DeviceID != "device-b" ||
		inventory.Devices[1].InstanceID != "runner-b" || inventory.Devices[0].Device.Owner != inventory.Owner ||
		inventory.Devices[1].Device.Owner != inventory.Owner ||
		inventory.Devices[0].Device.OS != "linux" || inventory.Devices[0].Device.Architecture != "amd64" ||
		inventory.Devices[0].Device.AvailableCPUCores != 8 || inventory.Devices[0].Device.AvailableMemoryBytes != 16384 ||
		inventory.Devices[0].Device.AvailableStorage != 8192 || len(inventory.Devices[0].Device.Runtimes) != 1 ||
		inventory.Devices[0].Device.Runtimes[0] != "oci" || inventory.Devices[0].Device.ConcurrencyLimit != 4 ||
		inventory.Devices[0].Device.ActiveConcurrency != 1 ||
		inventory.Devices[1].Device.OS != "linux" || inventory.Devices[1].Device.Architecture != "arm64" ||
		inventory.Devices[1].Device.AvailableCPUCores != 4 || inventory.Devices[1].Device.AvailableMemoryBytes != 8192 ||
		inventory.Devices[1].Device.AvailableStorage != 4096 || len(inventory.Devices[1].Device.Runtimes) != 1 ||
		inventory.Devices[1].Device.Runtimes[0] != "oci" || inventory.Devices[1].Device.ConcurrencyLimit != 2 ||
		inventory.Devices[1].Device.ActiveConcurrency != 0 ||
		!inventory.OwnerDeclarationUnverified || !inventory.InventoryUnverified ||
		inventory.ExecutionAuthorized || inventory.ReservationCreated || inventory.DispatchPerformed {
		t.Fatalf("Snaplink JWT Rust CLI inventory candidate=%#v stdout=%q decode=%v", inventory, inventoryOutput, err)
	}

	placementInput := filepath.Join(t.TempDir(), "placement.json")
	if err := os.WriteFile(placementInput,
		[]byte(devicePlacementPreviewBody(issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant)), 0o600); err != nil {
		t.Fatal(err)
	}
	placementOutput, placementStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, tokenA, t.TempDir(), "--json", "remote", "placement", "preview", "--input", placementInput,
	)
	if err != nil {
		t.Fatalf("Snaplink JWT Rust CLI placement preview failed: stderr=%q stdout=%q err=%v", placementStderr, placementOutput, err)
	}
	var placement deviceplacement.Result
	if err := json.Unmarshal([]byte(placementOutput), &placement); err != nil ||
		placement.SchemaVersion != deviceplacement.ResultSchemaVersion ||
		placement.EvaluationMode != deviceplacement.EvaluationMode ||
		!placement.OwnerDeclarationUnverified || !placement.DeviceAttributesUnverified ||
		placement.ExecutionAuthorized || placement.ReservationCreated || placement.DispatchPerformed ||
		len(placement.DeviceResults) != 1 || placement.DeviceResults[0].DeviceID != "device-1" {
		t.Fatalf("Snaplink JWT Rust CLI placement preview=%#v stdout=%q decode=%v", placement, placementOutput, err)
	}

	return freshSubmitted
}

func assertSnaplinkRustTUISharedSession(
	t *testing.T, executable, apiURL, issuer, accessToken, conversationID, intentID string,
	initialAggregateVersion, initialChangeCursor uint64,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Logf("skipping Snaplink JWT Rust TUI sub-check: script PTY launcher is unavailable: %v", err)
		return
	}
	home := t.TempDir()
	placementInput := filepath.Join(home, "placement.json")
	if err := os.WriteFile(placementInput,
		[]byte(devicePlacementPreviewBody(issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant)), 0o600); err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui",
	)
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader(
		"open " + conversationID + "\nsync\nrun-intents\nrun-intents timeline " + intentID + "\nrun-intents submit pending intent from Rust TUI\nsync\nrun-intents\ninventory read\nprompt Prompt from Snaplink-authenticated Rust TUI\nplacement-preview --input " + placementInput + "\nquit\n",
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Snaplink JWT Rust TUI failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Snaplink JWT Rust TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"Shared from client A",
		"prompt sent from client B",
		"Pending Run-intent",
		"No Run was started",
		"Prompt stored. No Run was started.",
		"Prompt from Snaplink-authenticated Rust TUI",
		"Run-intent",
		"Run-intent event",
		"remote device inventory [forge.device-inventory-observation/v1]",
		"device-a / runner-a: approval=approved cordon=clear liveness=online cpu=8 memory=16384 storage=8192 gpu=false gpu_memory=0 runtimes=oci",
		"device-b / runner-b: approval=approved cordon=clear liveness=online cpu=4 memory=8192 storage=4096 gpu=false gpu_memory=0 runtimes=oci",
		"offline placement preview",
		"device-1: matches",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("Snaplink JWT Rust TUI output omitted %q: %q", want, output)
		}
	}
	if !strings.Contains(output, "Synced ") ||
		!strings.Contains(output, " owner-visible changes through cursor ") {
		t.Fatalf("Snaplink JWT Rust TUI did not report a change-feed sync: %q", output)
	}
	if !strings.Contains(output, fmt.Sprintf("owner-visible changes through cursor %d", initialChangeCursor+1)) {
		t.Fatalf("Snaplink JWT Rust TUI did not sync the fresh pending-intent change: cursor=%d output=%q", initialChangeCursor+1, output)
	}
	if !strings.Contains(output, fmt.Sprintf("(version %d)", initialAggregateVersion+1)) {
		t.Fatalf("Snaplink JWT Rust TUI did not render the fresh pending-intent aggregate version: %q", output)
	}
	const freshMarker = `Pending Run-intent "`
	freshStart := strings.Index(output, freshMarker)
	if freshStart < 0 {
		t.Fatalf("Snaplink JWT Rust TUI did not print the fresh pending-intent receipt: %q", output)
	}
	freshIDStart := freshStart + len(freshMarker)
	freshIDEnd := strings.Index(output[freshIDStart:], `" stored.`)
	if freshIDEnd <= 0 {
		t.Fatalf("Snaplink JWT Rust TUI fresh pending-intent receipt was not parseable: %q", output)
	}
	freshID := output[freshIDStart : freshIDStart+freshIDEnd]
	if strings.Count(output, freshID) < 2 || strings.Count(output, intentID) < 2 ||
		!strings.Contains(output, "latest_sequence=1") {
		t.Fatalf("Snaplink JWT Rust TUI did not read back the fresh pending-intent receipt: id=%q output=%q", freshID, output)
	}
}

func snaplinkExecutionProfile(t *testing.T, projectID string) (*executionprofile.Catalog, intentmodel.ServerExecutionProfile) {
	t.Helper()
	profile := intentmodel.ServerExecutionProfile{
		ID: "profile-snaplink-inert-v1", SHA256: sha256.Sum256([]byte("snaplink-inert-execution-profile-v1")),
	}
	catalog, err := executionprofile.New([]executionprofile.Binding{{ProjectID: projectID, Profile: profile}})
	if err != nil {
		t.Fatal(err)
	}
	return catalog, profile
}

func startSnaplinkForgeTestIssuer(t *testing.T) (string, *http.Client, string, string, func()) {
	t.Helper()
	ctx := context.Background()
	ssoHTTP := httptest.NewUnstartedServer(nil)
	issuer := "https://" + ssoHTTP.Listener.Addr().String()
	users := defaultimpl.NewMemoryUserProvider()
	if err := users.CreateOrUpdate(ctx, &sso.User{ID: snaplinkForgeTestUser, Name: "Forge Snaplink Test User"}); err != nil {
		t.Fatal(err)
	}
	clients := defaultimpl.NewMemoryClientStore()
	forgeScopes := []string{"openid", "profile", "forge:conversations:read", "forge:conversations:write", deviceInventoryReadCandidateScope, devicePlacementRegistryCandidateScope, lifecycleRegistryCandidateReadScope}
	for _, clientID := range []string{"forge-console", "forge-cli"} {
		clients.AddSeed(&sso.Client{
			ID: clientID, Name: clientID, TenantID: snaplinkForgeTestTenant, SubjectType: "public",
			AllowedScopes: forgeScopes, AllowedResources: []string{snaplinkForgeTestAudience},
			AllowedAuthenticators: []string{"password"}, TokenStrategy: "jwt", Active: true,
			TokenEndpointAuthMethod: "none", SkipConsent: true,
		})
	}
	password := authenticators.NewPasswordAuthenticator(authenticators.PasswordVerifierFunc(
		func(_ context.Context, username, supplied string) (*sso.AuthResult, error) {
			if username != snaplinkForgeTestUser || supplied != snaplinkForgeTestPassword {
				return nil, fmt.Errorf("bad credentials")
			}
			return &sso.AuthResult{UserID: snaplinkForgeTestUser}, nil
		},
	))
	issuerImpl := defaultimpl.NewEd25519JWTIssuer(
		defaultimpl.WithEd25519Issuer(issuer), defaultimpl.WithEd25519TokenTTL(5*time.Minute),
	)
	snaplink := sso.NewServer(
		sso.WithIssuer(issuer), sso.WithUserProvider(users),
		sso.WithSessionManager(defaultimpl.NewMemorySessionManager()), sso.WithClientStore(clients),
		sso.WithAuthenticator(password), sso.WithTokenIssuer("jwt", issuerImpl),
		sso.WithDefaultTokenStrategy("jwt"),
	)
	ssoHTTP.Config.Handler = snaplink.Handler()
	ssoHTTP.StartTLS()
	client := ssoHTTP.Client()
	tokenA := snaplinkForgeLogin(t, client, issuer, "forge-console")
	tokenB := snaplinkForgeLogin(t, client, issuer, "forge-cli")
	if tokenA == tokenB {
		t.Fatal("independent Snaplink clients returned identical access tokens")
	}
	return issuer, client, tokenA, tokenB, ssoHTTP.Close
}

func assertSnaplinkRustCLIRequestsHaveNoExecutionOrDeviceEffects(
	t *testing.T, requests []recordedConversationRequest, conversationID, intentID, freshIntentID string,
) {
	t.Helper()
	want := []recordedConversationRequest{
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/prompts", query: "limit=128"},
		{method: http.MethodGet, path: conversationChangesPath, query: "after_cursor=0&limit=128"},
		{method: http.MethodPost, path: conversationCollectionPath + "/" + conversationID + "/run-intents"},
		{method: http.MethodPost, path: conversationCollectionPath + "/" + conversationID + "/run-intents"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/runs", query: "limit=25"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/run-intents", query: "limit=25"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/run-intents/" + intentID + "/timeline", query: "after_sequence=0&limit=25"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/run-intents/" + freshIntentID + "/timeline", query: "after_sequence=0&limit=25"},
		{method: http.MethodGet, path: deviceInventoryReadCandidatePath},
		{method: http.MethodPost, path: devicePlacementPreviewPath},
	}
	if len(requests) != len(want) {
		t.Fatalf("Snaplink JWT CLI issued %d HTTP requests; expected only read-only session/Prompt/Run calls: %#v", len(requests), requests)
	}
	for index, request := range requests {
		if request != want[index] {
			t.Fatalf("Snaplink JWT CLI request[%d]=%#v want=%#v (device and Run effect routes must remain untouched)", index, request, want[index])
		}
	}
}

func assertSnaplinkRustTUIRequestsHaveNoExecutionOrDeviceEffects(
	t *testing.T, requests []recordedConversationRequest, conversationID, intentID string,
) {
	t.Helper()
	promptPath := conversationCollectionPath + "/" + conversationID + "/prompts"
	changeQuery := "after_cursor=3&limit=128"
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		changeQuery = "after_cursor=6&limit=128"
	}
	want := []recordedConversationRequest{
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: promptPath, query: "limit=128"},
		{method: http.MethodGet, path: conversationChangesPath, query: "after_cursor=0&limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: promptPath, query: "limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/run-intents", query: "limit=25"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/run-intents/" + intentID + "/timeline", query: "after_sequence=0&limit=25"},
		{method: http.MethodPost, path: conversationCollectionPath + "/" + conversationID + "/run-intents"},
		{method: http.MethodGet, path: promptPath, query: "limit=128"},
		{method: http.MethodGet, path: conversationChangesPath, query: changeQuery},
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: promptPath, query: "limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/run-intents", query: "limit=25"},
		// The post-submit sync refreshes the explicitly opened pending-intent
		// view, and the following run-intents command reads the same page again.
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/run-intents", query: "limit=25"},
		{method: http.MethodGet, path: deviceInventoryReadCandidatePath},
		{method: http.MethodPost, path: promptPath},
		{method: http.MethodGet, path: promptPath, query: "limit=128"},
		{method: http.MethodPost, path: devicePlacementPreviewPath},
	}
	if len(requests) != len(want) {
		t.Fatalf("Snaplink JWT TUI issued %d HTTP requests; expected only shared-session and change-feed calls: %#v", len(requests), requests)
	}
	for index, request := range requests {
		if request != want[index] {
			t.Fatalf("Snaplink JWT TUI request[%d]=%#v want=%#v (device and Run effect routes must remain untouched)", index, request, want[index])
		}
	}
}

func snaplinkCreateProjectConversation(
	t *testing.T, client *http.Client, baseURL, token, projectID string,
) model.Conversation {
	t.Helper()
	response := snaplinkConversationRequest(t, client, baseURL, token, http.MethodPost,
		conversationCollectionPath, "snaplink-execution-conversation",
		fmt.Sprintf(`{"scope":{"kind":"project","id":%q},"title":"Shared from client A"}`, projectID))
	if response.StatusCode != http.StatusCreated {
		t.Fatalf("Snaplink project Conversation create status=%d body=%q", response.StatusCode, readConversationClientBody(t, response))
	}
	var conversation model.Conversation
	if !decodeSnaplinkResponse(t, response, &conversation) || conversation.ID == "" ||
		conversation.Scope != (model.ConversationScope{Kind: "project", ID: projectID}) {
		t.Fatalf("Snaplink project Conversation=%#v", conversation)
	}
	return conversation
}

func snaplinkPreviewExecutionConsent(
	t *testing.T, client *http.Client, baseURL, token, conversationID, projectID string,
	profile intentmodel.ServerExecutionProfile,
) executionConsentPreviewResponse {
	t.Helper()
	response := snaplinkConversationRequest(t, client, baseURL, token, http.MethodGet,
		conversationCollectionPath+"/"+conversationID+"/execution-consents", "", "")
	var preview executionConsentPreviewResponse
	if response.StatusCode != http.StatusOK || !decodeSnaplinkResponse(t, response, &preview) ||
		preview.ConversationID != conversationID || preview.ProjectID != projectID ||
		preview.ProfileID != profile.ID || preview.ProfileSHA256 != hex.EncodeToString(profile.SHA256[:]) {
		t.Fatalf("Snaplink execution consent preview status=%d preview=%#v", response.StatusCode, preview)
	}
	return preview
}

func snaplinkGrantExecutionConsent(
	t *testing.T, client *http.Client, baseURL, token, conversationID string,
	preview executionConsentPreviewResponse,
) executionConsentGrantResponse {
	t.Helper()
	expires := uint64(time.Now().Add(time.Hour).UnixMilli())
	body := executionGrantBody(preview, expires)
	response := snaplinkConversationRequest(t, client, baseURL, token, http.MethodPost,
		conversationCollectionPath+"/"+conversationID+"/execution-consents", "snaplink-execution-consent", body)
	var grant executionConsentGrantResponse
	if response.StatusCode != http.StatusCreated || !decodeSnaplinkResponse(t, response, &grant) ||
		grant.Replayed || grant.Grant.GrantID == "" || grant.Grant.ExpiresAtMS != expires {
		t.Fatalf("Snaplink execution consent grant status=%d grant=%#v", response.StatusCode, grant)
	}
	return grant
}

func snaplinkSubmitPendingIntent(
	t *testing.T, client *http.Client, baseURL, token, conversationID, key string,
	expectedVersion uint64, content, projectID string, profile intentmodel.ServerExecutionProfile,
) intentmodel.PendingRunIntentSubmissionResult {
	t.Helper()
	response := snaplinkConversationRequest(t, client, baseURL, token, http.MethodPost,
		conversationCollectionPath+"/"+conversationID+"/run-intents", key,
		fmt.Sprintf(`{"content":%q,"expected_version":%d}`, content, expectedVersion))
	var result intentmodel.PendingRunIntentSubmissionResult
	if response.StatusCode != http.StatusCreated || !decodeSnaplinkResponse(t, response, &result) || result.Replayed ||
		result.Intent.IntentID == "" || result.Intent.Status != "pending" || result.Intent.ProjectID != projectID ||
		result.Intent.ProfileID != profile.ID || result.Prompt.Content != content || result.InitialEvent.Type != "submitted" {
		t.Fatalf("Snaplink pending intent status=%d result=%#v", response.StatusCode, result)
	}
	return result
}

func snaplinkAssertPendingIntentViews(
	t *testing.T, client *http.Client, baseURL, token, conversationID string,
	first intentmodel.PendingRunIntentSubmissionResult,
) {
	t.Helper()
	path := conversationCollectionPath + "/" + conversationID + "/run-intents"
	response := snaplinkConversationRequest(t, client, baseURL, token, http.MethodGet, path, "", "")
	var page intentmodel.OwnedPendingRunIntentPage
	if response.StatusCode != http.StatusOK || !decodeSnaplinkResponse(t, response, &page) || len(page.Intents) != 1 ||
		page.Intents[0].IntentID != first.Intent.IntentID {
		t.Fatalf("Snaplink pending intent page status=%d page=%#v", response.StatusCode, page)
	}
	timeline := snaplinkConversationRequest(t, client, baseURL, token, http.MethodGet,
		path+"/"+first.Intent.IntentID+"/timeline", "", "")
	var timelinePage intentmodel.OwnedPendingRunIntentTimelinePage
	if timeline.StatusCode != http.StatusOK || !decodeSnaplinkResponse(t, timeline, &timelinePage) || len(timelinePage.Events) != 1 ||
		timelinePage.Events[0].Type != "submitted" {
		t.Fatalf("Snaplink pending intent timeline status=%d page=%#v", timeline.StatusCode, timelinePage)
	}
	runs := snaplinkConversationRequest(t, client, baseURL, token, http.MethodGet,
		conversationCollectionPath+"/"+conversationID+"/runs?limit=25", "", "")
	var runPage struct {
		Runs []json.RawMessage `json:"runs"`
	}
	if runs.StatusCode != http.StatusOK || !decodeSnaplinkResponse(t, runs, &runPage) || len(runPage.Runs) != 0 {
		t.Fatalf("pending intent created a normal Run status=%d runs=%#v", runs.StatusCode, runPage.Runs)
	}
}

func snaplinkAssertFreshPendingIntentReadback(
	t *testing.T, client *http.Client, baseURL, token, conversationID string,
	first intentmodel.PendingRunIntentSubmissionResult,
) {
	t.Helper()
	promptResponse := snaplinkConversationRequest(t, client, baseURL, token, http.MethodGet,
		conversationCollectionPath+"/"+conversationID+"/prompts?limit=128", "", "")
	var prompts model.ConversationPromptPage
	if promptResponse.StatusCode != http.StatusOK || !decodeSnaplinkResponse(t, promptResponse, &prompts) ||
		prompts.ConversationID != conversationID || prompts.HasMore {
		t.Fatalf("fresh Snaplink Prompt readback status=%d page=%#v", promptResponse.StatusCode, prompts)
	}
	promptContents := make(map[string]string, len(prompts.Prompts))
	for _, prompt := range prompts.Prompts {
		if prompt.ConversationID != conversationID || prompt.Role != "user" || prompt.ID == "" {
			t.Fatalf("fresh Snaplink Prompt readback contained invalid prompt=%#v", prompt)
		}
		promptContents[prompt.ID] = prompt.Content
	}
	expectedPendingContents := map[string]uint64{
		"prompt sent from client B":    first.Intent.AggregateVersion,
		"pending intent from Rust CLI": first.Intent.AggregateVersion + 1,
	}
	if _, ok := promptContentsByContent(promptContents)["pending intent from Flutter Console"]; ok {
		expectedPendingContents["pending intent from Flutter Console"] = first.Intent.AggregateVersion + 2
	}
	if _, ok := promptContentsByContent(promptContents)["pending intent from Rust TUI"]; ok {
		tuiVersion := first.Intent.AggregateVersion + 2
		if _, hasFlutter := expectedPendingContents["pending intent from Flutter Console"]; hasFlutter {
			tuiVersion = first.Intent.AggregateVersion + 5
		}
		expectedPendingContents["pending intent from Rust TUI"] = tuiVersion
	}
	expectedPromptCount := len(expectedPendingContents)
	if _, ok := promptContentsByContent(promptContents)["Prompt submitted by Snaplink Flutter API client"]; ok {
		expectedPromptCount++
	}
	if _, ok := promptContentsByContent(promptContents)["Prompt submitted from Snaplink Flutter screen"]; ok {
		expectedPromptCount++
	}
	if _, ok := promptContentsByContent(promptContents)["Prompt from Snaplink-authenticated Rust TUI"]; ok {
		expectedPromptCount++
	}
	if len(prompts.Prompts) != expectedPromptCount {
		t.Fatalf("fresh Snaplink Prompt readback omitted or duplicated cross-client writes: page=%#v expected=%d", prompts, expectedPromptCount)
	}

	path := conversationCollectionPath + "/" + conversationID + "/run-intents"
	response := snaplinkConversationRequest(t, client, baseURL, token, http.MethodGet, path+"?limit=25", "", "")
	var page intentmodel.OwnedPendingRunIntentPage
	if response.StatusCode != http.StatusOK || !decodeSnaplinkResponse(t, response, &page) ||
		page.ConversationID != conversationID || len(page.Intents) != len(expectedPendingContents) || page.HasMore {
		t.Fatalf("fresh Snaplink pending intent page status=%d page=%#v expected=%d", response.StatusCode, page, len(expectedPendingContents))
	}
	seenPendingContents := make(map[string]bool, len(page.Intents))
	for _, intent := range page.Intents {
		content, ok := promptContents[intent.PromptID]
		version, expected := expectedPendingContents[content]
		if !ok || !expected || seenPendingContents[content] || intent.ConversationID != conversationID ||
			intent.Status != "pending" || intent.IntentID == "" || intent.LatestSequence != 1 ||
			intent.AggregateVersion != version {
			t.Fatalf("fresh Snaplink pending intent binding/version drifted: intent=%#v content=%q page=%#v", intent, content, page)
		}
		seenPendingContents[content] = true
		timelineResponse := snaplinkConversationRequest(t, client, baseURL, token, http.MethodGet,
			path+"/"+intent.IntentID+"/timeline", "", "")
		rawTimeline := readConversationClientBody(t, timelineResponse)
		if timelineResponse.StatusCode != http.StatusOK || strings.Contains(rawTimeline, content) {
			t.Fatalf("fresh Snaplink pending timeline leaked Prompt content status=%d body=%q", timelineResponse.StatusCode, rawTimeline)
		}
		var timelinePage intentmodel.OwnedPendingRunIntentTimelinePage
		if err := json.Unmarshal([]byte(rawTimeline), &timelinePage); err != nil ||
			timelinePage.ConversationID != conversationID || timelinePage.IntentID != intent.IntentID ||
			timelinePage.AfterSequence != 0 || timelinePage.ScannedThroughSequence != 1 ||
			timelinePage.HasMore || len(timelinePage.Events) != 1 || timelinePage.Events[0].Sequence != 1 ||
			timelinePage.Events[0].Type != "submitted" {
			t.Fatalf("fresh Snaplink pending timeline=%#v body=%q decode=%v", timelinePage, rawTimeline, err)
		}
	}
	if len(seenPendingContents) != len(expectedPendingContents) {
		t.Fatalf("fresh Snaplink pending readback omitted a receipt: page=%#v seen=%#v", page, seenPendingContents)
	}

	runs := snaplinkConversationRequest(t, client, baseURL, token, http.MethodGet,
		conversationCollectionPath+"/"+conversationID+"/runs?limit=25", "", "")
	var runPage struct {
		Runs []json.RawMessage `json:"runs"`
	}
	if runs.StatusCode != http.StatusOK || !decodeSnaplinkResponse(t, runs, &runPage) || len(runPage.Runs) != 0 {
		t.Fatalf("fresh pending readback created a normal Run status=%d runs=%#v", runs.StatusCode, runPage.Runs)
	}
}

func promptContentsByContent(prompts map[string]string) map[string]struct{} {
	contents := make(map[string]struct{}, len(prompts))
	for _, content := range prompts {
		contents[content] = struct{}{}
	}
	return contents
}
