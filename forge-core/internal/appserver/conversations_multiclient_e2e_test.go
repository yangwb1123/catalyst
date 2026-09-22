package appserver

import (
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"

	"forgeos/forge-core/internal/runtimebridge"
)

func TestIndependentClientsShareOwnedConversationAndPrompts(t *testing.T) {
	configuredExecutable := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredExecutable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for two-client HTTP to Rust Hub integration")
	}
	executable, err := exec.LookPath(configuredExecutable)
	if err != nil {
		t.Fatalf("FORGE_RUNTIME_BIN must name a built forge-runtime executable available to the test: %v", err)
	}
	executable, err = filepath.Abs(executable)
	if err != nil {
		t.Fatalf("resolve forge-runtime executable path: %v", err)
	}
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	if err := os.Mkdir(runtimeState, 0o700); err != nil {
		t.Fatal(err)
	}
	initializeRuntimeHubForIntegration(t, executable, runtimeState)
	appState := filepath.Join(t.TempDir(), "app-server-state")
	if err := os.Mkdir(appState, 0o700); err != nil {
		t.Fatal(err)
	}
	bridge, err := runtimebridge.New(runtimebridge.Config{
		Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState,
	})
	if err != nil {
		t.Fatal(err)
	}
	identity, authenticator := newMultiPrincipalConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(bridge, nil))
	recorder := &conversationHTTPRecorder{}
	server := httptest.NewUnstartedServer(http.NotFoundHandler())
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test", Commit: "test"},
		server.Listener.Addr().String(), maxInFlightRequests, sessions,
	)
	if err != nil {
		server.Close()
		t.Fatal(err)
	}
	server.Config.Handler = recorder.wrap(routes)
	if os.Getenv("FORGE_BROWSER_E2E") == "1" {
		webBuildDir := os.Getenv("FORGE_WEB_BUILD_DIR")
		if webBuildDir == "" {
			server.Close()
			t.Fatal("FORGE_WEB_BUILD_DIR is required when FORGE_BROWSER_E2E=1")
		}
		server.Config.Handler = serveForgeConsoleWebAssets(webBuildDir, recorder.wrap(routes))
	}
	server.Start()
	t.Cleanup(server.Close)

	clientA := &http.Client{Timeout: 20 * time.Second}
	clientB := &http.Client{Timeout: 20 * time.Second}
	tokenA := tokenForIndependentClient(identity, "forge:conversations:read forge:conversations:write", "client-a")
	tokenB := tokenForIndependentClient(identity, "forge:conversations:read forge:conversations:write", "client-b")
	if tokenA == tokenB {
		t.Fatal("independent clients must use distinct access tokens")
	}
	created := createSharedConversationAsClientA(t, clientA, server.URL, tokenA)
	assertConversationVisibleToClientB(t, clientB, server.URL, tokenB, created)
	promptPath := conversationCollectionPath + "/" + created.ID + "/prompts"
	appendPromptAsClientB(t, clientB, server.URL, tokenB, promptPath)
	assertPromptVisibleToClientA(t, clientA, server.URL, tokenA, promptPath, created.ID)

	cliChangesAfter := uint64(2)
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		firstConsoleRequest := len(recorder.snapshot())
		runForgeConsoleLiveAPITest(t, recorder, server.URL, identity, created.ID)
		consoleRequests := recorder.snapshot()[firstConsoleRequest:]
		assertForgeConsoleAPIAndWidgetRequestsHaveNoExecutionOrDeviceEffects(t, consoleRequests, created.ID, "", false)
		firstReplayRequest := len(recorder.snapshot())
		assertForgeRuntimeCLIReplaysConsolePrompt(t, executable, server.URL, identity, created.ID)
		replayRequests := recorder.snapshot()[firstReplayRequest:]
		assertForgeRuntimeCLIReplayRequestsArePromptOnly(t, replayRequests, created.ID)
		assertForgeConsolePromptVisibleToGoClient(t, clientA, server.URL, tokenA, promptPath, created.ID)
		cliChangesAfter = 4
	}
	firstCLIRequest := len(recorder.snapshot())
	cliConversation := assertForgeRuntimeCLIClientsShareOwnedConversationAndPrompts(t, executable, server.URL, identity, cliChangesAfter)
	assertForgeRuntimeCLIRequestsHaveNoExecutionOrDeviceEffects(
		t, recorder.snapshot()[firstCLIRequest:], cliChangesAfter,
	)
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		firstTUIRequest := len(recorder.snapshot())
		runForgeRuntimeTUI(t, executable, server.URL, tokenForIndependentClient(
			identity, "forge:conversations:read forge:conversations:write", "tui-client",
		), created.ID)
		tuiRequests := recorder.snapshot()[firstTUIRequest:]
		assertForgeRuntimeTUIRequestsHaveNoExecutionOrDeviceEffects(t, tuiRequests, created.ID)
		assertForgeTUIWriteVisibleToGoClient(t, clientA, server.URL, tokenA, promptPath, created.ID)
	}
	if os.Getenv("FORGE_BROWSER_E2E") == "1" {
		firstBrowserRequest := len(recorder.snapshot())
		runForgeConsoleBrowserE2E(t, server.URL, identity, created.ID)
		browserRequests := recorder.snapshot()[firstBrowserRequest:]
		assertForgeConsoleBrowserRequestsHaveNoExecutionOrDeviceEffects(t, browserRequests, created.ID)
		assertForgeConsoleBrowserChangeCursorResumes(t, browserRequests)
		assertForgeBrowserWriteVisibleToGoClient(t, clientA, server.URL, tokenA, promptPath, created.ID)
	}
	assertNoPendingRunIntentAfterPrompt(t, bridge, identity, cliConversation.ID)
	assertNoPendingRunIntentAfterPrompt(t, bridge, identity, created.ID)
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" &&
		os.Getenv("FORGE_MOBILE_SHARED_SESSION_E2E") != "0" {
		// Run the host-side native lifecycle after the shared-client assertions
		// have completed. It uses the current owner aggregate as its CAS base,
		// then proves a cold-start replay without changing those earlier client
		// request-count contracts.
		for _, native := range []struct {
			platform       string
			clientID       string
			prompt         string
			idempotencyKey string
		}{
			{
				platform:       "android-host",
				clientID:       "android-host-native-client",
				prompt:         "Prompt submitted by Android host-side native lifecycle",
				idempotencyKey: "host-native-android-lifecycle-prompt",
			},
			{
				platform:       "ios-host",
				clientID:       "ios-host-native-client",
				prompt:         "Prompt submitted by iOS host-side native lifecycle",
				idempotencyKey: "host-native-ios-lifecycle-prompt",
			},
		} {
			nativeToken := tokenForPrincipalWithTTL(
				identity, "account-42",
				"forge:conversations:read forge:conversations:write",
				native.clientID, 15*time.Minute,
			)
			rotatedNativeToken := tokenForPrincipalWithTTL(
				identity, "account-42",
				"forge:conversations:read forge:conversations:write",
				native.clientID+"-rotated", 15*time.Minute,
			)
			nativeClient := &http.Client{Timeout: 20 * time.Second}
			nativeVersion := readOwnedConversationAggregateVersion(
				t, nativeClient, server.URL, nativeToken, created.ID,
			)
			nativeAfterCursor := readOwnedConversationCursor(
				t, nativeClient, server.URL, nativeToken,
			)
			firstNativeRequest := len(recorder.snapshot())
			runForgeMobileSharedSessionE2EWithToken(
				t, native.platform, native.prompt, native.idempotencyKey,
				server.URL, nativeToken, rotatedNativeToken, created.ID,
				nativeVersion, nativeAfterCursor,
			)
			nativeRequests := recorder.snapshot()[firstNativeRequest:]
			assertForgeMobileSharedSessionRequestsArePromptOnly(
				t, nativeRequests, created.ID, nativeAfterCursor,
			)
			assertForgeMobileSharedSessionVisibleToGoClient(
				t, nativeClient, server.URL, nativeToken, created.ID,
				native.prompt, nativeVersion,
			)
			assertNoPendingRunIntentAfterPrompt(t, bridge, identity, created.ID)
		}
	}
	assertForgeRuntimeCLIListsOwnedConversationsAcrossPages(
		t, executable, server.URL, identity, bridge, recorder,
	)
}
