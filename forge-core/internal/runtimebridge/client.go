package runtimebridge

import (
	"context"
	"encoding/json"
	"errors"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"os"
	"path/filepath"
	"strings"
	"time"
)

const (
	protocolVersion                   = "forgeos.runtime-bridge/v1"
	writeProtocolVersion              = "forgeos.runtime-bridge/v2"
	maxRequestBytes                   = 16 * 1024
	maxWriteRequestBytes              = 2 * 1024 * 1024
	maxResponseBytes                  = 2 * 1024 * 1024
	maxChangeLimit                    = 128
	maxPromptPageLimit                = 128
	maxBootstrapPageLimit             = 128
	maxOwnedRunPageLimit              = 25
	maxOwnedRunTimelinePageLimit      = 128
	maxPromptContentBytes             = 256 * 1024
	maxConversationImportPromptCount  = 128
	maxConversationImportContentBytes = 256 * 1024
	maxEntityIDBytes                  = 128
	maxRoleBytes                      = 64
	maxSQLiteInteger                  = uint64(1<<63 - 1)
	maxSafeJSONInteger                = uint64(1<<53 - 1)
	defaultTimeout                    = 5 * time.Second
	maxTimeout                        = 30 * time.Second
	childWaitDelay                    = 150 * time.Millisecond
	maxJSONDepth                      = 64
)

// Config identifies one trusted direct Runtime executable and its separate Hub state directory.
// The configured executable must not be a wrapper that launches child processes.
type Config struct {
	Executable        string
	AppServerStateDir string
	RuntimeStateDir   string
	Timeout           time.Duration
}

// Client issues bounded owner-scoped reads and writes to the Rust-owned Hub process.
type Client struct {
	executable string
	database   string
	timeout    time.Duration
}

// Error contains only a stable, non-sensitive failure code.
type Error struct {
	Code string
}

func (e *Error) Error() string {
	return "Runtime bridge request failed: " + e.Code
}

type request struct {
	APIVersion        string                             `json:"api_version"`
	RequestID         string                             `json:"request_id"`
	Operation         string                             `json:"operation"`
	AfterCursor       *uint64                            `json:"after_cursor,omitempty"`
	AfterID           *string                            `json:"after_id,omitempty"`
	Limit             *int                               `json:"limit,omitempty"`
	ConversationID    string                             `json:"conversation_id,omitempty"`
	IntentID          string                             `json:"intent_id,omitempty"`
	Before            any                                `json:"before,omitempty"`
	BeforeCreatedAtMS *uint64                            `json:"before_created_at_ms,omitempty"`
	BeforeRunID       *string                            `json:"before_run_id,omitempty"`
	AfterSequence     *uint64                            `json:"after_sequence,omitempty"`
	RunID             string                             `json:"run_id,omitempty"`
	Cursor            *model.ConversationBootstrapCursor `json:"cursor,omitempty"`
	Owner             *model.Owner                       `json:"owner,omitempty"`
	Scope             *model.ConversationScope           `json:"scope,omitempty"`
	Title             string                             `json:"title,omitempty"`
	Prompts           *[]model.ConversationImportPrompt  `json:"prompts,omitempty"`
	ProjectID         string                             `json:"project_id,omitempty"`
	GrantID           string                             `json:"grant_id,omitempty"`
	ExpiresAtMS       *uint64                            `json:"expires_at_ms,omitempty"`
	IdempotencyKey    string                             `json:"idempotency_key,omitempty"`
	Content           string                             `json:"content,omitempty"`
	ExpectedVersion   *uint64                            `json:"expected_version,omitempty"`
	ProfileID         string                             `json:"profile_id,omitempty"`
	ProfileSHA256     *[32]byte                          `json:"profile_sha256,omitempty"`
}

type ownedPromptAppendResult struct {
	Prompt           model.ConversationPrompt `json:"prompt"`
	AggregateVersion uint64                   `json:"aggregate_version"`
	Replayed         bool                     `json:"replayed"`
}

type envelope struct {
	APIVersion string          `json:"api_version"`
	RequestID  string          `json:"request_id"`
	OK         bool            `json:"ok"`
	Result     json.RawMessage `json:"result"`
	Error      *remoteError    `json:"error,omitempty"`
}

type remoteError struct {
	Code    string `json:"code"`
	Message string `json:"message"`
}

