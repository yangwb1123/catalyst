package appserver

import (
	"encoding/json"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/runtimebridge"
)

// TestFlutterPromptBytesReachRustCLIAndTUIWhenConfigured proves that the
// authenticated Flutter API/native path and the Rust CLI/TUI consume the same
// owner-scoped Prompt bytes. It deliberately stops at shared-session storage:
// the Prompt does not create a Run or authorize any device work.
func TestFlutterPromptBytesReachRustCLIAndTUIWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CONSOLE_E2E") != "1" {
		t.Skip("set FORGE_CONSOLE_E2E=1 for Flutter-to-Rust Prompt byte integration")
	}
	configuredExecutable := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredExecutable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for Flutter-to-Rust Prompt byte integration")
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
		Timeout: 5 * time.Second,
	})
	if err != nil {
		t.Fatal(err)
	}
	identity, authenticator := newMultiPrincipalConversationTestIdentity(t)
	t.Cleanup(authenticator.Close)
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
	server.Start()
	t.Cleanup(server.Close)

	const scopes = "forge:conversations:read forge:conversations:write"
	token := tokenForIndependentClient(identity, scopes, "flutter-byte-client")
	client := &http.Client{Timeout: 20 * time.Second}
	created := createSharedConversationAsClientA(t, client, server.URL, token)
	appendPromptAsClientB(t, client, server.URL, token,
		conversationCollectionPath+"/"+created.ID+"/prompts")

	const (
		apiPrompt    = "  Flutter API byte probe\n"
		widgetPrompt = "  Flutter widget byte probe\n"
	)
	runForgeConsoleLiveAPITestWithToken(
		t, recorder, server.URL, token, created.ID, 2,
		apiPrompt, widgetPrompt, "prompt sent from client B", 2,
		"flutter-byte-probe",
		devicePlacementPreviewBody(identity.issuer, "account-42", "tenant-slate"),
		"", false,
	)

	historyOutput, historyStderr, err := runForgeRuntimeCLI(t, executable, server.URL, token, t.TempDir(),
		"--json", "remote", "prompts", "list", created.ID)
	if err != nil {
		t.Fatalf("Rust CLI Prompt history read failed: stderr=%q stdout=%q err=%v", historyStderr, historyOutput, err)
	}
	var history model.ConversationPromptPage
	if err := json.Unmarshal([]byte(historyOutput), &history); err != nil {
		t.Fatalf("decode Rust CLI Prompt history: %v stdout=%q", err, historyOutput)
	}
	if history.ConversationID != created.ID || len(history.Prompts) != 3 ||
		!promptContentIsPresent(history.Prompts, "prompt sent from client B") ||
		!promptContentIsPresent(history.Prompts, apiPrompt) ||
		!promptContentIsPresent(history.Prompts, widgetPrompt) {
		t.Fatalf("Rust CLI did not preserve Flutter Prompt bytes: %#v stdout=%q", history, historyOutput)
	}

	runForgeRuntimeTUIReadPromptBytes(t, executable, server.URL, token, created.ID, apiPrompt, widgetPrompt)
}

func runForgeRuntimeTUIReadPromptBytes(
	t *testing.T,
	executable, apiURL, accessToken, conversationID string,
	expected ...string,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("the Prompt byte TUI integration requires a PTY launcher named script: %v", err)
	}
	home := t.TempDir()
	command := exec.Command(ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("open " + conversationID + "\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Forge Prompt byte TUI failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("Forge Prompt byte TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, content := range expected {
		encoded, err := json.Marshal(content)
		if err != nil {
			t.Fatalf("encode expected TUI Prompt bytes: %v", err)
		}
		if !strings.Contains(output, string(encoded)) {
			t.Fatalf("Forge TUI Prompt history omitted exact JSON Prompt %q: %q", string(encoded), output)
		}
	}
}
