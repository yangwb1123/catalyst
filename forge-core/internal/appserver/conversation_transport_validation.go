package appserver

import (
	"forgeos/forge-core/internal/auditprojection"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
	"strings"
)

func conversationTimestampsJSONSafe(conversation model.Conversation) bool {
	return conversation.CreatedAtMS <= conversation.UpdatedAtMS &&
		conversation.CreatedAtMS <= maxSafeJSONInteger &&
		conversation.UpdatedAtMS <= maxSafeJSONInteger
}

func conversationEntryTimestampsJSONSafe(entry model.OwnedConversationEntry) bool {
	return conversationTimestampsJSONSafe(entry.Conversation)
}

func conversationAggregateVersionJSONSafe(version uint64) bool {
	return version > 0 && version <= maxSafeJSONInteger
}

func conversationEntryJSONSafe(entry model.OwnedConversationEntry) bool {
	return conversationEntryTimestampsJSONSafe(entry) &&
		conversationAggregateVersionJSONSafe(entry.AggregateVersion)
}

func conversationPageJSONSafe(page model.OwnedConversationPage) bool {
	for _, entry := range page.Conversations {
		if !conversationEntryJSONSafe(entry) {
			return false
		}
	}
	return true
}

func conversationImportJSONSafe(result model.OwnedConversationImportResult) bool {
	return conversationTimestampsJSONSafe(result.Conversation) &&
		conversationAggregateVersionJSONSafe(result.AggregateVersion)
}

func conversationPromptAppendJSONSafe(prompt model.ConversationPrompt, aggregateVersion uint64) bool {
	return prompt.CreatedAtMS <= maxSafeJSONInteger &&
		conversationAggregateVersionJSONSafe(aggregateVersion)
}

// conversationPromptPageJSONSafe protects the HTTP boundary even when a
// backend implementation other than the Rust bridge is used. The bridge has
// its own equivalent validator, but the Coordinator must not serialize an
// unsafe or cross-Conversation page merely because the backend returned a
// typed value.
func conversationPromptPageJSONSafe(
	page model.ConversationPromptPage,
	conversationID string,
	before *model.PromptPageCursor,
	limit int,
) bool {
	if page.ConversationID != conversationID || page.Prompts == nil ||
		len(page.Prompts) > limit || page.HasMore != (page.NextCursor != nil) {
		return false
	}
	seen := make(map[string]struct{}, len(page.Prompts))
	contentBytes := 0
	for index, prompt := range page.Prompts {
		if !validTransportEntityID(prompt.ID) || prompt.ConversationID != conversationID ||
			prompt.Role == "" || len(prompt.Role) > 64 ||
			len(prompt.Content) > promptContentMaxBytes ||
			prompt.CreatedAtMS > maxSafeJSONInteger {
			return false
		}
		if _, exists := seen[prompt.ID]; exists {
			return false
		}
		seen[prompt.ID] = struct{}{}
		contentBytes += len(prompt.Content)
		if contentBytes > promptContentMaxBytes {
			return false
		}
		if index > 0 {
			previous := page.Prompts[index-1]
			if previous.CreatedAtMS < prompt.CreatedAtMS ||
				(previous.CreatedAtMS == prompt.CreatedAtMS && previous.ID <= prompt.ID) {
				return false
			}
		}
		if before != nil && (prompt.CreatedAtMS > before.CreatedAtMS ||
			(prompt.CreatedAtMS == before.CreatedAtMS && prompt.ID >= before.PromptID)) {
			return false
		}
	}
	if page.HasMore {
		if len(page.Prompts) == 0 || page.NextCursor == nil {
			return false
		}
		last := page.Prompts[len(page.Prompts)-1]
		return page.NextCursor.CreatedAtMS == last.CreatedAtMS &&
			page.NextCursor.PromptID == last.ID &&
			page.NextCursor.CreatedAtMS <= maxSafeJSONInteger &&
			validTransportEntityID(page.NextCursor.PromptID)
	}
	return page.NextCursor == nil
}

