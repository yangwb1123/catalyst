package model

// SnapshotAtCursor is a sanitized Hub bootstrap projection and its store-local cursor.
type SnapshotAtCursor struct {
	Snapshot Snapshot `json:"snapshot"`
	Cursor   uint64   `json:"cursor"`
}

// Snapshot contains Global Conversations without canonical project filesystem paths.
type Snapshot struct {
	Scope               ConversationScope    `json:"scope"`
	Projects            []Project            `json:"projects"`
	Conversations       []Conversation       `json:"conversations"`
	Groups              []SessionGroup       `json:"groups"`
	GroupProjectMembers []GroupProjectMember `json:"group_project_members"`
}

// ConversationScope is the closed Hub scope representation.
type ConversationScope struct {
	Kind string `json:"kind"`
	ID   string `json:"id,omitempty"`
}

// Project omits the canonical local path returned by the internal Hub model.
type Project struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	CreatedAtMS uint64 `json:"created_at_ms"`
}

// Conversation is one canonical Hub Conversation.
type Conversation struct {
	ID          string            `json:"id"`
	Scope       ConversationScope `json:"scope"`
	Title       string            `json:"title"`
	CreatedAtMS uint64            `json:"created_at_ms"`
	UpdatedAtMS uint64            `json:"updated_at_ms"`
}

// Owner is the exact issuer/subject/tenant tuple from a verified Snaplink token.
// It crosses the trusted local process boundary and is never sourced from an
// HTTP request body.
type Owner struct {
	Issuer   string `json:"issuer"`
	Subject  string `json:"subject"`
	TenantID string `json:"tenant_id"`
}

// OwnedConversationEntry is one sanitized owner-visible Conversation row.
type OwnedConversationEntry struct {
	Conversation     Conversation `json:"conversation"`
	AggregateVersion uint64       `json:"aggregate_version"`
}

// ConversationImportPrompt is one user-visible transcript entry accepted by
// the owner-scoped Conversation import operation.
type ConversationImportPrompt struct {
	Role    string `json:"role"`
	Content string `json:"content"`
}

// OwnedConversationImportResult is the receipt for one atomic transcript import.
type OwnedConversationImportResult struct {
	Conversation        Conversation `json:"conversation"`
	AggregateVersion    uint64       `json:"aggregate_version"`
	ImportedPromptCount int          `json:"imported_prompt_count"`
	Replayed            bool         `json:"replayed"`
}

type OwnedConversationPage struct {
	Conversations []OwnedConversationEntry `json:"conversations"`
	NextAfterID   *string                  `json:"next_after_id,omitempty"`
	HasMore       bool                     `json:"has_more"`
}

// OwnedProjectConversationIdentity contains only the Conversation and Project
// IDs needed by trusted server policy to resolve an execution profile.
type OwnedProjectConversationIdentity struct {
	ConversationID string `json:"conversation_id"`
	ProjectID      string `json:"project_id"`
}

// SessionGroup is the small Hub group read projection.
type SessionGroup struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	CreatedAtMS uint64 `json:"created_at_ms"`
}

// GroupProjectMember is the Hub's descriptive project membership projection.
type GroupProjectMember struct {
	GroupID   string `json:"group_id"`
	ProjectID string `json:"project_id"`
	Role      string `json:"role"`
	AddedAtMS uint64 `json:"added_at_ms"`
}

// ChangePage is a bounded page from the Hub-local change feed.
type ChangePage struct {
	AfterCursor uint64   `json:"after_cursor"`
	NextCursor  uint64   `json:"next_cursor"`
	HeadCursor  uint64   `json:"head_cursor"`
	HasMore     bool     `json:"has_more"`
	Changes     []Change `json:"changes"`
}

// PromptPageCursor is an exclusive position in newest-first Conversation history.
type PromptPageCursor struct {
	CreatedAtMS uint64 `json:"created_at_ms"`
	PromptID    string `json:"prompt_id"`
}

// ConversationPrompt is the sanitized body projection; it omits the idempotency key.
type ConversationPrompt struct {
	ID             string `json:"id"`
	ConversationID string `json:"conversation_id"`
	Role           string `json:"role"`
	Content        string `json:"content"`
	CreatedAtMS    uint64 `json:"created_at_ms"`
}

// ConversationPromptPage contains a bounded newest-first history page.
type ConversationPromptPage struct {
	ConversationID string               `json:"conversation_id"`
	Prompts        []ConversationPrompt `json:"prompts"`
	NextCursor     *PromptPageCursor    `json:"next_cursor,omitempty"`
	HasMore        bool                 `json:"has_more"`
}

// ConversationBootstrapPhase selects one indexed source for a frozen-head bootstrap.
type ConversationBootstrapPhase string

const (
	ConversationBootstrapLegacyBaseline ConversationBootstrapPhase = "legacy_baseline"
	ConversationBootstrapChangeLog      ConversationBootstrapPhase = "change_log"
)

// ConversationBootstrapCursor binds a continuation position to the first page's journal head.
type ConversationBootstrapCursor struct {
	SnapshotCursor      uint64                     `json:"snapshot_cursor"`
	Phase               ConversationBootstrapPhase `json:"phase"`
	AfterConversationID *string                    `json:"after_conversation_id,omitempty"`
	AfterChangeCursor   *uint64                    `json:"after_change_cursor,omitempty"`
}

// ConversationBootstrapEntry is sanitized Conversation metadata and its Hub versions.
type ConversationBootstrapEntry struct {
	Conversation     Conversation `json:"conversation"`
	CreationCursor   uint64       `json:"creation_cursor"`
	AggregateVersion uint64       `json:"aggregate_version"`
}

// ConversationBootstrapPage is one bounded page through a frozen change head.
type ConversationBootstrapPage struct {
	SnapshotCursor       uint64                       `json:"snapshot_cursor"`
	Conversations        []ConversationBootstrapEntry `json:"conversations"`
	ScannedThroughCursor uint64                       `json:"scanned_through_cursor"`
	NextCursor           *ConversationBootstrapCursor `json:"next_cursor,omitempty"`
	HasMore              bool                         `json:"has_more"`
}

// Change contains the Hub's ID-only Conversation or Prompt reference.
type Change struct {
	Cursor           uint64 `json:"cursor"`
	SchemaVersion    uint16 `json:"schema_version"`
	ConversationID   string `json:"conversation_id"`
	EntityID         string `json:"entity_id"`
	AggregateVersion uint64 `json:"aggregate_version"`
	Kind             string `json:"kind"`
	CreatedAtMS      uint64 `json:"created_at_ms"`
}

// OwnedConversationChangePage is a bounded replay page containing only one
// verified owner's Conversation changes. Cursors are dense within that owner.
type OwnedConversationChangePage struct {
	AfterCursor          uint64   `json:"after_cursor"`
	ScannedThroughCursor uint64   `json:"scanned_through_cursor"`
	HasMore              bool     `json:"has_more"`
	Changes              []Change `json:"changes"`
}
