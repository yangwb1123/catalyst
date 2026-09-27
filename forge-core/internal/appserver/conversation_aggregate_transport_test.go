package appserver

import (
	"fmt"
	"net/http"
	"testing"

	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestConversationHTTPAggregateVersionUsesJSONSafeBoundary(t *testing.T) {
	conversation := model.Conversation{
		ID: "conversation-1", Scope: model.ConversationScope{Kind: "global"}, Title: "Shared",
		CreatedAtMS: 1, UpdatedAtMS: 1,
	}
	backend := &fakeConversationBackend{
		detail: model.OwnedConversationEntry{Conversation: conversation, AggregateVersion: maxSafeJSONInteger},
		listPage: model.OwnedConversationPage{Conversations: []model.OwnedConversationEntry{{
			Conversation: conversation, AggregateVersion: maxSafeJSONInteger,
		}}},
		importResult: model.OwnedConversationImportResult{
			Conversation: conversation, AggregateVersion: maxSafeJSONInteger,
		},
		appendPrompt: model.ConversationPrompt{
			ID: "prompt-1", ConversationID: conversation.ID, Role: "user", Content: "ship it", CreatedAtMS: 1,
		},
		appendAggVer: maxSafeJSONInteger,
	}
	identity, handler := conversationTestHandler(t, backend)

	detailPath := conversationCollectionPath + "/conversation-1"
	if response := requestConversationAPI(t, handler, identity, http.MethodGet, detailPath,
		"forge:conversations:read", "", "", ""); response.Code != http.StatusOK {
		t.Fatalf("safe detail status=%d body=%q", response.Code, response.Body.String())
	}
	backend.detail.AggregateVersion = maxSafeJSONInteger + 1
	if response := requestConversationAPI(t, handler, identity, http.MethodGet, detailPath,
		"forge:conversations:read", "", "", ""); response.Code != http.StatusBadGateway {
		t.Fatalf("unsafe detail status=%d body=%q", response.Code, response.Body.String())
	}

	backend.listPage.Conversations[0].AggregateVersion = maxSafeJSONInteger
	if response := requestConversationAPI(t, handler, identity, http.MethodGet, conversationCollectionPath,
		"forge:conversations:read", "", "", ""); response.Code != http.StatusOK {
		t.Fatalf("safe list status=%d body=%q", response.Code, response.Body.String())
	}
	backend.listPage.Conversations[0].AggregateVersion = maxSafeJSONInteger + 1
	if response := requestConversationAPI(t, handler, identity, http.MethodGet, conversationCollectionPath,
		"forge:conversations:read", "", "", ""); response.Code != http.StatusBadGateway {
		t.Fatalf("unsafe list status=%d body=%q", response.Code, response.Body.String())
	}

	importBody := `{"title":"Shared","prompts":[]}`
	backend.importResult.AggregateVersion = maxSafeJSONInteger
	if response := requestConversationAPI(t, handler, identity, http.MethodPost, conversationImportPath,
		"forge:conversations:write", "application/json", "aggregate-import", importBody); response.Code != http.StatusCreated {
		t.Fatalf("safe import status=%d body=%q", response.Code, response.Body.String())
	}
	backend.importResult.AggregateVersion = maxSafeJSONInteger + 1
	if response := requestConversationAPI(t, handler, identity, http.MethodPost, conversationImportPath,
		"forge:conversations:write", "application/json", "aggregate-import-unsafe", importBody); response.Code != http.StatusBadGateway {
		t.Fatalf("unsafe import status=%d body=%q", response.Code, response.Body.String())
	}

	promptPath := conversationCollectionPath + "/conversation-1/prompts"
	appendBody := fmt.Sprintf(`{"content":"ship it","expected_version":%d}`, maxSafeJSONInteger-1)
	backend.appendAggVer = maxSafeJSONInteger
	if response := requestConversationAPI(t, handler, identity, http.MethodPost, promptPath,
		"forge:conversations:write", "application/json", "aggregate-prompt", appendBody); response.Code != http.StatusCreated {
		t.Fatalf("safe append status=%d body=%q", response.Code, response.Body.String())
	}
	backend.appendAggVer = maxSafeJSONInteger + 1
	if response := requestConversationAPI(t, handler, identity, http.MethodPost, promptPath,
		"forge:conversations:write", "application/json", "aggregate-prompt-unsafe", appendBody); response.Code != http.StatusBadGateway {
		t.Fatalf("unsafe append status=%d body=%q", response.Code, response.Body.String())
	}
}
