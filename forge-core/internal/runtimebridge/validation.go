package runtimebridge

import (
	"bytes"
	"encoding/json"
	"errors"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"io"
	"strings"
	"unicode/utf8"
)

func decodeStrict(data []byte, destination any) error {
	if !utf8.Valid(data) {
		return errors.New("invalid UTF-8 JSON")
	}
	if err := rejectDuplicateKeys(data); err != nil {
		return err
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(destination); err != nil {
		return err
	}
	if err := decoder.Decode(new(any)); !errors.Is(err, io.EOF) {
		return errors.New("trailing JSON")
	}
	return nil
}

func rejectDuplicateKeys(data []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	first, err := decoder.Token()
	if err != nil {
		return err
	}
	if err := walkJSONValue(decoder, first, 1); err != nil {
		return err
	}
	if _, err := decoder.Token(); !errors.Is(err, io.EOF) {
		return errors.New("trailing JSON token")
	}
	return nil
}

func walkJSONValue(decoder *json.Decoder, token json.Token, depth int) error {
	if depth > maxJSONDepth {
		return errors.New("JSON nesting limit exceeded")
	}
	delimiter, ok := token.(json.Delim)
	if !ok {
		return nil
	}
	switch delimiter {
	case '{':
		seen := make(map[string]struct{})
		for decoder.More() {
			keyToken, err := decoder.Token()
			if err != nil {
				return err
			}
			key, ok := keyToken.(string)
			if !ok {
				return errors.New("invalid JSON object key")
			}
			if _, exists := seen[key]; exists {
				return errors.New("duplicate JSON object key")
			}
			seen[key] = struct{}{}
			value, err := decoder.Token()
			if err != nil {
				return err
			}
			if err := walkJSONValue(decoder, value, depth+1); err != nil {
				return err
			}
		}
		return consumeClosingDelimiter(decoder, '}')
	case '[':
		for decoder.More() {
			value, err := decoder.Token()
			if err != nil {
				return err
			}
			if err := walkJSONValue(decoder, value, depth+1); err != nil {
				return err
			}
		}
		return consumeClosingDelimiter(decoder, ']')
	default:
		return errors.New("invalid JSON delimiter")
	}
}

func consumeClosingDelimiter(decoder *json.Decoder, expected json.Delim) error {
	closing, err := decoder.Token()
	if err != nil || closing != expected {
		return errors.New("invalid JSON delimiter")
	}
	return nil
}

func requireObjectFields(data []byte, fields ...string) error {
	var object map[string]json.RawMessage
	if err := json.Unmarshal(data, &object); err != nil || object == nil {
		return errors.New("expected JSON object")
	}
	for _, field := range fields {
		value, found := object[field]
		if !found || len(value) == 0 || bytes.Equal(value, []byte("null")) {
			return errors.New("required JSON field missing")
		}
	}
	return nil
}

func requireObjectFieldSet(data []byte, fields ...string) error {
	if err := requireObjectFields(data, fields...); err != nil {
		return err
	}
	var object map[string]json.RawMessage
	if err := json.Unmarshal(data, &object); err != nil || len(object) != len(fields) {
		return errors.New("unexpected JSON object fields")
	}
	return nil
}

func validSnapshot(data []byte, snapshot model.SnapshotAtCursor) bool {
	if requireObjectFields(data, "snapshot", "cursor") != nil ||
		snapshot.Snapshot.Scope.Kind != "global" || snapshot.Snapshot.Scope.ID != "" ||
		snapshot.Snapshot.Projects == nil || snapshot.Snapshot.Conversations == nil ||
		snapshot.Snapshot.Groups == nil || snapshot.Snapshot.GroupProjectMembers == nil {
		return false
	}
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil || requireObjectFieldSet(data, "snapshot", "cursor") != nil ||
		requireObjectFields(root["snapshot"],
			"scope", "projects", "conversations", "groups", "group_project_members") != nil {
		return false
	}
	var projection map[string]json.RawMessage
	if json.Unmarshal(root["snapshot"], &projection) != nil ||
		!validConversationScope(projection["scope"]) {
		return false
	}
	return requireArrayObjectFields(projection["projects"], "id", "name", "created_at_ms") == nil &&
		requireArrayObjectFields(projection["conversations"], "id", "scope", "title", "created_at_ms", "updated_at_ms") == nil &&
		validConversationScopes(projection["conversations"]) &&
		requireArrayObjectFields(projection["groups"], "id", "name", "created_at_ms") == nil &&
		requireArrayObjectFields(projection["group_project_members"], "group_id", "project_id", "role", "added_at_ms") == nil
}

func validConversationScopes(data []byte) bool {
	var conversations []map[string]json.RawMessage
	if json.Unmarshal(data, &conversations) != nil || conversations == nil {
		return false
	}
	for _, conversation := range conversations {
		if !validConversationScope(conversation["scope"]) {
			return false
		}
	}
	return true
}

func validConversationScope(data []byte) bool {
	var scope model.ConversationScope
	if decodeStrict(data, &scope) != nil {
		return false
	}
	switch scope.Kind {
	case "global":
		return scope.ID == "" && requireObjectFieldSet(data, "kind") == nil
	case "project", "group":
		return scope.ID != "" && requireObjectFieldSet(data, "kind", "id") == nil
	default:
		return false
	}
}

func validOwner(owner model.Owner) bool {
	return strings.TrimSpace(owner.Issuer) != "" && len(owner.Issuer) <= 2048 &&
		strings.TrimSpace(owner.Subject) != "" && len(owner.Subject) <= 255 &&
		strings.TrimSpace(owner.TenantID) != "" && len(owner.TenantID) <= 256 &&
		!strings.ContainsAny(owner.Issuer+owner.Subject+owner.TenantID, "\x00\r\n")
}

func validScope(scope model.ConversationScope) bool {
	data, err := json.Marshal(scope)
	return err == nil && validConversationScope(data)
}

func validConversation(conversation model.Conversation) bool {
	return validEntityID(conversation.ID) && strings.TrimSpace(conversation.Title) != "" &&
		len(conversation.Title) <= 256 && conversation.CreatedAtMS <= conversation.UpdatedAtMS &&
		conversation.UpdatedAtMS <= maxSQLiteInteger && validScope(conversation.Scope)
}

func validPromptCursor(cursor model.PromptPageCursor) bool {
	return cursor.CreatedAtMS <= maxSQLiteInteger && validEntityID(cursor.PromptID)
}

func validOwnedConversationPage(
	data []byte,
	page model.OwnedConversationPage,
	afterID string,
	limit int,
) bool {
	fields := []string{"conversations", "has_more"}
	if page.NextAfterID != nil {
		fields = append(fields, "next_after_id")
	}
	if requireObjectFieldSet(data, fields...) != nil || page.Conversations == nil ||
		len(page.Conversations) > limit || page.HasMore != (page.NextAfterID != nil) {
		return false
	}
	previous := afterID
	for _, entry := range page.Conversations {
		if !validConversation(entry.Conversation) || entry.AggregateVersion == 0 ||
			(entry.Conversation.ID <= previous && previous != "") {
			return false
		}
		previous = entry.Conversation.ID
	}
	if page.HasMore {
		return len(page.Conversations) == limit && page.NextAfterID != nil &&
			*page.NextAfterID == page.Conversations[len(page.Conversations)-1].Conversation.ID
	}
	return page.NextAfterID == nil
}

func validOwnedPromptAppend(
	data []byte,
	result ownedPromptAppendResult,
	conversationID string,
	content string,
) bool {
	if requireObjectFieldSet(data, "prompt", "aggregate_version", "replayed") != nil ||
		result.AggregateVersion == 0 || result.Prompt.ConversationID != conversationID ||
		result.Prompt.Role != "user" || result.Prompt.Content != content ||
		!validEntityID(result.Prompt.ID) || result.Prompt.CreatedAtMS > maxSQLiteInteger {
		return false
	}
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil {
		return false
	}
	return requireObjectFieldSet(root["prompt"], "id", "conversation_id", "role", "content", "created_at_ms") == nil
}

func validOwnedConversationImport(
	data []byte,
	result model.OwnedConversationImportResult,
	title string,
	promptCount int,
) bool {
	if requireObjectFieldSet(data, "conversation", "aggregate_version", "imported_prompt_count", "replayed") != nil ||
		!validConversation(result.Conversation) || result.Conversation.Scope != (model.ConversationScope{Kind: "global"}) ||
		result.Conversation.Title != title || result.AggregateVersion == 0 ||
		result.ImportedPromptCount != promptCount || promptCount < 0 ||
		promptCount > maxConversationImportPromptCount {
		return false
	}
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil {
		return false
	}
	return requireObjectFieldSet(root["conversation"],
		"id", "scope", "title", "created_at_ms", "updated_at_ms") == nil
}

func requireArrayObjectFields(data []byte, fields ...string) error {
	var entries []json.RawMessage
	if err := json.Unmarshal(data, &entries); err != nil || entries == nil {
		return errors.New("expected JSON array")
	}
	for _, entry := range entries {
		if err := requireObjectFields(entry, fields...); err != nil {
			return err
		}
	}
	return nil
}

func validChangeFields(data []byte, page model.ChangePage) bool {
	if requireObjectFields(data, "after_cursor", "next_cursor", "head_cursor", "has_more", "changes") != nil ||
		page.Changes == nil {
		return false
	}
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil {
		return false
	}
	var changes []json.RawMessage
	if json.Unmarshal(root["changes"], &changes) != nil || changes == nil {
		return false
	}
	for _, change := range changes {
		if requireObjectFields(change, "cursor", "schema_version", "conversation_id", "entity_id",
			"aggregate_version", "kind", "created_at_ms") != nil {
			return false
		}
	}
	return true
}

func validPromptPage(
	data []byte,
	page model.ConversationPromptPage,
	conversationID string,
	before *model.PromptPageCursor,
	limit int,
) bool {
	fields := []string{"conversation_id", "prompts", "has_more"}
	if page.NextCursor != nil {
		fields = append(fields, "next_cursor")
	}
	if requireObjectFieldSet(data, fields...) != nil || page.ConversationID != conversationID ||
		page.Prompts == nil || len(page.Prompts) > limit {
		return false
	}
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil {
		return false
	}
	var prompts []json.RawMessage
	if json.Unmarshal(root["prompts"], &prompts) != nil || prompts == nil || len(prompts) != len(page.Prompts) {
		return false
	}
	if !validPromptEntries(prompts, page, conversationID, before) {
		return false
	}
	if page.HasMore {
		return validNextPromptCursor(root, page)
	}
	return page.NextCursor == nil
}

func validPromptEntries(
	rawPrompts []json.RawMessage,
	page model.ConversationPromptPage,
	conversationID string,
	before *model.PromptPageCursor,
) bool {
	seen := make(map[string]struct{}, len(page.Prompts))
	contentBytes := 0
	for index, prompt := range page.Prompts {
		raw := rawPrompts[index]
		if requireObjectFieldSet(raw, "id", "conversation_id", "role", "content", "created_at_ms") != nil ||
			prompt.ID == "" || len(prompt.ID) > maxEntityIDBytes ||
			prompt.ConversationID != conversationID || prompt.Role == "" || len(prompt.Role) > maxRoleBytes ||
			len(prompt.Content) > maxPromptContentBytes || prompt.CreatedAtMS > maxSQLiteInteger {
			return false
		}
		if _, exists := seen[prompt.ID]; exists {
			return false
		}
		seen[prompt.ID] = struct{}{}
		contentBytes += len(prompt.Content)
		if contentBytes > maxPromptContentBytes {
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
	return true
}

func validNextPromptCursor(root map[string]json.RawMessage, page model.ConversationPromptPage) bool {
	if len(page.Prompts) == 0 || page.NextCursor == nil ||
		!samePromptCursor(*page.NextCursor, page.Prompts[len(page.Prompts)-1]) {
		return false
	}
	var fields map[string]json.RawMessage
	if json.Unmarshal(root["next_cursor"], &fields) != nil ||
		requireObjectFieldSet(root["next_cursor"], "created_at_ms", "prompt_id") != nil {
		return false
	}
	return page.NextCursor.PromptID != "" && len(page.NextCursor.PromptID) <= maxEntityIDBytes &&
		page.NextCursor.CreatedAtMS <= maxSQLiteInteger
}

func samePromptCursor(cursor model.PromptPageCursor, prompt model.ConversationPrompt) bool {
	return cursor.CreatedAtMS == prompt.CreatedAtMS && cursor.PromptID == prompt.ID
}

func unframeResponse(framed []byte) ([]byte, error) {
	if len(framed) == 0 || len(framed) > maxResponseBytes || framed[len(framed)-1] != '\n' ||
		bytes.Contains(framed[:len(framed)-1], []byte{'\n'}) || bytes.IndexByte(framed, '\r') >= 0 {
		return nil, errors.New("invalid response frame")
	}
	return framed[:len(framed)-1], nil
}

func validPage(page model.ChangePage, after uint64, limit int) bool {
	if page.AfterCursor != after || page.HeadCursor < after || page.NextCursor > page.HeadCursor ||
		len(page.Changes) > limit {
		return false
	}
	expected := after
	for _, change := range page.Changes {
		if expected == ^uint64(0) || change.Cursor != expected+1 ||
			change.SchemaVersion != 1 || change.AggregateVersion == 0 ||
			change.ConversationID == "" || change.EntityID == "" ||
			(change.Kind != "conversation_created" && change.Kind != "prompt_appended") {
			return false
		}
		expected = change.Cursor
	}
	if len(page.Changes) == 0 && page.AfterCursor != page.HeadCursor {
		return false
	}
	return page.NextCursor == expected && page.HasMore == (page.NextCursor < page.HeadCursor)
}

type limitedBuffer struct {
	bytes.Buffer
	limit    int
	exceeded bool
}

func (buffer *limitedBuffer) Write(value []byte) (int, error) {
	if buffer.Len()+len(value) > buffer.limit {
		buffer.exceeded = true
		return len(value), nil
	}
	return buffer.Buffer.Write(value)
}
