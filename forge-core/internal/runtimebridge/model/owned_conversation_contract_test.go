package model

import (
	"bytes"
	"encoding/json"
	"os"
	"testing"
)

func TestOwnedConversationContractFixture(t *testing.T) {
	fixturePath := os.Getenv("FORGE_CONTRACT_FIXTURE")
	if fixturePath == "" {
		t.Skip("FORGE_CONTRACT_FIXTURE is set by the cross-repository contract test")
	}
	fixture, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatal(err)
	}
	var page OwnedConversationPage
	decoder := json.NewDecoder(bytes.NewReader(fixture))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&page); err != nil {
		t.Fatalf("decode shared conversation page fixture: %v", err)
	}
	if len(page.Conversations) != 2 || page.HasMore || page.NextAfterID != nil {
		t.Fatalf("unexpected shared conversation page: %+v", page)
	}
	if page.Conversations[0].Conversation.ID != "conversation-001" ||
		page.Conversations[0].Conversation.Scope.Kind != "project" ||
		page.Conversations[1].Conversation.ID != "conversation-002" ||
		page.Conversations[1].Conversation.Scope.Kind != "global" {
		t.Fatalf("unexpected shared conversation identities: %+v", page.Conversations)
	}
}
