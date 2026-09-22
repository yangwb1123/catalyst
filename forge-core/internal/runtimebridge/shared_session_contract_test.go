package runtimebridge

import (
	"encoding/json"
	"os"
	"testing"

	model "forgeos/forge-core/internal/runtimebridge/model"
)

// TestSharedSessionContractFixture keeps the Go bridge validators, Rust remote
// client, and Flutter decoder pinned to one closed shared-session response
// shape. The fixture is caller-supplied test data; it is not a live service
// or device/execution authorization.
func TestSharedSessionContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_SESSION_CONTRACT_FIXTURE")
	if path == "" {
		t.Skip("FORGE_SESSION_CONTRACT_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var envelope map[string]json.RawMessage
	if err := decodeStrict(data, &envelope); err != nil || len(envelope) != 5 {
		t.Fatalf("decode shared-session envelope: %v", err)
	}
	for _, key := range []string{"conversation_page", "conversation_detail", "prompt_page", "change_page", "append_prompt"} {
		if _, ok := envelope[key]; !ok {
			t.Fatalf("shared-session fixture missing %q", key)
		}
	}

	var detail model.OwnedConversationEntry
	if err := decodeStrict(envelope["conversation_detail"], &detail); err != nil ||
		!validOwnedConversationEntry(envelope["conversation_detail"], detail) ||
		detail.Conversation.ID != "conversation-001" {
		t.Fatalf("invalid shared Conversation detail: %v %#v", err, detail)
	}

	var conversations model.OwnedConversationPage
	if err := decodeStrict(envelope["conversation_page"], &conversations); err != nil ||
		!validOwnedConversationPage(envelope["conversation_page"], conversations, "", 2) {
		t.Fatalf("invalid shared Conversation page: %v %#v", err, conversations)
	}

	var prompts model.ConversationPromptPage
	if err := decodeStrict(envelope["prompt_page"], &prompts); err != nil ||
		!validPromptPage(envelope["prompt_page"], prompts, "conversation-001", nil, 2) {
		t.Fatalf("invalid shared Prompt page: %v %#v", err, prompts)
	}

	var changes model.OwnedConversationChangePage
	if err := decodeStrict(envelope["change_page"], &changes); err != nil ||
		!validOwnedConversationChangePage(envelope["change_page"], changes, 0, 2) {
		t.Fatalf("invalid shared change page: %v %#v", err, changes)
	}

	var appendResult ownedPromptAppendResult
	if err := decodeStrict(envelope["append_prompt"], &appendResult); err != nil ||
		!validOwnedPromptAppend(envelope["append_prompt"], appendResult,
			"conversation-001", "send this from another client") {
		t.Fatalf("invalid shared Prompt append: %v %#v", err, appendResult)
	}
}
