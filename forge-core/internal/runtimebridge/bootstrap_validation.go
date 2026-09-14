package runtimebridge

import (
	"encoding/json"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"strings"
)

func validConversationBootstrapCursor(cursor model.ConversationBootstrapCursor) bool {
	if cursor.SnapshotCursor > maxSQLiteInteger {
		return false
	}
	switch cursor.Phase {
	case model.ConversationBootstrapLegacyBaseline:
		return cursor.AfterConversationID != nil && validEntityID(*cursor.AfterConversationID) &&
			cursor.AfterChangeCursor == nil
	case model.ConversationBootstrapChangeLog:
		return cursor.AfterConversationID == nil && cursor.AfterChangeCursor != nil &&
			*cursor.AfterChangeCursor <= cursor.SnapshotCursor
	default:
		return false
	}
}

func validEntityID(value string) bool {
	return strings.TrimSpace(value) != "" && len(value) <= maxEntityIDBytes
}

func validConversationBootstrapPage(
	data []byte,
	page model.ConversationBootstrapPage,
	cursor *model.ConversationBootstrapCursor,
	limit int,
) bool {
	fields := []string{"snapshot_cursor", "conversations", "scanned_through_cursor", "has_more"}
	if page.NextCursor != nil {
		fields = append(fields, "next_cursor")
	}
	if requireObjectFieldSet(data, fields...) != nil || page.SnapshotCursor > maxSQLiteInteger ||
		page.ScannedThroughCursor > page.SnapshotCursor ||
		page.Conversations == nil || len(page.Conversations) > limit {
		return false
	}
	if cursor != nil && page.SnapshotCursor != cursor.SnapshotCursor {
		return false
	}
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil {
		return false
	}
	var rawEntries []json.RawMessage
	if json.Unmarshal(root["conversations"], &rawEntries) != nil || rawEntries == nil ||
		len(rawEntries) != len(page.Conversations) {
		return false
	}
	for index, entry := range page.Conversations {
		if !validConversationBootstrapEntry(rawEntries[index], entry) {
			return false
		}
	}
	if page.HasMore != (page.NextCursor != nil) {
		return false
	}
	if page.NextCursor != nil && !validConversationBootstrapCursorJSON(root["next_cursor"], *page.NextCursor) {
		return false
	}
	return validBootstrapPageProgress(page, cursor, limit)
}

func validConversationBootstrapEntry(data []byte, entry model.ConversationBootstrapEntry) bool {
	if requireObjectFieldSet(data, "conversation", "creation_cursor", "aggregate_version") != nil ||
		!validEntityID(entry.Conversation.ID) || strings.TrimSpace(entry.Conversation.Title) == "" ||
		len(entry.Conversation.Title) > 256 || entry.Conversation.CreatedAtMS > maxSQLiteInteger ||
		entry.Conversation.UpdatedAtMS > maxSQLiteInteger ||
		entry.Conversation.UpdatedAtMS < entry.Conversation.CreatedAtMS ||
		entry.AggregateVersion > maxSQLiteInteger ||
		entry.CreationCursor > maxSQLiteInteger {
		return false
	}
	if entry.CreationCursor > 0 && entry.AggregateVersion == 0 {
		return false
	}
	var object map[string]json.RawMessage
	if json.Unmarshal(data, &object) != nil ||
		requireObjectFieldSet(object["conversation"], "id", "scope", "title", "created_at_ms", "updated_at_ms") != nil {
		return false
	}
	var conversation map[string]json.RawMessage
	if json.Unmarshal(object["conversation"], &conversation) != nil ||
		!validConversationScope(conversation["scope"]) {
		return false
	}
	var scope model.ConversationScope
	if decodeStrict(conversation["scope"], &scope) != nil ||
		(scope.Kind != "global" && !validEntityID(scope.ID)) {
		return false
	}
	return true
}

func validConversationBootstrapCursorJSON(data []byte, cursor model.ConversationBootstrapCursor) bool {
	var fields []string
	switch cursor.Phase {
	case model.ConversationBootstrapLegacyBaseline:
		fields = []string{"snapshot_cursor", "phase", "after_conversation_id"}
	case model.ConversationBootstrapChangeLog:
		fields = []string{"snapshot_cursor", "phase", "after_change_cursor"}
	default:
		return false
	}
	return requireObjectFieldSet(data, fields...) == nil
}

