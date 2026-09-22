package runtimebridge

import (
	"encoding/json"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"testing"
)

func TestOwnerPrincipalAndPaginationValidation(t *testing.T) {
	owner := model.Owner{Issuer: "https://id.example.test", Subject: "user-1", TenantID: "tenant-1"}
	if !validOwner(owner) {
		t.Fatal("valid owner rejected")
	}
	owner.TenantID = " "
	if validOwner(owner) {
		t.Fatal("blank tenant accepted")
	}
	page := model.OwnedConversationPage{
		Conversations: []model.OwnedConversationEntry{{
			Conversation: model.Conversation{
				ID: "conversation-a", Scope: model.ConversationScope{Kind: "global"},
				Title: "Shared", CreatedAtMS: 10, UpdatedAtMS: 11,
			},
			AggregateVersion: 1,
		}},
		HasMore: false,
	}
	data, err := json.Marshal(page)
	if err != nil || !validOwnedConversationPage(data, page, "", 2) {
		t.Fatalf("valid owner page rejected: %v %s", err, data)
	}
	page.Conversations = append(page.Conversations, page.Conversations[0])
	data, err = json.Marshal(page)
	if err != nil || validOwnedConversationPage(data, page, "", 2) {
		t.Fatal("duplicate conversation page accepted")
	}
}

func TestOwnedPromptAppendProjectionHidesIdempotencyAndFixesRole(t *testing.T) {
	result := ownedPromptAppendResult{
		Prompt: model.ConversationPrompt{
			ID: "prompt-1", ConversationID: "conversation-1", Role: "user",
			Content: "compute this", CreatedAtMS: 20,
		},
		AggregateVersion: 2,
	}
	data, err := json.Marshal(result)
	if err != nil || !validOwnedPromptAppend(data, result, "conversation-1", "compute this") {
		t.Fatalf("valid Prompt response rejected: %v %s", err, data)
	}
	result.Prompt.Role = "assistant"
	data, err = json.Marshal(result)
	if err != nil || validOwnedPromptAppend(data, result, "conversation-1", "compute this") {
		t.Fatal("non-user API Prompt accepted")
	}
}

func TestConversationTimestampsUseJSONSafeIntegerBoundary(t *testing.T) {
	conversation := model.Conversation{
		ID: "conversation-1", Scope: model.ConversationScope{Kind: "global"}, Title: "Shared",
		CreatedAtMS: maxSafeJSONInteger, UpdatedAtMS: maxSafeJSONInteger,
	}
	if !validConversation(conversation) {
		t.Fatal("JSON-safe Conversation timestamp boundary rejected")
	}
	conversation.CreatedAtMS = maxSafeJSONInteger + 1
	conversation.UpdatedAtMS = maxSafeJSONInteger + 1
	if validConversation(conversation) {
		t.Fatal("Conversation timestamp above JSON-safe integer accepted")
	}
}
