// Package runmodel contains the sanitized owner-scoped Run projections carried
// across the Runtime process boundary.
package runmodel

// OwnedRunPageCursor is the exclusive position for a newest-first Run page.
type OwnedRunPageCursor struct {
	CreatedAtMS uint64 `json:"created_at_ms"`
	RunID       string `json:"run_id"`
}

// OwnedRunSummary deliberately excludes execution configuration and journal bodies.
type OwnedRunSummary struct {
	RunID          string `json:"run_id"`
	PromptID       string `json:"prompt_id"`
	CreatedAtMS    uint64 `json:"created_at_ms"`
	LatestSequence uint64 `json:"latest_sequence"`
	Status         string `json:"status"`
}

// OwnedRunPage is a bounded page from one owner-bound Conversation.
type OwnedRunPage struct {
	ConversationID string              `json:"conversation_id"`
	Runs           []OwnedRunSummary   `json:"runs"`
	NextCursor     *OwnedRunPageCursor `json:"next_cursor,omitempty"`
	HasMore        bool                `json:"has_more"`
}

// OwnedRunEventSummary contains only the closed event kind and its envelope metadata.
type OwnedRunEventSummary struct {
	Sequence    uint64 `json:"seq"`
	EmittedAtMS uint64 `json:"emitted_at_ms"`
	Type        string `json:"type"`
}

// OwnedRunTimelinePage is a bounded, forward-only page of sanitized Run event metadata.
type OwnedRunTimelinePage struct {
	ConversationID         string                 `json:"conversation_id"`
	RunID                  string                 `json:"run_id"`
	AfterSequence          uint64                 `json:"after_sequence"`
	ScannedThroughSequence uint64                 `json:"scanned_through_sequence"`
	HasMore                bool                   `json:"has_more"`
	Events                 []OwnedRunEventSummary `json:"events"`
}
