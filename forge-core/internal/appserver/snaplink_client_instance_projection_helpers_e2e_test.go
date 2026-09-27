package appserver

import (
	"context"
	"encoding/json"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// runForgeConsoleClientInstanceSessionProjectionE2EWithToken drives the real
// shared Web/App/Mobile Flutter Sessions Gate against the same authenticated
// candidate mux and real owner Conversations used by this test. The candidate
// is supplied only through this opt-in harness; normal Gate construction
// remains request-free and the production route remains closed.
func runForgeConsoleClientInstanceSessionProjectionE2EWithToken(
	t *testing.T,
	apiURL, token string,
	owner deviceplacement.Owner,
	instanceID string,
	visible, hidden model.Conversation,
	prompt string,
	recorders ...*conversationHTTPRecorder,
) {
	t.Helper()
	var recorder *conversationHTTPRecorder
	if len(recorders) > 0 {
		recorder = recorders[0]
	}
	requestStart := 0
	if recorder != nil {
		requestStart = len(recorder.snapshot())
	}
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
		t.Fatalf("Flutter is required for client-instance session projection E2E: %v", err)
	}
	inputJSON, err := json.Marshal(struct {
		APIURL              string                `json:"api_url"`
		AccessToken         string                `json:"access_token"`
		Owner               deviceplacement.Owner `json:"owner"`
		InstanceID          string                `json:"instance_id"`
		VisibleConversation string                `json:"visible_conversation_id"`
		HiddenConversation  string                `json:"hidden_conversation_id"`
		VisibleTitle        string                `json:"visible_title"`
		HiddenTitle         string                `json:"hidden_title"`
		Prompt              string                `json:"prompt"`
	}{
		APIURL: apiURL, AccessToken: token, Owner: owner,
		InstanceID:          instanceID,
		VisibleConversation: visible.ID,
		HiddenConversation:  hidden.ID,
		VisibleTitle:        visible.Title,
		HiddenTitle:         hidden.Title,
		Prompt:              prompt,
	})
	if err != nil {
		t.Fatalf("encode Flutter client-instance session projection input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "client-instance-session-projection-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter client-instance projection input: %v", err)
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
		"test/forge_client_instance_session_projection_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_CLIENT_INSTANCE_SESSION_PROJECTION_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter client-instance session projection E2E failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Flutter client-instance session projection output exceeded the size limit")
	}
	if recorder != nil {
		assertConsoleClientInstanceProjectionPromptRequests(
			t, recorder.snapshot()[requestStart:], visible.ID, hidden.ID,
		)
	}
}

func assertConsoleClientInstanceProjectionPromptRequests(
	t *testing.T,
	requests []recordedConversationRequest,
	visibleConversationID, hiddenConversationID string,
) {
	t.Helper()
	visiblePromptPath := conversationCollectionPath + "/" + visibleConversationID + "/prompts"
	hiddenPromptPath := conversationCollectionPath + "/" + hiddenConversationID + "/prompts"
	sessionReadIndex, resourceReadIndex, inventoryReadIndex, visiblePromptPostIndex := -1, -1, -1, -1
	for index, request := range requests {
		switch {
		case request.method == http.MethodGet && request.path == clientInstanceSessionViewCandidatePath:
			sessionReadIndex = index
		case request.method == http.MethodGet && request.path == clientInstanceResourceViewCandidatePath:
			resourceReadIndex = index
		case request.method == http.MethodGet && request.path == deviceInventoryReadCandidateV2Path:
			inventoryReadIndex = index
		case request.method == http.MethodPost && request.path == visiblePromptPath:
			visiblePromptPostIndex = index
		case request.method == http.MethodPost && request.path == hiddenPromptPath:
			t.Fatalf("Console hidden instance Prompt POST escaped the local filter: %#v", requests)
		}
	}
	if sessionReadIndex < 0 || resourceReadIndex < 0 || inventoryReadIndex < 0 || visiblePromptPostIndex < 0 {
		t.Fatalf(
			"Console Prompt journey did not converge client-instance and inventory/resource views before the visible POST: %#v",
			requests,
		)
	}
	visiblePromptPosts := 0
	for _, request := range requests {
		if request.method == http.MethodPost && request.path == visiblePromptPath {
			visiblePromptPosts++
		}
	}
	if visiblePromptPosts != 1 {
		t.Fatalf(
			"Console Prompt journey sent %d visible Prompt POSTs; want exactly one: %#v",
			visiblePromptPosts, requests,
		)
	}
	if sessionReadIndex > visiblePromptPostIndex || resourceReadIndex > visiblePromptPostIndex || inventoryReadIndex > visiblePromptPostIndex {
		t.Fatalf(
			"Console Prompt POST preceded a required freshness observation: %#v",
			requests,
		)
	}
}

