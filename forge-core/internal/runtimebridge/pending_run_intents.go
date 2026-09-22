package runtimebridge

import (
	"context"
	"encoding/json"
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"strings"
	"unicode"
)

// SubmitOwnedPromptRunIntent writes a Prompt and a consent-checked pending
// intent atomically. The owner and profile are trusted internal inputs. A
// replay returns the original receipt and profile ID without reinterpreting
// current consent, CAS state, or profile configuration.
func (client *Client) SubmitOwnedPromptRunIntent(
	ctx context.Context,
	owner model.Owner,
	conversationID string,
	content string,
	idempotencyKey string,
	expectedVersion uint64,
	profile intentmodel.ServerExecutionProfile,
) (intentmodel.PendingRunIntentSubmissionResult, error) {
	if !validOwner(owner) || !validEntityID(conversationID) || strings.TrimSpace(content) == "" ||
		len(content) > maxPromptContentBytes || !validPendingIntentIdempotencyKey(idempotencyKey) ||
		expectedVersion > maxSafeJSONInteger || !validEntityID(profile.ID) {
		return intentmodel.PendingRunIntentSubmissionResult{}, &Error{Code: "invalid_owned_prompt_request"}
	}
	ownerCopy, versionCopy, digestCopy := owner, expectedVersion, profile.SHA256
	response, err := client.callWrite(ctx, request{
		Operation: "submit_owned_prompt_run_intent", Owner: &ownerCopy,
		ConversationID: conversationID, Content: content, IdempotencyKey: idempotencyKey,
		ExpectedVersion: &versionCopy, ProfileID: profile.ID, ProfileSHA256: &digestCopy,
	})
	if err != nil {
		return intentmodel.PendingRunIntentSubmissionResult{}, err
	}
	var result intentmodel.PendingRunIntentSubmissionResult
	if err := decodeStrict(response, &result); err != nil ||
		!validPendingRunIntentSubmission(response, result, conversationID, content, profile.ID) {
		return intentmodel.PendingRunIntentSubmissionResult{}, &Error{Code: "invalid_runtime_response"}
	}
	return result, nil
}

// OwnedConversationPendingRunIntents lists sanitized pending intents after an
// exact owner check in Hub.
func (client *Client) OwnedConversationPendingRunIntents(
	ctx context.Context,
	owner model.Owner,
	conversationID string,
	before *intentmodel.PendingRunIntentCursor,
	limit int,
) (intentmodel.OwnedPendingRunIntentPage, error) {
	if !validOwner(owner) || !validEntityID(conversationID) || limit < 1 || limit > maxOwnedRunPageLimit ||
		(before != nil && !validPendingRunIntentCursor(*before)) {
		return intentmodel.OwnedPendingRunIntentPage{}, &Error{Code: "invalid_pending_run_intent_request"}
	}
	ownerCopy, limitCopy := owner, limit
	response, err := client.callWrite(ctx, request{
		Operation: "owned_pending_run_intent_page", Owner: &ownerCopy,
		ConversationID: conversationID, Before: before, Limit: &limitCopy,
	})
	if err != nil {
		return intentmodel.OwnedPendingRunIntentPage{}, err
	}
	var page intentmodel.OwnedPendingRunIntentPage
	if err := decodeStrict(response, &page); err != nil ||
		!validOwnedPendingRunIntentPage(response, page, conversationID, before, limit) {
		return intentmodel.OwnedPendingRunIntentPage{}, &Error{Code: "invalid_runtime_response"}
	}
	return page, nil
}