// New validates the configured process and keeps Runtime state separate from Go state.
func New(config Config) (*Client, error) {
	executable, err := cleanAbsolutePath(config.Executable)
	if err != nil {
		return nil, &Error{Code: "invalid_configuration"}
	}
	appState, err := existingDirectory(config.AppServerStateDir)
	if err != nil {
		return nil, &Error{Code: "invalid_configuration"}
	}
	runtimeState, err := existingDirectory(config.RuntimeStateDir)
	if err != nil || pathsOverlap(appState, runtimeState) {
		return nil, &Error{Code: "invalid_configuration"}
	}
	timeout := config.Timeout
	if timeout == 0 {
		timeout = defaultTimeout
	}
	if timeout < time.Millisecond || timeout > maxTimeout {
		return nil, &Error{Code: "invalid_configuration"}
	}
	return &Client{
		executable: executable,
		database:   filepath.Join(runtimeState, "hub.sqlite3"),
		timeout:    timeout,
	}, nil
}

// model.SnapshotAtCursor reads the sanitized Global Hub snapshot and its matching store cursor.
func (c *Client) SnapshotAtCursor(ctx context.Context) (model.SnapshotAtCursor, error) {
	var result model.SnapshotAtCursor
	response, err := c.call(ctx, request{Operation: "snapshot_at_cursor"})
	if err != nil {
		return result, err
	}
	if err := decodeStrict(response, &result); err != nil || result.Snapshot.Scope.Kind != "global" ||
		!validSnapshot(response, result) {
		return model.SnapshotAtCursor{}, &Error{Code: "invalid_runtime_response"}
	}
	return result, nil
}

// ChangesAfter reads one contiguous, bounded page from the Hub-local change cursor.
func (c *Client) ChangesAfter(ctx context.Context, after uint64, limit int) (model.ChangePage, error) {
	if after > maxSafeJSONInteger {
		return model.ChangePage{}, &Error{Code: "invalid_cursor"}
	}
	if limit < 1 || limit > maxChangeLimit {
		return model.ChangePage{}, &Error{Code: "invalid_limit"}
	}
	response, err := c.call(ctx, request{
		Operation:   "conversation_changes_after",
		AfterCursor: &after,
		Limit:       &limit,
	})
	if err != nil {
		return model.ChangePage{}, err
	}
	var page model.ChangePage
	if err := decodeStrict(response, &page); err != nil || !validPage(page, after, limit) ||
		!validChangeFields(response, page) {
		return model.ChangePage{}, &Error{Code: "invalid_runtime_response"}
	}
	return page, nil
}

// ConversationPrompts reads one byte-budgeted page from exactly one model.Conversation.
func (c *Client) ConversationPrompts(
	ctx context.Context,
	conversationID string,
	before *model.PromptPageCursor,
	limit int,
) (model.ConversationPromptPage, error) {
	if strings.TrimSpace(conversationID) == "" || len(conversationID) > maxEntityIDBytes {
		return model.ConversationPromptPage{}, &Error{Code: "invalid_conversation_id"}
	}
	if limit < 1 || limit > maxPromptPageLimit {
		return model.ConversationPromptPage{}, &Error{Code: "invalid_limit"}
	}
	if before != nil && (before.CreatedAtMS > maxSafeJSONInteger ||
		strings.TrimSpace(before.PromptID) == "" || len(before.PromptID) > maxEntityIDBytes) {
		return model.ConversationPromptPage{}, &Error{Code: "invalid_cursor"}
	}
	response, err := c.call(ctx, request{
		Operation:      "conversation_prompt_page",
		ConversationID: conversationID,
		Before:         before,
		Limit:          &limit,
	})
	if err != nil {
		return model.ConversationPromptPage{}, err
	}
	var page model.ConversationPromptPage
	if err := decodeStrict(response, &page); err != nil ||
		!validPromptPage(response, page, conversationID, before, limit) {
		return model.ConversationPromptPage{}, &Error{Code: "invalid_runtime_response"}
	}
	return page, nil
}

// ConversationBootstrap reads one bounded page of sanitized model.Conversation metadata.
// The first page captures the journal head; later pages must echo its cursor.
func (c *Client) ConversationBootstrap(
	ctx context.Context,
	cursor *model.ConversationBootstrapCursor,
	limit int,
) (model.ConversationBootstrapPage, error) {
	if limit < 1 || limit > maxBootstrapPageLimit {
		return model.ConversationBootstrapPage{}, &Error{Code: "invalid_limit"}
	}
	if cursor != nil && !validConversationBootstrapCursor(*cursor) {
		return model.ConversationBootstrapPage{}, &Error{Code: "invalid_cursor"}
	}
	response, err := c.call(ctx, request{
		Operation: "conversation_bootstrap_page",
		Cursor:    cursor,
		Limit:     &limit,
	})
	if err != nil {
		return model.ConversationBootstrapPage{}, err
	}
	var page model.ConversationBootstrapPage
	if err := decodeStrict(response, &page); err != nil ||
		!validConversationBootstrapPage(response, page, cursor, limit) {
		return model.ConversationBootstrapPage{}, &Error{Code: "invalid_runtime_response"}
	}
	return page, nil
}