func runForgeRuntimeClientInstanceProjectionTUI(
	t *testing.T,
	executable, apiURL, accessToken, conversationID string,
) string {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("client-instance projection TUI E2E requires script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader(
		"client-instances show-converged\n" +
			"inventory show-converged\n" +
			"instance client-tui-001\n" +
			"open " + conversationID + "\n" +
			"prompt Prompt submitted from client-instance TUI\n" +
			"quit\n",
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("client-instance projection TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("client-instance projection TUI output exceeded the size limit")
	}
	return stdoutBuffer.String()
}

// assertRuntimeTUIInstancePromptFreshness proves that the authenticated TUI
// prompt path refreshes the owner-bound inventory/resource pair immediately
// before its storage-only Prompt POST. The client-instance and inventory
// observations are both explicit display candidates; neither grants device or
// execution authority.
func assertRuntimeTUIInstancePromptFreshness(
	t *testing.T,
	requests []recordedConversationRequest,
	conversationID string,
) {
	t.Helper()
	promptPath := conversationCollectionPath + "/" + conversationID + "/prompts"
	want := []recordedConversationRequest{
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: clientInstanceSessionViewCandidatePath},
		{method: http.MethodGet, path: clientInstanceResourceViewCandidatePath},
		{method: http.MethodGet, path: deviceInventoryReadCandidateV2Path},
		{method: http.MethodGet, path: clientInstanceResourceViewCandidatePath},
		{method: http.MethodGet, path: promptPath, query: "limit=128"},
		{method: http.MethodGet, path: deviceInventoryReadCandidateV2Path},
		{method: http.MethodGet, path: clientInstanceResourceViewCandidatePath},
		{method: http.MethodPost, path: promptPath},
		{method: http.MethodGet, path: promptPath, query: "limit=128"},
	}
	if len(requests) != len(want) {
		t.Fatalf("TUI instance Prompt freshness issued %d requests; want %#v got %#v", len(requests), want, requests)
	}
	for index, request := range requests {
		if request != want[index] {
			t.Fatalf("TUI instance Prompt freshness request[%d]=%#v want=%#v", index, request, want[index])
		}
	}
}

func assertClientInstanceProjectionRequestsHaveNoExecutionOrDeviceEffects(
	t *testing.T,
	requests []recordedConversationRequest,
) {
	t.Helper()
	for _, request := range requests {
		path := strings.ToLower(request.path)
		forbidden := []string{
			"/api/v1/device-placement",
			"/api/v1/reservation",
			"/api/v1/dispatch",
			"/api/v1/runner",
			"/api/v1/execution",
		}
		for _, fragment := range forbidden {
			if strings.Contains(path, fragment) {
				t.Fatalf("client-instance projection issued forbidden device/execution request: %#v", request)
			}
		}
		if strings.Contains(path, "/api/v1/devices") && request.path != deviceInventoryReadCandidateV2Path {
			t.Fatalf("client-instance projection issued an unexpected device request: %#v", request)
		}
		if request.path == conversationCollectionPath && strings.Contains(request.query, "instance") {
			t.Fatalf("client-instance projection leaked an instance query into the authenticated session list: %#v", request)
		}
	}
}