// OwnedConversationPendingRunIntentTimeline returns only event envelope
// metadata after Hub checks exact owner, model.Conversation, and intent identity.
func (client *Client) OwnedConversationPendingRunIntentTimeline(
	ctx context.Context,
	owner model.Owner,
	conversationID string,
	intentID string,
	afterSequence uint64,
	limit int,
) (intentmodel.OwnedPendingRunIntentTimelinePage, error) {
	if !validOwner(owner) || !validEntityID(conversationID) || !validEntityID(intentID) ||
		afterSequence > maxSafeJSONInteger || limit < 1 || limit > maxOwnedRunTimelinePageLimit {
		return intentmodel.OwnedPendingRunIntentTimelinePage{}, &Error{Code: "invalid_pending_run_intent_request"}
	}
	ownerCopy, afterCopy, limitCopy := owner, afterSequence, limit
	response, err := client.callWrite(ctx, request{
		Operation: "owned_pending_run_intent_timeline_page", Owner: &ownerCopy,
		ConversationID: conversationID, IntentID: intentID,
		AfterSequence: &afterCopy, Limit: &limitCopy,
	})
	if err != nil {
		return intentmodel.OwnedPendingRunIntentTimelinePage{}, err
	}
	var page intentmodel.OwnedPendingRunIntentTimelinePage
	if err := decodeStrict(response, &page); err != nil ||
		!validOwnedPendingRunIntentTimelinePage(response, page, conversationID, intentID, afterSequence, limit) {
		return intentmodel.OwnedPendingRunIntentTimelinePage{}, &Error{Code: "invalid_runtime_response"}
	}
	return page, nil
}

func validPendingIntentIdempotencyKey(value string) bool {
	if strings.TrimSpace(value) == "" || len(value) > 256 {
		return false
	}
	return !strings.ContainsFunc(value, unicode.IsControl)
}

func validPendingRunIntentCursor(cursor intentmodel.PendingRunIntentCursor) bool {
	return cursor.SubmittedAtMS <= maxSafeJSONInteger && validEntityID(cursor.IntentID)
}

func validPendingRunIntent(intent intentmodel.PendingRunIntent, conversationID string) bool {
	return validEntityID(intent.IntentID) && intent.ConversationID == conversationID &&
		validEntityID(intent.PromptID) && validEntityID(intent.ProjectID) && validEntityID(intent.ProfileID) &&
		intent.SubmittedAtMS <= maxSafeJSONInteger && intent.AggregateVersion > 0 &&
		intent.AggregateVersion <= maxSafeJSONInteger && intent.LatestSequence == 1 && intent.Status == "pending"
}

func validPendingRunIntentSubmission(
	data []byte,
	result intentmodel.PendingRunIntentSubmissionResult,
	conversationID string,
	content string,
	requestedProfileID string,
) bool {
	if requireObjectFieldSet(data, "prompt", "intent", "initial_event", "replayed") != nil ||
		!validPromptForPendingIntent(result.Prompt, conversationID, content) ||
		!validPendingRunIntent(result.Intent, conversationID) || result.Intent.PromptID != result.Prompt.ID ||
		result.Prompt.CreatedAtMS != result.Intent.SubmittedAtMS ||
		(!result.Replayed && result.Intent.ProfileID != requestedProfileID) ||
		!validPendingRunIntentEvent(result.InitialEvent) || result.InitialEvent.Sequence != 1 ||
		result.InitialEvent.Type != "submitted" || result.InitialEvent.EmittedAtMS != result.Intent.SubmittedAtMS {
		return false
	}
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil ||
		requireObjectFieldSet(root["prompt"], "id", "conversation_id", "role", "content", "created_at_ms") != nil ||
		requireObjectFieldSet(root["intent"], "intent_id", "conversation_id", "prompt_id", "project_id", "profile_id",
			"submitted_at_ms", "aggregate_version", "latest_sequence", "status") != nil {
		return false
	}
	return requireObjectFieldSet(root["initial_event"], "event_id", "seq", "emitted_at_ms", "type") == nil
}

func validPromptForPendingIntent(prompt model.ConversationPrompt, conversationID, content string) bool {
	return validEntityID(prompt.ID) && prompt.ConversationID == conversationID && prompt.Role == "user" &&
		prompt.Content == content && prompt.CreatedAtMS <= maxSafeJSONInteger
}