// CreateOwnedConversation atomically creates a model.Conversation and binds it to a
// verified Snaplink principal in the Runtime Hub.
func (c *Client) CreateOwnedConversation(
	ctx context.Context,
	owner model.Owner,
	scope model.ConversationScope,
	title string,
	idempotencyKey string,
) (model.Conversation, error) {
	if !validOwner(owner) || !validScope(scope) || strings.TrimSpace(title) == "" || len(title) > 256 ||
		strings.TrimSpace(idempotencyKey) == "" || len(idempotencyKey) > 256 {
		return model.Conversation{}, &Error{Code: "invalid_owned_conversation_request"}
	}
	ownerCopy, scopeCopy := owner, scope
	response, err := c.callWrite(ctx, request{
		Operation: "create_owned_conversation", Owner: &ownerCopy, Scope: &scopeCopy,
		Title: title, IdempotencyKey: idempotencyKey,
	})
	if err != nil {
		return model.Conversation{}, err
	}
	var conversation model.Conversation
	if err := decodeStrict(response, &conversation); err != nil || !validConversation(conversation) ||
		conversation.Title != title || conversation.Scope != scope {
		return model.Conversation{}, &Error{Code: "invalid_runtime_response"}
	}
	return conversation, nil
}

// ImportOwnedConversation atomically imports a bounded transcript as a new
// Global model.Conversation owned by the verified principal.
func (c *Client) ImportOwnedConversation(
	ctx context.Context,
	owner model.Owner,
	title string,
	prompts []model.ConversationImportPrompt,
	idempotencyKey string,
) (model.OwnedConversationImportResult, error) {
	if !validOwner(owner) || strings.TrimSpace(title) == "" || len(title) > 256 ||
		strings.TrimSpace(idempotencyKey) == "" || len(idempotencyKey) > 256 ||
		len(prompts) > maxConversationImportPromptCount {
		return model.OwnedConversationImportResult{}, &Error{Code: "invalid_owned_conversation_import_request"}
	}
	totalContentBytes := 0
	for _, prompt := range prompts {
		if (prompt.Role != "user" && prompt.Role != "assistant") || strings.TrimSpace(prompt.Content) == "" ||
			len(prompt.Content) > maxConversationImportContentBytes-totalContentBytes {
			return model.OwnedConversationImportResult{}, &Error{Code: "invalid_owned_conversation_import_request"}
		}
		totalContentBytes += len(prompt.Content)
	}
	ownerCopy := owner
	promptCopy := append([]model.ConversationImportPrompt{}, prompts...)
	response, err := c.callWrite(ctx, request{
		Operation: "import_owned_conversation", Owner: &ownerCopy, Title: title,
		Prompts: &promptCopy, IdempotencyKey: idempotencyKey,
	})
	if err != nil {
		return model.OwnedConversationImportResult{}, err
	}
	var result model.OwnedConversationImportResult
	if err := decodeStrict(response, &result); err != nil ||
		!validOwnedConversationImport(response, result, title, len(promptCopy)) {
		return model.OwnedConversationImportResult{}, &Error{Code: "invalid_runtime_response"}
	}
	return result, nil
}

// ListOwnedConversations returns only the principal's owner-filtered page.
func (c *Client) ListOwnedConversations(
	ctx context.Context,
	owner model.Owner,
	afterID string,
	limit int,
) (model.OwnedConversationPage, error) {
	if !validOwner(owner) || limit < 1 || limit > maxBootstrapPageLimit ||
		len(afterID) > maxEntityIDBytes || (afterID != "" && strings.TrimSpace(afterID) == "") {
		return model.OwnedConversationPage{}, &Error{Code: "invalid_owned_conversation_request"}
	}
	ownerCopy := owner
	limitCopy := limit
	var after *string
	if afterID != "" {
		after = &afterID
	}
	response, err := c.callWrite(ctx, request{
		Operation: "list_owned_conversations", Owner: &ownerCopy, AfterID: after, Limit: &limitCopy,
	})
	if err != nil {
		return model.OwnedConversationPage{}, err
	}
	var page model.OwnedConversationPage
	if err := decodeStrict(response, &page); err != nil ||
		!validOwnedConversationPage(response, page, afterID, limit) {
		return model.OwnedConversationPage{}, &Error{Code: "invalid_runtime_response"}
	}
	return page, nil
}

