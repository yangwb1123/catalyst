package appserver

// This opt-in integration test closes the P1 concurrency evidence gap: two
// authenticated clients race one owner-scoped Prompt CAS, exactly one append
// commits, and the other receives a structured conflict without a duplicate
// Prompt or Run.  It exercises the real Go HTTP -> Rust Hub boundary.

import (
	"bytes"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"sync"
	"testing"

	"forgeos/forge-core/internal/runtimebridge"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestSnaplinkAuthenticatedConcurrentPromptCASWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CONCURRENT_PROMPT_CAS_E2E") != "1" {
		t.Skip("set FORGE_CONCURRENT_PROMPT_CAS_E2E=1 for concurrent Prompt CAS E2E")
	}
	executable := os.Getenv("FORGE_RUNTIME_BIN")
	if executable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for concurrent Prompt CAS E2E")
	}
	executable, err := exec.LookPath(executable)
	if err != nil {
		t.Fatalf("FORGE_RUNTIME_BIN must name a built forge-runtime executable: %v", err)
	}
	executable, err = filepath.Abs(executable)
	if err != nil {
		t.Fatalf("resolve forge-runtime executable: %v", err)
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
	server := httptest.NewServer(authenticator.Handler(newConversationRoutes(bridge)))
	t.Cleanup(server.Close)
	client := server.Client()
	const scopes = "forge:conversations:read forge:conversations:write"
	creatorToken := tokenForIndependentClient(identity, scopes, "concurrent-cas-creator")
	conversation := createSharedConversationAsClientA(t, client, server.URL, creatorToken)
	promptPath := conversationCollectionPath + "/" + conversation.ID + "/prompts"

	type result struct {
		name        string
		status      int
		body        []byte
		idempotency string
	}
	results := make(chan result, 2)
	start := make(chan struct{})
	var workers sync.WaitGroup
	for _, candidate := range []struct {
		name, clientID, key, content string
	}{
		{name: "client-a", clientID: "concurrent-cas-a", key: "concurrent-cas-a", content: "race from client A"},
		{name: "client-b", clientID: "concurrent-cas-b", key: "concurrent-cas-b", content: "race from client B"},
	} {
		candidate := candidate
		workers.Add(1)
		go func() {
			defer workers.Done()
			<-start
			token := tokenForIndependentClient(identity, scopes, candidate.clientID)
			request, requestErr := http.NewRequest(http.MethodPost, server.URL+promptPath,
				bytes.NewBufferString(`{"content":"`+candidate.content+`","expected_version":1}`))
			if requestErr != nil {
				results <- result{name: candidate.name, idempotency: candidate.key, body: []byte(requestErr.Error())}
				return
			}
			request.Header.Set("Authorization", "Bearer "+token)
			request.Header.Set("Content-Type", "application/json")
			request.Header.Set("Idempotency-Key", candidate.key)
			response, requestErr := client.Do(request)
			if requestErr != nil {
				results <- result{name: candidate.name, idempotency: candidate.key, body: []byte(requestErr.Error())}
				return
			}
			body, readErr := io.ReadAll(response.Body)
			_ = response.Body.Close()
			if readErr != nil {
				body = []byte(readErr.Error())
			}
			results <- result{name: candidate.name, status: response.StatusCode, body: body, idempotency: candidate.key}
		}()
	}
	close(start)
	workers.Wait()
	close(results)

	var committed, conflicted result
	for item := range results {
		switch item.status {
		case http.StatusCreated:
			if committed.status != 0 {
				t.Fatalf("two concurrent Prompt writes committed: first=%#v second=%#v", committed, item)
			}
			committed = item
		case http.StatusConflict:
			if conflicted.status != 0 {
				t.Fatalf("two concurrent Prompt writes conflicted: first=%#v second=%#v", conflicted, item)
			}
			conflicted = item
		default:
			t.Fatalf("concurrent Prompt %s returned status=%d body=%q", item.name, item.status, item.body)
		}
	}
	if committed.status != http.StatusCreated || conflicted.status != http.StatusConflict {
		t.Fatalf("concurrent Prompt CAS did not produce one commit and one conflict: committed=%#v conflicted=%#v", committed, conflicted)
	}
	var receipt struct {
		Prompt struct {
			ID             string `json:"id"`
			ConversationID string `json:"conversation_id"`
			Role           string `json:"role"`
			Content        string `json:"content"`
		} `json:"prompt"`
		AggregateVersion uint64 `json:"aggregate_version"`
		Replayed         bool   `json:"replayed"`
	}
	if err := json.Unmarshal(committed.body, &receipt); err != nil ||
		receipt.Prompt.ID == "" || receipt.Prompt.ConversationID != conversation.ID ||
		receipt.Prompt.Role != "user" || receipt.AggregateVersion != 2 || receipt.Replayed {
		t.Fatalf("winner receipt=%#v body=%q decode=%v", receipt, committed.body, err)
	}
	var conflict struct {
		Code string `json:"code"`
	}
	if err := json.Unmarshal(conflicted.body, &conflict); err != nil || conflict.Code != "conflict" {
		t.Fatalf("loser response body=%q code=%q decode=%v", conflicted.body, conflict.Code, err)
	}

	historyResponse := doConversationClientRequest(t, client, server.URL, creatorToken,
		http.MethodGet, promptPath+"?limit=128", "", "", "")
	if historyResponse.StatusCode != http.StatusOK {
		t.Fatalf("concurrent Prompt history status=%d body=%q", historyResponse.StatusCode, readConversationClientBody(t, historyResponse))
	}
	var history model.ConversationPromptPage
	if err := json.NewDecoder(historyResponse.Body).Decode(&history); err != nil {
		_ = historyResponse.Body.Close()
		t.Fatal(err)
	}
	_ = historyResponse.Body.Close()
	if len(history.Prompts) != 1 || history.Prompts[0].ID != receipt.Prompt.ID || history.Prompts[0].Content != receipt.Prompt.Content {
		t.Fatalf("concurrent Prompt history=%#v winner=%#v", history, receipt)
	}
	assertNoPendingRunIntentAfterPrompt(t, bridge, identity, conversation.ID)
	assertIntegrationNoNormalRun(t, authenticator.Handler(newConversationRoutes(bridge)), identity, conversation.ID)

	// Replaying the winner is still idempotent after the race and does not add
	// another Prompt.
	replay := doConversationClientRequest(t, client, server.URL,
		tokenForIndependentClient(identity, scopes, "concurrent-cas-replay"),
		http.MethodPost, promptPath, "application/json", committed.idempotency,
		`{"content":"`+receipt.Prompt.Content+`","expected_version":1}`)
	if replay.StatusCode != http.StatusOK {
		t.Fatalf("winner replay status=%d body=%q", replay.StatusCode, readConversationClientBody(t, replay))
	}
	_ = replay.Body.Close()
}