func validPendingRunIntentEvent(event intentmodel.PendingRunIntentEventSummary) bool {
	return validEntityID(event.EventID) && event.EmittedAtMS <= maxSafeJSONInteger && event.Type == "submitted"
}

func validOwnedPendingRunIntentPage(
	data []byte,
	page intentmodel.OwnedPendingRunIntentPage,
	conversationID string,
	before *intentmodel.PendingRunIntentCursor,
	limit int,
) bool {
	fields := []string{"conversation_id", "intents", "has_more"}
	if page.NextCursor != nil {
		fields = append(fields, "next_cursor")
	}
	if requireObjectFieldSet(data, fields...) != nil || page.ConversationID != conversationID ||
		page.Intents == nil || len(page.Intents) > limit || page.HasMore != (page.NextCursor != nil) {
		return false
	}
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil {
		return false
	}
	var rawIntents []json.RawMessage
	if json.Unmarshal(root["intents"], &rawIntents) != nil || len(rawIntents) != len(page.Intents) {
		return false
	}
	for index, intent := range page.Intents {
		if requireObjectFieldSet(rawIntents[index], "intent_id", "conversation_id", "prompt_id", "project_id", "profile_id",
			"submitted_at_ms", "aggregate_version", "latest_sequence", "status") != nil ||
			!validPendingRunIntent(intent, conversationID) {
			return false
		}
		if index > 0 && !newerPendingRunIntent(page.Intents[index-1], intent) {
			return false
		}
		if before != nil && !olderThanPendingRunIntentCursor(intent, *before) {
			return false
		}
	}
	if page.HasMore {
		if len(page.Intents) != limit || page.NextCursor == nil {
			return false
		}
		last := page.Intents[len(page.Intents)-1]
		return page.NextCursor.SubmittedAtMS == last.SubmittedAtMS && page.NextCursor.IntentID == last.IntentID &&
			requireObjectFieldSet(root["next_cursor"], "submitted_at_ms", "intent_id") == nil &&
			validPendingRunIntentCursor(*page.NextCursor)
	}
	return page.NextCursor == nil
}

func validOwnedPendingRunIntentTimelinePage(
	data []byte,
	page intentmodel.OwnedPendingRunIntentTimelinePage,
	conversationID string,
	intentID string,
	afterSequence uint64,
	limit int,
) bool {
	if requireObjectFieldSet(data, "conversation_id", "intent_id", "after_sequence", "scanned_through_sequence", "has_more", "events") != nil ||
		page.ConversationID != conversationID || page.IntentID != intentID || page.AfterSequence != afterSequence ||
		page.AfterSequence > maxSafeJSONInteger || page.ScannedThroughSequence > maxSafeJSONInteger ||
		page.Events == nil || len(page.Events) > limit || page.HasMore {
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
	for index, event := range page.Events {
		if requireObjectFieldSet(rawEvents[index], "event_id", "seq", "emitted_at_ms", "type") != nil ||
			!validPendingRunIntentEvent(event) || event.Sequence != 1 {
			return false
		}
	}
	if afterSequence == 0 {
		return len(page.Events) == 1 && page.ScannedThroughSequence == 1
	}
	return len(page.Events) == 0 && page.ScannedThroughSequence == afterSequence
}

func newerPendingRunIntent(left, right intentmodel.PendingRunIntent) bool {
	return left.SubmittedAtMS > right.SubmittedAtMS ||
		(left.SubmittedAtMS == right.SubmittedAtMS && left.IntentID > right.IntentID)
}

func olderThanPendingRunIntentCursor(intent intentmodel.PendingRunIntent, cursor intentmodel.PendingRunIntentCursor) bool {
	return intent.SubmittedAtMS < cursor.SubmittedAtMS ||
		(intent.SubmittedAtMS == cursor.SubmittedAtMS && intent.IntentID < cursor.IntentID)
}
