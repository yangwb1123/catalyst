package appserver

import (
	"net/http"
	"testing"

	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestConversationPromptAppendRejectsBackendResponseBindingDrift(t *testing.T) {
	basePrompt := model.ConversationPrompt{
		ID:             "prompt-1",
		ConversationID: "conversation-1",
		Role:           "user",
		Content:        "do work",
		CreatedAtMS:    21,
	}
	cases := []struct {
		name             string
		prompt           model.ConversationPrompt
		aggregateVersion uint64
	}{
		{
			name: "foreign conversation",
			prompt: func() model.ConversationPrompt {
				value := basePrompt
				value.ConversationID = "conversation-other"
				return value
			}(),
			aggregateVersion: 8,
		},
		{
			name: "non-user role",
			prompt: func() model.ConversationPrompt {
				value := basePrompt
				value.Role = "assistant"
				return value
			}(),
			aggregateVersion: 8,
		},
		{
			name: "content drift",
			prompt: func() model.ConversationPrompt {
				value := basePrompt
				value.Content = "different content"
				return value
			}(),
			aggregateVersion: 8,
		},
		{
			name: "missing prompt identity",
			prompt: func() model.ConversationPrompt {
				value := basePrompt
				value.ID = ""
				return value
			}(),
			aggregateVersion: 8,
		},
		{
			name:             "aggregate version drift",
			prompt:           basePrompt,
			aggregateVersion: 7,
		},
		{
			name: "unsafe prompt timestamp",
			prompt: func() model.ConversationPrompt {
				value := basePrompt
				value.CreatedAtMS = maxSafeJSONInteger + 1
				return value
			}(),
			aggregateVersion: 8,
		},
	}

	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			backend := &fakeConversationBackend{
				appendPrompt: test.prompt,
				appendAggVer: test.aggregateVersion,
			}
			identity, handler := conversationTestHandler(t, backend)
			response := requestConversationAPI(
				t,
				handler,
				identity,
				http.MethodPost,
				conversationCollectionPath+"/conversation-1/prompts",
				"forge:conversations:write",
				"application/json",
				"prompt-key",
				`{"content":"do work","expected_version":7}`,
			)
			if response.Code != http.StatusBadGateway || backend.appendCalls != 1 {
				t.Fatalf("response status=%d calls=%d body=%q", response.Code, backend.appendCalls, response.Body.String())
			}
		})
	}
}