// GetOwnedConversation returns one sanitized Conversation row only when the
// verified principal owns the requested ID. Runtime uses one uniform not-found
// result for missing and foreign IDs.
func (c *Client) GetOwnedConversation(
	ctx context.Context,
	owner model.Owner,
	conversationID string,
) (model.OwnedConversationEntry, error) {
	if !validOwner(owner) || strings.TrimSpace(conversationID) == "" || len(conversationID) > maxEntityIDBytes || strings.Contains(conversationID, "/") {
		return model.OwnedConversationEntry{}, &Error{Code: "invalid_owned_conversation_request"}
	}
	ownerCopy := owner
	response, err := c.callWrite(ctx, request{
		Operation: "get_owned_conversation", Owner: &ownerCopy, ConversationID: conversationID,
	})
	if err != nil {
		return model.OwnedConversationEntry{}, err
	}
	var entry model.OwnedConversationEntry
	if err := decodeStrict(response, &entry); err != nil || !validOwnedConversationEntry(response, entry) {
		return model.OwnedConversationEntry{}, &Error{Code: "invalid_runtime_response"}
	}
	return entry, nil
}

// OwnedConversationPrompts returns bounded history after an owner check in Hub.
func (c *Client) OwnedConversationPrompts(
	ctx context.Context,
	owner model.Owner,
	conversationID string,
	before *model.PromptPageCursor,
	limit int,
) (model.ConversationPromptPage, error) {
	if !validOwner(owner) || strings.TrimSpace(conversationID) == "" || len(conversationID) > maxEntityIDBytes ||
		limit < 1 || limit > maxPromptPageLimit || (before != nil && !validPromptCursor(*before)) {
		return model.ConversationPromptPage{}, &Error{Code: "invalid_prompt_request"}
	}
	ownerCopy, limitCopy := owner, limit
	response, err := c.callWrite(ctx, request{
		Operation: "owned_conversation_prompt_page", Owner: &ownerCopy,
		ConversationID: conversationID, Before: before, Limit: &limitCopy,
	})
	if err != nil {
		return model.ConversationPromptPage{}, err
	}
	var page model.ConversationPromptPage
	if err := decodeStrict(response, &page); err != nil ||
		!validPromptPage(response, page, conversationID, before, limit) {
		return model.ConversationPromptPage{}, &Error{Code: "invalid_runtime_response"}
	}
	return page, nil
}

// AppendOwnedPrompt writes a user Prompt with owner and aggregate-version CAS.
func (c *Client) AppendOwnedPrompt(
	ctx context.Context,
	owner model.Owner,
	conversationID string,
	content string,
	idempotencyKey string,
	expectedVersion uint64,
) (model.ConversationPrompt, uint64, bool, error) {
	if !validOwner(owner) || strings.TrimSpace(conversationID) == "" || len(conversationID) > maxEntityIDBytes ||
		strings.TrimSpace(content) == "" || len(content) > maxPromptContentBytes ||
		strings.TrimSpace(idempotencyKey) == "" || len(idempotencyKey) > 256 || expectedVersion > maxSafeJSONInteger {
		return model.ConversationPrompt{}, 0, false, &Error{Code: "invalid_owned_prompt_request"}
	}
	ownerCopy, versionCopy := owner, expectedVersion
	response, err := c.callWrite(ctx, request{
		Operation: "append_owned_prompt", Owner: &ownerCopy, ConversationID: conversationID,
		Content: content, IdempotencyKey: idempotencyKey, ExpectedVersion: &versionCopy,
	})
	if err != nil {
		return model.ConversationPrompt{}, 0, false, err
	}
	var result ownedPromptAppendResult
	if err := decodeStrict(response, &result); err != nil ||
		!validOwnedPromptAppend(response, result, conversationID, content) {
		return model.ConversationPrompt{}, 0, false, &Error{Code: "invalid_runtime_response"}
	}
	return result.Prompt, result.AggregateVersion, result.Replayed, nil
}

func cleanAbsolutePath(value string) (string, error) {
	if value == "" || strings.ContainsRune(value, 0) || !filepath.IsAbs(value) {
		return "", errors.New("invalid path")
	}
	clean := filepath.Clean(value)
	if value != clean || clean == string(filepath.Separator) {
		return "", errors.New("invalid path")
	}
	return clean, nil
}

func existingDirectory(value string) (string, error) {
	clean, err := cleanAbsolutePath(value)
	if err != nil {
		return "", err
	}
	real, err := filepath.EvalSymlinks(clean)
	if err != nil {
		return "", err
	}
	info, err := os.Stat(real)
	if err != nil || !info.IsDir() {
		return "", errors.New("invalid directory")
	}
	return filepath.Clean(real), nil
}

func pathsOverlap(left, right string) bool {
	return within(left, right) || within(right, left)
}

func within(root, path string) bool {
	relative, err := filepath.Rel(root, path)
	if err != nil || relative == "." {
		return err == nil
	}
	return relative != ".." && !strings.HasPrefix(relative, ".."+string(filepath.Separator)) &&
		!filepath.IsAbs(relative)
}
