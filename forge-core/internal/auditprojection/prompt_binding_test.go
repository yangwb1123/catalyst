package auditprojection

import (
	"errors"
	"testing"

	"forgeos/forge-core/internal/runtimebridge/model"
)

func TestProjectCommittedPromptBindsReceiptToCommittedChange(t *testing.T) {
	prompt := model.ConversationPrompt{
		ID: "prompt-19", ConversationID: "conversation-7", Role: "user",
		Content: "private prompt", CreatedAtMS: 1789300800000,
	}
	event, err := ProjectCommittedPrompt(projectionOwner(), prompt, projectionChange())
	if err != nil {
		t.Fatal(err)
	}
	if event.AggregateID != prompt.ConversationID || event.OperationID != prompt.ID ||
		event.AggregateVersion != int64(projectionChange().AggregateVersion) ||
		event.Payload.ContentIncluded {
		t.Fatalf("committed Prompt binding lost change identity: %#v", event)
	}
	replayed, err := ProjectCommittedPrompt(projectionOwner(), prompt, projectionChange())
	if err != nil || replayed != event {
		t.Fatalf("exact committed retry changed projected identity: first=%#v replay=%#v err=%v", event, replayed, err)
	}
}

func TestProjectCommittedPromptRejectsMismatchedReceiptAndChange(t *testing.T) {
	prompt := model.ConversationPrompt{
		ID: "prompt-19", ConversationID: "conversation-7", Role: "user",
		Content: "private prompt", CreatedAtMS: 1789300800000,
	}
	cases := []struct {
		name   string
		mutate func(*model.ConversationPrompt, *model.Change)
	}{
		{name: "prompt identity", mutate: func(value *model.ConversationPrompt, _ *model.Change) { value.ID = "prompt-other" }},
		{name: "conversation identity", mutate: func(value *model.ConversationPrompt, _ *model.Change) { value.ConversationID = "conversation-other" }},
		{name: "role", mutate: func(value *model.ConversationPrompt, _ *model.Change) { value.Role = "assistant" }},
		{name: "created time", mutate: func(value *model.ConversationPrompt, _ *model.Change) { value.CreatedAtMS++ }},
		{name: "change entity", mutate: func(_ *model.ConversationPrompt, value *model.Change) { value.EntityID = "prompt-other" }},
		{name: "change conversation", mutate: func(_ *model.ConversationPrompt, value *model.Change) { value.ConversationID = "conversation-other" }},
		{name: "change time", mutate: func(_ *model.ConversationPrompt, value *model.Change) { value.CreatedAtMS++ }},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			candidate, change := prompt, projectionChange()
			test.mutate(&candidate, &change)
			if _, err := ProjectCommittedPrompt(projectionOwner(), candidate, change); !errors.Is(err, ErrInvalidProjection) {
				t.Fatalf("mismatched Prompt/change accepted: %v", err)
			}
		})
	}
}
