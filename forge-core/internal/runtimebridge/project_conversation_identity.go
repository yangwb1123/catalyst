package runtimebridge

import (
	"context"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// model.OwnedProjectConversationIdentity resolves the Hub model.Project scope of one
// model.Conversation owned by the exact verified principal. The result is a minimal
// trusted input for server-owned profile selection; it carries no path, title,
// Prompt, or caller-selected model.Project data.
func (client *Client) OwnedProjectConversationIdentity(
	ctx context.Context,
	owner model.Owner,
	conversationID string,
) (model.OwnedProjectConversationIdentity, error) {
	if !validOwner(owner) || !validEntityID(conversationID) {
		return model.OwnedProjectConversationIdentity{}, &Error{Code: "invalid_owned_conversation_request"}
	}
	ownerCopy := owner
	response, err := client.callWrite(ctx, request{
		Operation:      "owned_project_conversation_identity",
		Owner:          &ownerCopy,
		ConversationID: conversationID,
	})
	if err != nil {
		return model.OwnedProjectConversationIdentity{}, err
	}
	var identity model.OwnedProjectConversationIdentity
	if err := decodeStrict(response, &identity); err != nil ||
		!validOwnedProjectConversationIdentity(response, identity, conversationID) {
		return model.OwnedProjectConversationIdentity{}, &Error{Code: "invalid_runtime_response"}
	}
	return identity, nil
}

func validOwnedProjectConversationIdentity(
	data []byte,
	identity model.OwnedProjectConversationIdentity,
	requestedConversationID string,
) bool {
	return requireObjectFieldSet(data, "conversation_id", "project_id") == nil &&
		validEntityID(identity.ConversationID) && identity.ConversationID == requestedConversationID &&
		validEntityID(identity.ProjectID)
}
