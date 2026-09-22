package runtimebridge

import (
	"context"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestRustRPCBinarySnapshotRoundTripWhenConfigured(t *testing.T) {
	client := configuredRustRuntimeClient(t)
	cursor := assertRustSnapshotAndPromptRoundTrip(t, client)
	assertRustBootstrapRoundTrip(t, client)
	assertRustChangeTail(t, client, cursor)
	assertRustOwnedConversationRoundTrip(t, client)
}

func assertRustOwnedConversationRoundTrip(t *testing.T, client *Client) {
	t.Helper()
	requestID, err := newRequestID()
	if err != nil {
		t.Fatal(err)
	}
	owner := model.Owner{
		Issuer: "https://identity.example", Subject: "e2e-" + requestID, TenantID: "tenant-e2e",
	}
	conversation, err := client.CreateOwnedConversation(context.Background(), owner,
		model.ConversationScope{Kind: "global"}, "Go to Rust owner boundary", "create-"+requestID)
	if err != nil {
		t.Fatal(err)
	}
	page, err := client.ListOwnedConversations(context.Background(), owner, "", 16)
	if err != nil {
		t.Fatal(err)
	}
	aggregateVersion := ownedConversationVersion(t, page, conversation.ID)
	detail, err := client.GetOwnedConversation(context.Background(), owner, conversation.ID)
	if err != nil || detail.Conversation.ID != conversation.ID || detail.AggregateVersion != aggregateVersion {
		t.Fatalf("owned Conversation detail = %#v, error %v", detail, err)
	}

	content, idempotencyKey := "persist one user Prompt", "prompt-"+requestID
	prompt, nextVersion, replayed, err := client.AppendOwnedPrompt(context.Background(), owner,
		conversation.ID, content, idempotencyKey, aggregateVersion)
	if err != nil || replayed || nextVersion != aggregateVersion+1 {
		t.Fatalf("owned Prompt append = %#v version=%d replayed=%v err=%v", prompt, nextVersion, replayed, err)
	}
	retry, retryVersion, replayed, err := client.AppendOwnedPrompt(context.Background(), owner,
		conversation.ID, content, idempotencyKey, aggregateVersion)
	if err != nil || !replayed || retry.ID != prompt.ID || retryVersion != nextVersion {
		t.Fatalf("owned Prompt retry = %#v version=%d replayed=%v err=%v", retry, retryVersion, replayed, err)
	}
	if _, _, _, err := client.AppendOwnedPrompt(context.Background(), owner,
		conversation.ID, "stale write", "stale-"+requestID, aggregateVersion); err == nil || err.(*Error).Code != "conflict" {
		t.Fatalf("stale owned Prompt write was not a conflict: %v", err)
	}
	foreign := owner
	foreign.TenantID = "tenant-other"
	if _, err := client.GetOwnedConversation(context.Background(), foreign, conversation.ID); err == nil || err.(*Error).Code != "not_found" {
		t.Fatalf("foreign owner could read Conversation detail: %v", err)
	}
	if _, err := client.OwnedConversationPrompts(context.Background(), foreign, conversation.ID, nil, 8); err == nil || err.(*Error).Code != "not_found" {
		t.Fatalf("foreign owner could read Prompt page: %v", err)
	}
	history, err := client.OwnedConversationPrompts(context.Background(), owner, conversation.ID, nil, 8)
	if err != nil || len(history.Prompts) != 1 || history.Prompts[0].ID != prompt.ID {
		t.Fatalf("owner Prompt page = %#v, error %v", history, err)
	}
}

func ownedConversationVersion(t *testing.T, page model.OwnedConversationPage, conversationID string) uint64 {
	t.Helper()
	for _, entry := range page.Conversations {
		if entry.Conversation.ID == conversationID && entry.AggregateVersion > 0 {
			return entry.AggregateVersion
		}
	}
	t.Fatalf("new owned Conversation missing from owner page: %#v", page)
	return 0
}

func configuredRustRuntimeClient(t *testing.T) *Client {
	t.Helper()
	executable := os.Getenv("FORGE_RUNTIME_BIN")
	runtimeState := os.Getenv("FORGE_RUNTIME_STATE_DIR")
	if executable == "" || runtimeState == "" {
		t.Skip("set FORGE_RUNTIME_BIN and FORGE_RUNTIME_STATE_DIR after initializing a Hub")
	}
	appState := filepath.Join(t.TempDir(), "app-server-state")
	makeDirectory(t, appState)
	client, err := New(Config{
		Executable: executable, AppServerStateDir: appState,
		RuntimeStateDir: runtimeState, Timeout: 5 * time.Second,
	})
	if err != nil {
		t.Fatal(err)
	}
	return client
}

func assertRustSnapshotAndPromptRoundTrip(t *testing.T, client *Client) uint64 {
	t.Helper()
	snapshot, err := client.SnapshotAtCursor(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if snapshot.Snapshot.Scope.Kind != "global" {
		t.Fatalf("snapshot scope = %#v", snapshot.Snapshot.Scope)
	}
	if len(snapshot.Snapshot.Conversations) > 0 {
		conversation := snapshot.Snapshot.Conversations[0]
		prompts, err := client.ConversationPrompts(context.Background(), conversation.ID, nil, 8)
		if err != nil {
			t.Fatal(err)
		}
		if prompts.ConversationID != conversation.ID {
			t.Fatalf("Prompt page model.Conversation = %q, want %q", prompts.ConversationID, conversation.ID)
		}
		for _, prompt := range prompts.Prompts {
			if prompt.ConversationID != conversation.ID {
				t.Fatalf("cross-model.Conversation Prompt in page: %#v", prompt)
			}
		}
	}
	return snapshot.Cursor
}

func assertRustBootstrapRoundTrip(t *testing.T, client *Client) {
	t.Helper()
	var cursor *model.ConversationBootstrapCursor
	for pageNumber := 0; pageNumber < 1024; pageNumber++ {
		page, err := client.ConversationBootstrap(context.Background(), cursor, 8)
		if err != nil {
			t.Fatal(err)
		}
		for _, entry := range page.Conversations {
			if entry.CreationCursor > page.ScannedThroughCursor ||
				entry.CreationCursor > page.SnapshotCursor {
				t.Fatalf("bootstrap entry exceeds its scanned head: %#v", entry)
			}
		}
		if !page.HasMore {
			return
		}
		cursor = page.NextCursor
	}
	t.Fatal("bootstrap did not finish within 1024 pages")
}

func assertRustChangeTail(t *testing.T, client *Client, cursor uint64) {
	t.Helper()
	page, err := client.ChangesAfter(context.Background(), cursor, 1)
	if err != nil {
		t.Fatal(err)
	}
	if page.AfterCursor != cursor || page.HeadCursor != cursor || len(page.Changes) != 0 {
		t.Fatalf("changes after snapshot = %#v", page)
	}
}
