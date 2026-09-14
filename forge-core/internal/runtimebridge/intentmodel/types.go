// Package intentmodel contains the sanitized pending-run-intent DTOs. An
// intent is a receipt and does not imply that execution has started.
package intentmodel

import "forgeos/forge-core/internal/runtimebridge/model"

// ServerExecutionProfile is an opaque profile identity selected by trusted
// server policy. Callers must not populate it from an HTTP request.
type ServerExecutionProfile struct {
	ID     string
	SHA256 [32]byte
}

// PendingRunIntentCursor is the exclusive position for a newest-first intent page.
type PendingRunIntentCursor struct {
	SubmittedAtMS uint64 `json:"submitted_at_ms"`
	IntentID      string `json:"intent_id"`
}

// PendingRunIntent contains the sanitized receipt for one consent-checked intent.
type PendingRunIntent struct {
	IntentID         string `json:"intent_id"`
	ConversationID   string `json:"conversation_id"`
	PromptID         string `json:"prompt_id"`
	ProjectID        string `json:"project_id"`
	ProfileID        string `json:"profile_id"`
	SubmittedAtMS    uint64 `json:"submitted_at_ms"`
	AggregateVersion uint64 `json:"aggregate_version"`
	LatestSequence   uint64 `json:"latest_sequence"`
	Status           string `json:"status"`
}

// PendingRunIntentEventSummary exposes only the immutable event envelope.
type PendingRunIntentEventSummary struct {
	EventID     string `json:"event_id"`
	Sequence    uint64 `json:"seq"`
	EmittedAtMS uint64 `json:"emitted_at_ms"`
	Type        string `json:"type"`
}

// PendingRunIntentSubmissionResult is the original immutable receipt returned
// by submit, including on idempotent replay under changed current policy.
type PendingRunIntentSubmissionResult struct {
	Prompt       model.ConversationPrompt     `json:"prompt"`
	Intent       PendingRunIntent             `json:"intent"`
	InitialEvent PendingRunIntentEventSummary `json:"initial_event"`
	Replayed     bool                         `json:"replayed"`
}

// OwnedPendingRunIntentPage is one bounded owner-scoped pending intent page.
type OwnedPendingRunIntentPage struct {
	ConversationID string                  `json:"conversation_id"`
	Intents        []PendingRunIntent      `json:"intents"`
	NextCursor     *PendingRunIntentCursor `json:"next_cursor,omitempty"`
	HasMore        bool                    `json:"has_more"`
}

// OwnedPendingRunIntentTimelinePage is one bounded page of payload-free intent events.
type OwnedPendingRunIntentTimelinePage struct {
	ConversationID         string                         `json:"conversation_id"`
	IntentID               string                         `json:"intent_id"`
	AfterSequence          uint64                         `json:"after_sequence"`
	ScannedThroughSequence uint64                         `json:"scanned_through_sequence"`
	HasMore                bool                           `json:"has_more"`
	Events                 []PendingRunIntentEventSummary `json:"events"`
}