// conversationRunPageJSONSafe keeps the read-only Run summary route bounded
// and content-free at the final HTTP boundary.
func conversationRunPageJSONSafe(
	page runmodel.OwnedRunPage,
	conversationID string,
	before *runmodel.OwnedRunPageCursor,
	limit int,
) bool {
	if page.ConversationID != conversationID || page.Runs == nil || len(page.Runs) > limit ||
		page.HasMore != (page.NextCursor != nil) {
		return false
	}
	seen := make(map[string]struct{}, len(page.Runs))
	for index, run := range page.Runs {
		if !validTransportEntityID(run.RunID) || !validTransportEntityID(run.PromptID) ||
			run.CreatedAtMS > maxSafeJSONInteger || run.LatestSequence == 0 ||
			run.LatestSequence > maxSafeJSONInteger || !validTransportRunStatus(run.Status) {
			return false
		}
		if _, exists := seen[run.RunID]; exists {
			return false
		}
		seen[run.RunID] = struct{}{}
		if index > 0 {
			previous := page.Runs[index-1]
			if previous.CreatedAtMS < run.CreatedAtMS ||
				(previous.CreatedAtMS == run.CreatedAtMS && strings.Compare(previous.RunID, run.RunID) <= 0) {
				return false
			}
		}
		if before != nil && (run.CreatedAtMS > before.CreatedAtMS ||
			(run.CreatedAtMS == before.CreatedAtMS && strings.Compare(run.RunID, before.RunID) >= 0)) {
			return false
		}
	}
	if page.HasMore {
		if len(page.Runs) == 0 || page.NextCursor == nil {
			return false
		}
		last := page.Runs[len(page.Runs)-1]
		return page.NextCursor.CreatedAtMS == last.CreatedAtMS &&
			page.NextCursor.RunID == last.RunID &&
			page.NextCursor.CreatedAtMS <= maxSafeJSONInteger &&
			validTransportEntityID(page.NextCursor.RunID)
	}
	return page.NextCursor == nil
}

// conversationRunTimelineJSONSafe validates the forward-only, metadata-only
// Run timeline projection before it is exposed to an authenticated caller.
func conversationRunTimelineJSONSafe(
	page runmodel.OwnedRunTimelinePage,
	conversationID string,
	runID string,
	after uint64,
	limit int,
) bool {
	if page.ConversationID != conversationID || page.RunID != runID ||
		page.AfterSequence != after || page.ScannedThroughSequence < after ||
		page.ScannedThroughSequence > maxSafeJSONInteger || page.Events == nil ||
		len(page.Events) > limit || (page.HasMore && len(page.Events) == 0) {
		return false
	}
	previous := after
	for _, event := range page.Events {
		if previous == maxSafeJSONInteger || event.Sequence != previous+1 ||
			event.Sequence > maxSafeJSONInteger || event.EmittedAtMS > maxSafeJSONInteger ||
			!validTransportRunEventType(event.Type) {
			return false
		}
		previous = event.Sequence
	}
	if len(page.Events) == 0 {
		return !page.HasMore && page.ScannedThroughSequence == after
	}
	return page.ScannedThroughSequence == previous
}

func validTransportEntityID(value string) bool {
	return strings.TrimSpace(value) != "" && len(value) <= conversationIDMaxBytes
}

func validTransportRunStatus(value string) bool {
	switch value {
	case "nonterminal", "completed", "cancelled", "limit_exceeded", "failed":
		return true
	default:
		return false
	}
}

func validTransportRunEventType(value string) bool {
	switch value {
	case "run_started", "turn_started", "activity", "run_finished":
		return true
	default:
		return false
	}
}

func runObservedJSONSafe(
	observed auditprojection.RunObserved,
	owner model.Owner,
	conversationID string,
	runID string,
) bool {
	if observed.ConversationID != conversationID || observed.RunID != runID {
		return false
	}
	expected, err := auditprojection.ProjectRunObserved(owner, conversationID, runmodel.OwnedRunSummary{
		RunID:          observed.RunID,
		PromptID:       observed.PromptID,
		CreatedAtMS:    observed.CreatedAtMS,
		LatestSequence: observed.LatestSequence,
		Status:         observed.Status,
	})
	return err == nil && observed == expected
}

func conversationPageTimestampsJSONSafe(page model.OwnedConversationPage) bool {
	for _, entry := range page.Conversations {
		if !conversationEntryTimestampsJSONSafe(entry) {
			return false
		}
	}
	return true
}

func conversationChangePageJSONSafe(page model.OwnedConversationChangePage) bool {
	if page.AfterCursor > maxSafeJSONInteger || page.ScannedThroughCursor > maxSafeJSONInteger {
		return false
	}
	for _, change := range page.Changes {
		if change.Cursor > maxSafeJSONInteger ||
			uint64(change.SchemaVersion) > maxSafeJSONInteger ||
			change.AggregateVersion > maxSafeJSONInteger ||
			change.CreatedAtMS > maxSafeJSONInteger {
			return false
		}
	}
	return true
}
