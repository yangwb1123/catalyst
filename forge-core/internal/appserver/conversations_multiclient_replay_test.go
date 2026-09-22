package appserver

import (
	"encoding/json"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"net/http"
	"testing"
	"time"
)

func assertForgeRuntimeCLIReplaysConsolePrompt(
	t *testing.T,
	executable, apiURL string,
	identity *conversationTestIdentity,
	conversationID string,
) {
	t.Helper()
	const scopes = "forge:conversations:read forge:conversations:write"
	const promptContent = "Prompt submitted by Flutter Console API client"
	const idempotencyKey = "console-client-prompt"
	output, stderr, err := runForgeRuntimeCLI(
		t, executable, apiURL,
		tokenForIndependentClient(identity, scopes, "cli-console-replay"),
		t.TempDir(),
		"--json", "--idempotency-key", idempotencyKey,
		"remote", "prompts", "add", conversationID, "--expected-version", "2", promptContent,
	)
	if err != nil {
		t.Fatalf("Rust CLI Flutter Prompt replay failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var result struct {
		Prompt           model.ConversationPrompt `json:"prompt"`
		AggregateVersion uint64                   `json:"aggregate_version"`
		Replayed         bool                     `json:"replayed"`
	}
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("decode Rust CLI Flutter Prompt replay: %v; stdout=%q", err, output)
	}
	if result.Prompt.ConversationID != conversationID || result.Prompt.Content != promptContent ||
		result.AggregateVersion != 3 || !result.Replayed {
		t.Fatalf("Rust CLI Flutter Prompt replay result=%#v stdout=%q", result, output)
	}

	client := &http.Client{Timeout: 20 * time.Second}
	token := tokenForIndependentClient(identity, scopes, "cli-console-replay-history")
	promptPath := conversationCollectionPath + "/" + conversationID + "/prompts"
	historyResponse := doConversationClientRequest(t, client, apiURL, token,
		http.MethodGet, promptPath+"?limit=100", "", "", "")
	if historyResponse.StatusCode != http.StatusOK {
		t.Fatalf("Rust CLI replay history status=%d body=%q", historyResponse.StatusCode, readConversationClientBody(t, historyResponse))
	}
	var history model.ConversationPromptPage
	if err := json.NewDecoder(historyResponse.Body).Decode(&history); err != nil {
		_ = historyResponse.Body.Close()
		t.Fatalf("decode Rust CLI replay history: %v", err)
	}
	_ = historyResponse.Body.Close()
	matching := make([]model.ConversationPrompt, 0, 1)
	for _, prompt := range history.Prompts {
		if prompt.Content == promptContent {
			matching = append(matching, prompt)
		}
	}
	if len(matching) != 1 || matching[0].ID != result.Prompt.ID || len(history.Prompts) != 3 {
		t.Fatalf("Rust CLI replay duplicated or changed Flutter Prompt: history=%#v result=%#v", history, result)
	}

	changesResponse := doConversationClientRequest(t, client, apiURL, token,
		http.MethodGet, conversationChangesPath+"?after_cursor=2&limit=128", "", "", "")
	if changesResponse.StatusCode != http.StatusOK {
		t.Fatalf("Rust CLI replay change feed status=%d body=%q", changesResponse.StatusCode, readConversationClientBody(t, changesResponse))
	}
	var changes model.OwnedConversationChangePage
	if err := json.NewDecoder(changesResponse.Body).Decode(&changes); err != nil {
		_ = changesResponse.Body.Close()
		t.Fatalf("decode Rust CLI replay change feed: %v", err)
	}
	_ = changesResponse.Body.Close()
	if changes.AfterCursor != 2 || changes.ScannedThroughCursor != 4 || changes.HasMore ||
		len(changes.Changes) != 2 || changes.Changes[0].AggregateVersion != 3 ||
		changes.Changes[1].AggregateVersion != 4 {
		t.Fatalf("Rust CLI replay emitted an extra change: %#v", changes)
	}
}

func assertForgeRuntimeCLIReplayRequestsArePromptOnly(
	t *testing.T,
	requests []recordedConversationRequest,
	conversationID string,
) {
	t.Helper()
	promptPath := conversationCollectionPath + "/" + conversationID + "/prompts"
	want := []recordedConversationRequest{
		{method: http.MethodPost, path: promptPath},
		{method: http.MethodGet, path: promptPath, query: "limit=100"},
		{method: http.MethodGet, path: conversationChangesPath, query: "after_cursor=2&limit=128"},
	}
	if len(requests) != len(want) {
		t.Fatalf("Rust CLI replay issued %d requests; want exactly %d prompt/read requests: %#v", len(requests), len(want), requests)
	}
	for index, request := range requests {
		if request != want[index] {
			t.Fatalf("Rust CLI replay request[%d]=%#v want=%#v", index, request, want[index])
		}
	}
}
