package appserver

import (
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

func pendingIntentNumericJSONSafe(intent intentmodel.PendingRunIntent) bool {
	return intent.SubmittedAtMS <= maxSafeJSONInteger &&
		intent.AggregateVersion > 0 && intent.AggregateVersion <= maxSafeJSONInteger &&
		intent.LatestSequence > 0 && intent.LatestSequence <= maxSafeJSONInteger
}

func pendingIntentEventNumericJSONSafe(event intentmodel.PendingRunIntentEventSummary) bool {
	return event.Sequence > 0 && event.Sequence <= maxSafeJSONInteger &&
		event.EmittedAtMS <= maxSafeJSONInteger
}

func pendingIntentPromptNumericJSONSafe(prompt model.ConversationPrompt) bool {
	return prompt.CreatedAtMS <= maxSafeJSONInteger
}

func pendingIntentSubmissionJSONSafe(
	result intentmodel.PendingRunIntentSubmissionResult,
	expectedVersion uint64,
) bool {
	return pendingIntentPromptNumericJSONSafe(result.Prompt) &&
		pendingIntentNumericJSONSafe(result.Intent) &&
		pendingIntentEventNumericJSONSafe(result.InitialEvent) &&
		(result.Replayed ||
			(expectedVersion < maxSafeJSONInteger && result.Intent.AggregateVersion == expectedVersion+1))
}

func pendingIntentPageJSONSafe(page intentmodel.OwnedPendingRunIntentPage) bool {
	for _, intent := range page.Intents {
		if !pendingIntentNumericJSONSafe(intent) {
			return false
		}
	}
	return page.NextCursor == nil || page.NextCursor.SubmittedAtMS <= maxSafeJSONInteger
}

func pendingIntentTimelineJSONSafe(page intentmodel.OwnedPendingRunIntentTimelinePage) bool {
	if page.AfterSequence > maxSafeJSONInteger || page.ScannedThroughSequence > maxSafeJSONInteger {
		return false
	}
	for _, event := range page.Events {
		if !pendingIntentEventNumericJSONSafe(event) {
			return false
		}
	}
	return true
}
