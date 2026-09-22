package appserver

import (
	"context"
	"encoding/json"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"time"
)

// runForgeMobileSharedSessionE2EWithToken drives the host-side Flutter
// lifecycle harness. The harness uses the same ForgeCredentialStore path as
// Android/iOS, with an injected secure-store backend because this environment
// does not provide a physical-device instrumentation runner.
func runForgeMobileSharedSessionE2EWithToken(
	t *testing.T,
	platform, prompt, idempotencyKey, apiURL, token, rotatedToken, conversationID string,
	expectedVersion, afterCursor uint64,
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
		t.Fatalf("Flutter is required for the host-side native lifecycle E2E: %v", err)
	}
	inputJSON, err := json.Marshal(struct {
		Platform           string `json:"platform"`
		APIURL             string `json:"api_url"`
		AccessToken        string `json:"access_token"`
		RotatedAccessToken string `json:"rotated_access_token"`
		ConversationID     string `json:"conversation_id"`
		ExpectedVersion    uint64 `json:"expected_version"`
		AfterCursor        uint64 `json:"after_cursor"`
		Prompt             string `json:"prompt"`
		IdempotencyKey     string `json:"idempotency_key"`
	}{
		Platform: platform, APIURL: apiURL, AccessToken: token,
		RotatedAccessToken: rotatedToken,
		ConversationID:     conversationID, ExpectedVersion: expectedVersion,
		AfterCursor: afterCursor, Prompt: prompt, IdempotencyKey: idempotencyKey,
	})
	if err != nil {
		t.Fatalf("encode host-side native lifecycle input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "mobile-shared-session-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private host-side native lifecycle input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub", "test/forge_mobile_shared_session_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_MOBILE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter host-side native lifecycle E2E failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Flutter host-side native lifecycle E2E output exceeded the size limit")
	}
}

func readOwnedConversationAggregateVersion(
	t *testing.T,
	client *http.Client,
	baseURL, token, conversationID string,
) uint64 {
	t.Helper()
	response := doConversationClientRequest(
		t, client, baseURL, token, http.MethodGet,
		conversationCollectionPath+"/"+conversationID, "", "", "",
	)
	if response.StatusCode != http.StatusOK {
		t.Fatalf("read Conversation detail status=%d body=%q",
			response.StatusCode, readConversationClientBody(t, response))
	}
	var entry model.OwnedConversationEntry
	if err := json.NewDecoder(response.Body).Decode(&entry); err != nil {
		_ = response.Body.Close()
		t.Fatalf("decode Conversation detail: %v", err)
	}
	_ = response.Body.Close()
	if entry.Conversation.ID != conversationID || entry.AggregateVersion == 0 {
		t.Fatalf("invalid Conversation detail=%#v", entry)
	}
	return entry.AggregateVersion
}

func readOwnedConversationCursor(
	t *testing.T,
	client *http.Client,
	baseURL, token string,
) uint64 {
	t.Helper()
	var after uint64
	for page := 0; page < 32; page++ {
		response := doConversationClientRequest(
			t, client, baseURL, token, http.MethodGet,
			conversationChangesPath+"?after_cursor="+strconv.FormatUint(after, 10)+"&limit=128",
			"", "", "",
		)
		if response.StatusCode != http.StatusOK {
			t.Fatalf("read owner Conversation cursor status=%d body=%q",
				response.StatusCode, readConversationClientBody(t, response))
		}
		var changes model.OwnedConversationChangePage
		if err := json.NewDecoder(response.Body).Decode(&changes); err != nil {
			_ = response.Body.Close()
			t.Fatalf("decode owner Conversation cursor: %v", err)
		}
		_ = response.Body.Close()
		if changes.AfterCursor != after || changes.ScannedThroughCursor < after {
			t.Fatalf("owner cursor did not make valid progress: %#v", changes)
		}
		if !changes.HasMore {
			return changes.ScannedThroughCursor
		}
		if changes.ScannedThroughCursor == after {
			t.Fatalf("owner cursor made no progress with more pages: %#v", changes)
		}
		after = changes.ScannedThroughCursor
	}
	t.Fatal("owner Conversation cursor exceeded bounded pages")
	return 0
}

func assertForgeMobileSharedSessionRequestsArePromptOnly(
	t *testing.T,
	requests []recordedConversationRequest,
	conversationID string,
	afterCursor uint64,
) {
	t.Helper()
	promptPath := conversationCollectionPath + "/" + conversationID + "/prompts"
	want := []recordedConversationRequest{
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=50"},
		{method: http.MethodGet, path: conversationChangesPath, query: "after_cursor=" + strconv.FormatUint(afterCursor, 10) + "&limit=128"},
		{method: http.MethodGet, path: promptPath, query: "limit=100"},
		{method: http.MethodPost, path: promptPath},
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=50"},
		{method: http.MethodPost, path: promptPath},
		{method: http.MethodGet, path: promptPath, query: "limit=100"},
		{method: http.MethodGet, path: conversationChangesPath, query: "after_cursor=" + strconv.FormatUint(afterCursor, 10) + "&limit=128"},
	}
	if len(requests) != len(want) {
		t.Fatalf("host-side native lifecycle issued %d requests; want exactly %d prompt/session calls: %#v",
			len(requests), len(want), requests)
	}
	for index, request := range requests {
		if request != want[index] {
			t.Fatalf("host-side native lifecycle request[%d]=%#v want=%#v (devices, Run-intents, placement, and dispatch must remain untouched)",
				index, request, want[index])
		}
		for _, forbidden := range []string{"/devices", "/run-intents", "placement", "dispatch"} {
			if strings.Contains(request.path, forbidden) {
				t.Fatalf("host-side native lifecycle touched a forbidden route: %#v", request)
			}
		}
	}
}

func assertForgeMobileSharedSessionVisibleToGoClient(
	t *testing.T,
	client *http.Client,
	baseURL, token, conversationID, expectedPrompt string,
	expectedVersion uint64,
) {
	t.Helper()
	entryVersion := readOwnedConversationAggregateVersion(
		t, client, baseURL, token, conversationID,
	)
	if entryVersion != expectedVersion+1 {
		t.Fatalf("host-side native lifecycle aggregate version=%d want=%d",
			entryVersion, expectedVersion+1)
	}
	promptPath := conversationCollectionPath + "/" + conversationID + "/prompts"
	response := doConversationClientRequest(
		t, client, baseURL, token, http.MethodGet, promptPath+"?limit=100", "", "", "",
	)
	if response.StatusCode != http.StatusOK {
		t.Fatalf("read host-side native Prompt history status=%d body=%q",
			response.StatusCode, readConversationClientBody(t, response))
	}
	var history model.ConversationPromptPage
	if err := json.NewDecoder(response.Body).Decode(&history); err != nil {
		_ = response.Body.Close()
		t.Fatalf("decode host-side native Prompt history: %v", err)
	}
	_ = response.Body.Close()
	matching := 0
	for _, prompt := range history.Prompts {
		if prompt.Content == expectedPrompt {
			matching++
		}
	}
	if matching != 1 {
		t.Fatalf("host-side native lifecycle Prompt count=%d history=%#v", matching, history)
	}
}
