package appserver

import (
	model "forgeos/forge-core/internal/runtimebridge/model"
	"net/http"
	"testing"
)

func TestConversationHTTPRejectsTimestampAboveJSONSafeInteger(t *testing.T) {
	backend := &fakeConversationBackend{detail: model.OwnedConversationEntry{
		Conversation: model.Conversation{
			ID: "conversation-1", Scope: model.ConversationScope{Kind: "global"}, Title: "Shared",
			CreatedAtMS: maxSafeJSONInteger, UpdatedAtMS: maxSafeJSONInteger,
		},
		AggregateVersion: 1,
	}}
	identity, handler := conversationTestHandler(t, backend)
	request := func() int {
		response := requestConversationAPI(
			t, handler, identity, http.MethodGet,
			conversationCollectionPath+"/conversation-1",
			"forge:conversations:read", "", "", "",
		)
		return response.Code
	}
	if status := request(); status != http.StatusOK {
		t.Fatalf("JSON-safe boundary status=%d", status)
	}
	backend.detail.Conversation.CreatedAtMS = maxSafeJSONInteger + 1
	backend.detail.Conversation.UpdatedAtMS = maxSafeJSONInteger + 1
	if status := request(); status != http.StatusBadGateway {
		t.Fatalf("unsafe timestamp status=%d", status)
	}
	backend.detail.Conversation.CreatedAtMS = 20
	backend.detail.Conversation.UpdatedAtMS = 10
	if status := request(); status != http.StatusBadGateway {
		t.Fatalf("chronologically invalid timestamp status=%d", status)
	}
}

func TestConversationPageTimestampTransportValidation(t *testing.T) {
	conversation := model.Conversation{
		ID: "conversation-1", Scope: model.ConversationScope{Kind: "global"}, Title: "Shared",
		CreatedAtMS: maxSafeJSONInteger, UpdatedAtMS: maxSafeJSONInteger,
	}
	page := model.OwnedConversationPage{Conversations: []model.OwnedConversationEntry{{
		Conversation: conversation, AggregateVersion: 1,
	}}}
	if !conversationPageTimestampsJSONSafe(page) {
		t.Fatal("JSON-safe Conversation page rejected")
	}
	page.Conversations[0].Conversation.UpdatedAtMS = maxSafeJSONInteger + 1
	if conversationPageTimestampsJSONSafe(page) {
		t.Fatal("unsafe Conversation page accepted")
	}
	page.Conversations[0].Conversation.CreatedAtMS = 20
	page.Conversations[0].Conversation.UpdatedAtMS = 10
	if conversationPageTimestampsJSONSafe(page) {
		t.Fatal("chronologically invalid Conversation page accepted")
	}
}
