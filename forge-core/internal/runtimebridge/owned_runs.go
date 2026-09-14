package runtimebridge

import (
	"context"
	"encoding/json"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
	"strings"
)

// OwnedConversationRuns lists only Runs attached to a model.Conversation owned by the exact principal.
func (client *Client) OwnedConversationRuns(
	ctx context.Context,
	owner model.Owner,
	conversationID string,
	before *runmodel.OwnedRunPageCursor,
	limit int,
) (runmodel.OwnedRunPage, error) {
	if !validOwner(owner) || !validEntityID(conversationID) || limit < 1 || limit > maxOwnedRunPageLimit ||
		(before != nil && (before.CreatedAtMS > maxSQLiteInteger || !validEntityID(before.RunID))) {
		return runmodel.OwnedRunPage{}, &Error{Code: "invalid_owned_run_request"}
	}
	ownerCopy := owner
	request := request{
		Operation: "owned_run_page", Owner: &ownerCopy, ConversationID: conversationID, Limit: &limit,
	}
	if before != nil {
		createdAtMS, runID := before.CreatedAtMS, before.RunID
		request.BeforeCreatedAtMS, request.BeforeRunID = &createdAtMS, &runID
	}
	response, err := client.callWrite(ctx, request)
	if err != nil {
		return runmodel.OwnedRunPage{}, err
	}
	var page runmodel.OwnedRunPage
	if err := decodeStrict(response, &page); err != nil ||
		!validOwnedRunPage(response, page, conversationID, before, limit) {
		return runmodel.OwnedRunPage{}, &Error{Code: "invalid_runtime_response"}
	}
	return page, nil
}

// OwnedConversationRunTimeline reads sanitized event envelopes after the requested sequence.
func (client *Client) OwnedConversationRunTimeline(
	ctx context.Context,
	owner model.Owner,
	conversationID string,
	runID string,
	after uint64,
	limit int,
) (runmodel.OwnedRunTimelinePage, error) {
	if !validOwner(owner) || !validEntityID(conversationID) || !validEntityID(runID) ||
		after > maxSQLiteInteger || limit < 1 || limit > maxOwnedRunTimelinePageLimit {
		return runmodel.OwnedRunTimelinePage{}, &Error{Code: "invalid_owned_run_request"}
	}
	ownerCopy := owner
	request := request{
		Operation: "owned_run_timeline_page", Owner: &ownerCopy, ConversationID: conversationID,
		RunID: runID, AfterSequence: &after, Limit: &limit,
	}
	response, err := client.callWrite(ctx, request)
	if err != nil {
		return runmodel.OwnedRunTimelinePage{}, err
	}
	var page runmodel.OwnedRunTimelinePage
	if err := decodeStrict(response, &page); err != nil ||
		!validOwnedRunTimelinePage(response, page, conversationID, runID, after, limit) {
		return runmodel.OwnedRunTimelinePage{}, &Error{Code: "invalid_runtime_response"}
	}
	return page, nil
}

func validOwnedRunPage(
	data []byte,
	page runmodel.OwnedRunPage,
	conversationID string,
	before *runmodel.OwnedRunPageCursor,
	limit int,
) bool {
	fields := []string{"conversation_id", "runs", "has_more"}
	if page.NextCursor != nil {
		fields = append(fields, "next_cursor")
	}
	if requireObjectFieldSet(data, fields...) != nil || page.ConversationID != conversationID ||
		page.Runs == nil || len(page.Runs) > limit || page.HasMore != (page.NextCursor != nil) {
		return false
	}
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil {
		return false
	}
	var rawRuns []json.RawMessage
	if json.Unmarshal(root["runs"], &rawRuns) != nil || len(rawRuns) != len(page.Runs) {
		return false
	}
	for index, run := range page.Runs {
		if requireObjectFieldSet(rawRuns[index], "run_id", "prompt_id", "created_at_ms", "latest_sequence", "status") != nil ||
			!validEntityID(run.RunID) || !validEntityID(run.PromptID) || run.CreatedAtMS > maxSQLiteInteger ||
			run.LatestSequence == 0 || run.LatestSequence > maxSQLiteInteger || !validOwnedRunStatus(run.Status) {
			return false
		}
		if index > 0 && !newerRun(page.Runs[index-1], run) {
			return false
		}
		if before != nil && !olderThanRunCursor(run, *before) {
			return false
		}
	}
	if page.HasMore {
		if len(page.Runs) == 0 || page.NextCursor == nil {
			return false
		}
		last := page.Runs[len(page.Runs)-1]
		if page.NextCursor.CreatedAtMS != last.CreatedAtMS || page.NextCursor.RunID != last.RunID {
			return false
		}
		return requireObjectFieldSet(root["next_cursor"], "created_at_ms", "run_id") == nil &&
			page.NextCursor.CreatedAtMS <= maxSQLiteInteger && validEntityID(page.NextCursor.RunID)
	}
	return page.NextCursor == nil
}

func validOwnedRunTimelinePage(
	data []byte,
	page runmodel.OwnedRunTimelinePage,
	conversationID string,
	runID string,
	after uint64,
	limit int,
) bool {
	if requireObjectFieldSet(data, "conversation_id", "run_id", "after_sequence", "scanned_through_sequence", "has_more", "events") != nil ||
		page.ConversationID != conversationID || page.RunID != runID || page.AfterSequence != after ||
		page.ScannedThroughSequence < after || page.ScannedThroughSequence > maxSQLiteInteger ||
		page.Events == nil || len(page.Events) > limit || (page.HasMore && len(page.Events) == 0) {
		return false
	}
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil {
		return false
	}
	var rawEvents []json.RawMessage
	if json.Unmarshal(root["events"], &rawEvents) != nil || len(rawEvents) != len(page.Events) {
		return false
	}
	previous := after
	for index, event := range page.Events {
		if requireObjectFieldSet(rawEvents[index], "seq", "emitted_at_ms", "type") != nil ||
			event.Sequence != previous+1 || event.Sequence > maxSQLiteInteger ||
			event.EmittedAtMS > maxSQLiteInteger || !validOwnedRunEventType(event.Type) {
			return false
		}
		previous = event.Sequence
	}
	if len(page.Events) == 0 {
		return !page.HasMore && page.ScannedThroughSequence == after
	}
	return page.ScannedThroughSequence == previous
}

func validOwnedRunStatus(status string) bool {
	switch status {
	case "nonterminal", "completed", "cancelled", "limit_exceeded", "failed":
		return true
	default:
		return false
	}
}

func validOwnedRunEventType(eventType string) bool {
	switch eventType {
	case "run_started", "turn_started", "activity", "run_finished":
		return true
	default:
		return false
	}
}

func newerRun(left, right runmodel.OwnedRunSummary) bool {
	return left.CreatedAtMS > right.CreatedAtMS ||
		(left.CreatedAtMS == right.CreatedAtMS && strings.Compare(left.RunID, right.RunID) > 0)
}

func olderThanRunCursor(run runmodel.OwnedRunSummary, cursor runmodel.OwnedRunPageCursor) bool {
	return run.CreatedAtMS < cursor.CreatedAtMS ||
		(run.CreatedAtMS == cursor.CreatedAtMS && strings.Compare(run.RunID, cursor.RunID) < 0)
}
