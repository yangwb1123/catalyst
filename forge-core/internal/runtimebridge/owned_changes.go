package runtimebridge

import (
	"context"
	"encoding/json"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// OwnedConversationChangesAfter reads a bounded owner-scoped replay page.
// The owner is forwarded only over the trusted Runtime v2 boundary.
func (client *Client) OwnedConversationChangesAfter(
	ctx context.Context,
	owner model.Owner,
	after uint64,
	limit int,
) (model.OwnedConversationChangePage, error) {
	if !validOwner(owner) {
		return model.OwnedConversationChangePage{}, &Error{Code: "invalid_owned_request"}
	}
	if after > maxSafeJSONInteger {
		return model.OwnedConversationChangePage{}, &Error{Code: "invalid_cursor"}
	}
	if limit < 1 || limit > maxChangeLimit {
		return model.OwnedConversationChangePage{}, &Error{Code: "invalid_limit"}
	}
	ownerCopy := owner
	response, err := client.callWrite(ctx, request{
		Operation:   "owned_conversation_changes_after",
		Owner:       &ownerCopy,
		AfterCursor: &after,
		Limit:       &limit,
	})
	if err != nil {
		return model.OwnedConversationChangePage{}, err
	}
	var page model.OwnedConversationChangePage
	if err := decodeStrict(response, &page); err != nil ||
		!validOwnedConversationChangePage(response, page, after, limit) {
		return model.OwnedConversationChangePage{}, &Error{Code: "invalid_runtime_response"}
	}
	return page, nil
}

func validOwnedConversationChangePage(
	data []byte,
	page model.OwnedConversationChangePage,
	after uint64,
	limit int,
) bool {
	if after > maxSafeJSONInteger ||
		requireObjectFieldSet(data, "after_cursor", "scanned_through_cursor", "has_more", "changes") != nil ||
		page.AfterCursor != after || page.ScannedThroughCursor < after ||
		page.ScannedThroughCursor > maxSafeJSONInteger || page.Changes == nil || len(page.Changes) > limit {
		return false
	}
	if len(page.Changes) == 0 {
		return !page.HasMore && page.ScannedThroughCursor == after
	}
	if page.HasMore && len(page.Changes) != limit {
		return false
	}
	if page.Changes[len(page.Changes)-1].Cursor != page.ScannedThroughCursor {
		return false
	}
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil {
		return false
	}
	var rawChanges []json.RawMessage
	if json.Unmarshal(root["changes"], &rawChanges) != nil || len(rawChanges) != len(page.Changes) {
		return false
	}
	return validOwnedConversationChanges(page.Changes, rawChanges, after, page.ScannedThroughCursor)
}

func validOwnedConversationChanges(
	changes []model.Change,
	rawChanges []json.RawMessage,
	after uint64,
	scannedThrough uint64,
) bool {
	if len(rawChanges) != len(changes) {
		return false
	}
	previous := after
	for index, change := range changes {
		if previous == maxSafeJSONInteger || change.Cursor != previous+1 ||
			requireObjectFieldSet(rawChanges[index], "cursor", "schema_version", "conversation_id", "entity_id",
				"aggregate_version", "kind", "created_at_ms") != nil ||
			change.Cursor > scannedThrough || change.SchemaVersion != 1 ||
			!validEntityID(change.ConversationID) || !validEntityID(change.EntityID) ||
			change.AggregateVersion == 0 || change.AggregateVersion > maxSafeJSONInteger ||
			change.CreatedAtMS > maxSafeJSONInteger ||
			(change.Kind != "conversation_created" && change.Kind != "prompt_appended") {
			return false
		}
		if change.Kind == "conversation_created" && change.EntityID != change.ConversationID {
			return false
		}
		previous = change.Cursor
	}
	return true
}