func validBootstrapPageProgress(
	page model.ConversationBootstrapPage,
	cursor *model.ConversationBootstrapCursor,
	limit int,
) bool {
	phase, afterID, afterChange := bootstrapPageStart(cursor)
	if !validBootstrapScanRange(page, cursor, limit) || !validBootstrapEntryOrder(page, phase, afterID, afterChange) {
		return false
	}
	if phase == model.ConversationBootstrapLegacyBaseline {
		return validBaselinePageProgress(page, afterID)
	}
	return validChangePageProgress(page, afterChange)
}

func bootstrapPageStart(cursor *model.ConversationBootstrapCursor) (model.ConversationBootstrapPhase, string, uint64) {
	if cursor == nil {
		return model.ConversationBootstrapLegacyBaseline, "", 0
	}
	var afterID string
	if cursor.AfterConversationID != nil {
		afterID = *cursor.AfterConversationID
	}
	var afterChange uint64
	if cursor.AfterChangeCursor != nil {
		afterChange = *cursor.AfterChangeCursor
	}
	return cursor.Phase, afterID, afterChange
}

func validBootstrapScanRange(
	page model.ConversationBootstrapPage,
	cursor *model.ConversationBootstrapCursor,
	limit int,
) bool {
	if cursor == nil || cursor.Phase == model.ConversationBootstrapLegacyBaseline {
		return page.ScannedThroughCursor == 0
	}
	after := cursor.AfterChangeCursor
	return after != nil && page.ScannedThroughCursor >= *after &&
		page.ScannedThroughCursor-*after <= uint64(limit)
}

func validBootstrapEntryOrder(
	page model.ConversationBootstrapPage,
	phase model.ConversationBootstrapPhase,
	afterID string,
	afterChange uint64,
) bool {
	seen := make(map[string]struct{}, len(page.Conversations))
	for index, entry := range page.Conversations {
		id := entry.Conversation.ID
		if _, found := seen[id]; found {
			return false
		}
		seen[id] = struct{}{}
		if index > 0 && !bootstrapEntriesAscend(page.Conversations[index-1], entry, phase) {
			return false
		}
		if !bootstrapEntryAfter(entry, phase, afterID, afterChange, page.ScannedThroughCursor) {
			return false
		}
	}
	return true
}

func bootstrapEntriesAscend(previous, current model.ConversationBootstrapEntry, phase model.ConversationBootstrapPhase) bool {
	if phase == model.ConversationBootstrapLegacyBaseline {
		return previous.Conversation.ID < current.Conversation.ID
	}
	return previous.CreationCursor < current.CreationCursor
}

func bootstrapEntryAfter(
	entry model.ConversationBootstrapEntry,
	phase model.ConversationBootstrapPhase,
	afterID string,
	afterChange uint64,
	scannedThrough uint64,
) bool {
	if phase == model.ConversationBootstrapLegacyBaseline {
		return entry.CreationCursor == 0 && entry.Conversation.ID > afterID
	}
	return phase == model.ConversationBootstrapChangeLog && entry.CreationCursor > afterChange &&
		entry.CreationCursor <= scannedThrough
}

func validBaselinePageProgress(page model.ConversationBootstrapPage, afterID string) bool {
	if !page.HasMore {
		return page.NextCursor == nil && page.SnapshotCursor == 0
	}
	next := page.NextCursor
	if next == nil || !validConversationBootstrapCursor(*next) || next.SnapshotCursor != page.SnapshotCursor {
		return false
	}
	switch next.Phase {
	case model.ConversationBootstrapLegacyBaseline:
		if next.AfterConversationID == nil || len(page.Conversations) == 0 {
			return false
		}
		lastID := page.Conversations[len(page.Conversations)-1].Conversation.ID
		return *next.AfterConversationID == lastID && *next.AfterConversationID > afterID
	case model.ConversationBootstrapChangeLog:
		return next.AfterConversationID == nil && next.AfterChangeCursor != nil &&
			*next.AfterChangeCursor == 0 && page.SnapshotCursor > 0
	default:
		return false
	}
}

func validChangePageProgress(page model.ConversationBootstrapPage, afterChange uint64) bool {
	if !page.HasMore {
		return page.NextCursor == nil && page.ScannedThroughCursor == page.SnapshotCursor
	}
	next := page.NextCursor
	if next == nil || !validConversationBootstrapCursor(*next) || next.Phase != model.ConversationBootstrapChangeLog ||
		next.SnapshotCursor != page.SnapshotCursor || next.AfterChangeCursor == nil {
		return false
	}
	if *next.AfterChangeCursor <= afterChange || *next.AfterChangeCursor >= page.SnapshotCursor ||
		*next.AfterChangeCursor != page.ScannedThroughCursor {
		return false
	}
	return len(page.Conversations) == 0 ||
		page.Conversations[len(page.Conversations)-1].CreationCursor <= *next.AfterChangeCursor
}
