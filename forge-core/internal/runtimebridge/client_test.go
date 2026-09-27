package runtimebridge

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"time"
)

func TestSnapshotUsesFixedArgumentsAndSanitizedResponse(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "server-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	response := `{"snapshot":{"scope":{"kind":"global"},"projects":[{"id":"p1","name":"sample","created_at_ms":1}],"conversations":[],"groups":[],"group_project_members":[]},"cursor":9}`
	writeFake(t, executable, responseScript(database, response))
	client, err := New(Config{
		Executable: executable, AppServerStateDir: appState,
		RuntimeStateDir: runtimeState, Timeout: time.Second,
	})
	if err != nil {
		t.Fatal(err)
	}
	result, err := client.SnapshotAtCursor(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if result.Cursor != 9 || len(result.Snapshot.Projects) != 1 || result.Snapshot.Projects[0].ID != "p1" {
		t.Fatalf("snapshot result = %#v", result)
	}
}

func TestChangesAfterValidatesContiguousBoundedPage(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "server-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	response := `{"after_cursor":0,"next_cursor":1,"head_cursor":2,"has_more":true,"changes":[{"cursor":1,"schema_version":1,"conversation_id":"c1","entity_id":"c1","aggregate_version":1,"kind":"conversation_created","created_at_ms":1}]}`
	writeFake(t, executable, responseScript(database, response))
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	page, err := client.ChangesAfter(context.Background(), 0, 10)
	if err != nil {
		t.Fatal(err)
	}
	if page.NextCursor != 1 || !page.HasMore || page.Changes[0].Kind != "conversation_created" {
		t.Fatalf("change page = %#v", page)
	}
	if _, err := client.ChangesAfter(context.Background(), 0, maxChangeLimit+1); err == nil {
		t.Fatal("over-limit request succeeded")
	}
}

func TestConversationPromptsUsesBoundedSanitizedPage(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "server-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	result := `{"conversation_id":"c1","prompts":[{"id":"p3","conversation_id":"c1","role":"assistant","content":"answer","created_at_ms":3},{"id":"p2","conversation_id":"c1","role":"user","content":"question","created_at_ms":2}],"next_cursor":{"created_at_ms":2,"prompt_id":"p2"},"has_more":true}`
	writeFake(t, executable, responseScript(database, result))
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	page, err := client.ConversationPrompts(context.Background(), "c1", nil, 2)
	if err != nil {
		t.Fatal(err)
	}
	if len(page.Prompts) != 2 || page.Prompts[0].ID != "p3" || page.NextCursor == nil ||
		page.NextCursor.PromptID != "p2" || !page.HasMore {
		t.Fatalf("Prompt page = %#v", page)
	}
	if _, err := client.ConversationPrompts(context.Background(), " ", nil, 1); err == nil {
		t.Fatal("blank model.Conversation ID was accepted")
	}
	if _, err := client.ConversationPrompts(context.Background(), "c1", nil, maxPromptPageLimit+1); err == nil {
		t.Fatal("over-limit Prompt page was accepted")
	}
}

func TestOwnedConversationUsesV2ResponseEnvelope(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "server-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	result := `{"id":"c1","scope":{"kind":"global"},"title":"shared","created_at_ms":1,"updated_at_ms":1}`
	writeFake(t, executable, responseScriptVersion(database, result, writeProtocolVersion))
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	conversation, err := client.CreateOwnedConversation(context.Background(), model.Owner{
		Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate",
	}, model.ConversationScope{Kind: "global"}, "shared", "create-key-1")
	if err != nil || conversation.ID != "c1" {
		t.Fatalf("owned model.Conversation = %#v, error %v", conversation, err)
	}

	writeFake(t, executable, responseScript(database, result))
	if _, err := client.CreateOwnedConversation(context.Background(), model.Owner{
		Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate",
	}, model.ConversationScope{Kind: "global"}, "shared", "create-key-1"); err == nil || err.(*Error).Code != "invalid_runtime_response" {
		t.Fatalf("write accepted a v1 response envelope: %v", err)
	}
}

func TestGetOwnedConversationRejectsForeignResponseID(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "server-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	result := `{"conversation":{"id":"conversation-other","scope":{"kind":"global"},"title":"shared","created_at_ms":1,"updated_at_ms":1},"aggregate_version":1}`
	writeFake(t, executable, responseScript(database, result))
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
	if _, err := client.GetOwnedConversation(context.Background(), owner, "conversation-requested"); err == nil || err.(*Error).Code != "invalid_runtime_response" {
		t.Fatalf("foreign Conversation detail response was accepted: %v", err)
	}
}

func TestPromptPageValidationRejectsLeakageGapsAndOversizedContent(t *testing.T) {
	valid := []byte(`{"conversation_id":"c1","prompts":[{"id":"p2","conversation_id":"c1","role":"user","content":"new","created_at_ms":2},{"id":"p1","conversation_id":"c1","role":"assistant","content":"old","created_at_ms":1}],"next_cursor":{"created_at_ms":1,"prompt_id":"p1"},"has_more":true}`)
	var page model.ConversationPromptPage
	if err := decodeStrict(valid, &page); err != nil || !validPromptPage(valid, page, "c1", nil, 2) {
		t.Fatalf("valid Prompt page rejected: %v / %#v", err, page)
	}
	for _, data := range [][]byte{
		[]byte(`{"conversation_id":"c1","prompts":[{"id":"p1","conversation_id":"c1","role":"user","content":"x","created_at_ms":1,"idempotency_key":"secret"}],"has_more":false}`),
		[]byte(`{"conversation_id":"other","prompts":[{"id":"p1","conversation_id":"other","role":"user","content":"x","created_at_ms":1}],"has_more":false}`),
		[]byte(`{"conversation_id":"c1","prompts":[{"id":"p1","conversation_id":"c1","role":"user","content":"x","created_at_ms":1}],"next_cursor":{"created_at_ms":1,"prompt_id":"wrong"},"has_more":true}`),
		[]byte(`{"conversation_id":"c1","prompts":[{"id":"p1","conversation_id":"c1","role":"user","content":"x","created_at_ms":1}],"next_cursor":{"created_at_ms":1,"prompt_id":"p1","extra":true},"has_more":true}`),
	} {
		var malformed model.ConversationPromptPage
		if err := decodeStrict(data, &malformed); err == nil && validPromptPage(data, malformed, "c1", nil, 2) {
			t.Fatalf("malformed Prompt page accepted: %s", data)
		}
	}
	oversized := []byte(fmt.Sprintf(`{"conversation_id":"c1","prompts":[{"id":"p1","conversation_id":"c1","role":"user","content":%q,"created_at_ms":1}],"has_more":false}`, strings.Repeat("x", maxPromptContentBytes+1)))
	var oversizedPage model.ConversationPromptPage
	if err := decodeStrict(oversized, &oversizedPage); err != nil || validPromptPage(oversized, oversizedPage, "c1", nil, 1) {
		t.Fatal("over-budget Prompt body was accepted")
	}
	aggregate := []byte(fmt.Sprintf(`{"conversation_id":"c1","prompts":[{"id":"p2","conversation_id":"c1","role":"user","content":%q,"created_at_ms":2},{"id":"p1","conversation_id":"c1","role":"assistant","content":%q,"created_at_ms":1}],"has_more":false}`, strings.Repeat("x", maxPromptContentBytes/2+1), strings.Repeat("y", maxPromptContentBytes/2+1)))
	var aggregatePage model.ConversationPromptPage
	if err := decodeStrict(aggregate, &aggregatePage); err != nil || validPromptPage(aggregate, aggregatePage, "c1", nil, 2) {
		t.Fatal("aggregate Prompt content over the page budget was accepted")
	}
	ascending := []byte(`{"conversation_id":"c1","prompts":[{"id":"p1","conversation_id":"c1","role":"user","content":"old","created_at_ms":1},{"id":"p2","conversation_id":"c1","role":"user","content":"new","created_at_ms":2}],"has_more":false}`)
	var ascendingPage model.ConversationPromptPage
	if err := decodeStrict(ascending, &ascendingPage); err != nil || validPromptPage(ascending, ascendingPage, "c1", nil, 2) {
		t.Fatal("unstable Prompt order was accepted")
	}
	before := model.PromptPageCursor{CreatedAtMS: 2, PromptID: "p2"}
	older := []byte(`{"conversation_id":"c1","prompts":[{"id":"p1","conversation_id":"c1","role":"user","content":"old","created_at_ms":1}],"has_more":false}`)
	var olderPage model.ConversationPromptPage
	if err := decodeStrict(older, &olderPage); err != nil || !validPromptPage(older, olderPage, "c1", &before, 1) {
		t.Fatalf("valid exclusive Prompt cursor rejected: %v", err)
	}
	newer := []byte(`{"conversation_id":"c1","prompts":[{"id":"p3","conversation_id":"c1","role":"user","content":"newer","created_at_ms":3}],"has_more":false}`)
	var newerPage model.ConversationPromptPage
	if err := decodeStrict(newer, &newerPage); err != nil || validPromptPage(newer, newerPage, "c1", &before, 1) {
		t.Fatal("Prompt at or above the exclusive cursor was accepted")
	}
}

func TestPromptJSONNumbersUseSafeIntegerBoundary(t *testing.T) {
	valid := []byte(fmt.Sprintf(`{"conversation_id":"c1","prompts":[{"id":"p1","conversation_id":"c1","role":"user","content":"x","created_at_ms":%d}],"next_cursor":{"created_at_ms":%d,"prompt_id":"p1"},"has_more":true}`, maxSafeJSONInteger, maxSafeJSONInteger))
	var page model.ConversationPromptPage
	if err := decodeStrict(valid, &page); err != nil || !validPromptPage(valid, page, "c1", nil, 1) {
		t.Fatalf("JSON-safe Prompt boundary rejected: %v", err)
	}
	unsafe := bytes.ReplaceAll(valid, []byte(fmt.Sprint(maxSafeJSONInteger)), []byte(fmt.Sprint(maxSafeJSONInteger+1)))
	var unsafePage model.ConversationPromptPage
	if err := decodeStrict(unsafe, &unsafePage); err != nil || validPromptPage(unsafe, unsafePage, "c1", nil, 1) {
		t.Fatal("Prompt timestamp above JSON-safe integer accepted")
	}
	unsafeCursor := model.PromptPageCursor{CreatedAtMS: maxSafeJSONInteger + 1, PromptID: "p1"}
	if _, err := (&Client{}).ConversationPrompts(context.Background(), "c1", &unsafeCursor, 1); err == nil {
		t.Fatal("Prompt request cursor above JSON-safe integer accepted")
	}
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-1", TenantID: "tenant-1"}
	if _, err := (&Client{}).OwnedConversationPrompts(context.Background(), owner, "c1", &unsafeCursor, 1); err == nil {
		t.Fatal("owned Prompt request cursor above JSON-safe integer accepted")
	}
	result := ownedPromptAppendResult{Prompt: model.ConversationPrompt{
		ID: "p1", ConversationID: "c1", Role: "user", Content: "x", CreatedAtMS: maxSafeJSONInteger + 1,
	}, AggregateVersion: 1}
	if validOwnedPromptAppend([]byte(`{"prompt":{"id":"p1","conversation_id":"c1","role":"user","content":"x","created_at_ms":9007199254740992},"aggregate_version":1,"replayed":false}`), result, "c1", "x") {
		t.Fatal("Prompt append timestamp above JSON-safe integer accepted")
	}
	if _, _, _, err := (&Client{}).AppendOwnedPrompt(context.Background(), owner, "c1", "x", "key", maxSafeJSONInteger+1); err == nil {
		t.Fatal("Prompt append expected version above JSON-safe integer accepted")
	}
}

func TestOwnedAggregateVersionsUseSafeIntegerBoundary(t *testing.T) {
	conversation := model.Conversation{
		ID: "conversation-1", Scope: model.ConversationScope{Kind: "global"}, Title: "Shared",
		CreatedAtMS: 1, UpdatedAtMS: 1,
	}
	pageData := []byte(fmt.Sprintf(`{"conversations":[{"conversation":{"id":"conversation-1","scope":{"kind":"global"},"title":"Shared","created_at_ms":1,"updated_at_ms":1},"aggregate_version":%d}],"has_more":false}`, maxSafeJSONInteger))
	page := model.OwnedConversationPage{Conversations: []model.OwnedConversationEntry{{
		Conversation: conversation, AggregateVersion: maxSafeJSONInteger,
	}}}
	if !validOwnedConversationPage(pageData, page, "", 1) {
		t.Fatal("JSON-safe Conversation aggregate boundary rejected")
	}
	unsafePage := bytes.ReplaceAll(pageData, []byte(fmt.Sprint(maxSafeJSONInteger)), []byte(fmt.Sprint(maxSafeJSONInteger+1)))
	page.Conversations[0].AggregateVersion = maxSafeJSONInteger + 1
	if validOwnedConversationPage(unsafePage, page, "", 1) {
		t.Fatal("Conversation aggregate above JSON-safe integer accepted")
	}

	detailData := []byte(fmt.Sprintf(`{"conversation":{"id":"conversation-1","scope":{"kind":"global"},"title":"Shared","created_at_ms":1,"updated_at_ms":1},"aggregate_version":%d}`, maxSafeJSONInteger))
	entry := model.OwnedConversationEntry{Conversation: conversation, AggregateVersion: maxSafeJSONInteger}
	if !validOwnedConversationEntry(detailData, entry) {
		t.Fatal("JSON-safe Conversation detail aggregate boundary rejected")
	}
	entry.AggregateVersion = maxSafeJSONInteger + 1
	if validOwnedConversationEntry(unsafePage, entry) {
		t.Fatal("Conversation detail aggregate above JSON-safe integer accepted")
	}

	appendData := []byte(fmt.Sprintf(`{"prompt":{"id":"prompt-1","conversation_id":"conversation-1","role":"user","content":"ship it","created_at_ms":1},"aggregate_version":%d,"replayed":false}`, maxSafeJSONInteger))
	appendResult := ownedPromptAppendResult{Prompt: model.ConversationPrompt{
		ID: "prompt-1", ConversationID: "conversation-1", Role: "user", Content: "ship it", CreatedAtMS: 1,
	}, AggregateVersion: maxSafeJSONInteger}
	if !validOwnedPromptAppend(appendData, appendResult, "conversation-1", "ship it") {
		t.Fatal("JSON-safe Prompt append aggregate boundary rejected")
	}
	unsafeAppend := bytes.ReplaceAll(appendData, []byte(fmt.Sprint(maxSafeJSONInteger)), []byte(fmt.Sprint(maxSafeJSONInteger+1)))
	appendResult.AggregateVersion = maxSafeJSONInteger + 1
	if validOwnedPromptAppend(unsafeAppend, appendResult, "conversation-1", "ship it") {
		t.Fatal("Prompt append aggregate above JSON-safe integer accepted")
	}

	importData := []byte(fmt.Sprintf(`{"conversation":{"id":"conversation-1","scope":{"kind":"global"},"title":"Shared","created_at_ms":1,"updated_at_ms":1},"aggregate_version":%d,"imported_prompt_count":0,"replayed":false}`, maxSafeJSONInteger))
	importResult := model.OwnedConversationImportResult{Conversation: conversation, AggregateVersion: maxSafeJSONInteger}
	if !validOwnedConversationImport(importData, importResult, "Shared", 0) {
		t.Fatal("JSON-safe import aggregate boundary rejected")
	}
	unsafeImport := bytes.ReplaceAll(importData, []byte(fmt.Sprint(maxSafeJSONInteger)), []byte(fmt.Sprint(maxSafeJSONInteger+1)))
	importResult.AggregateVersion = maxSafeJSONInteger + 1
	if validOwnedConversationImport(unsafeImport, importResult, "Shared", 0) {
		t.Fatal("import aggregate above JSON-safe integer accepted")
	}
}

func TestOwnedPromptAppendBindsTheExpectedAggregateVersion(t *testing.T) {
	valid := []byte(`{"prompt":{"id":"prompt-1","conversation_id":"conversation-1","role":"user","content":"ship it","created_at_ms":1},"aggregate_version":8,"replayed":false}`)
	result := ownedPromptAppendResult{Prompt: model.ConversationPrompt{
		ID: "prompt-1", ConversationID: "conversation-1", Role: "user", Content: "ship it", CreatedAtMS: 1,
	}, AggregateVersion: 8}
	if !validOwnedPromptAppendForVersion(valid, result, "conversation-1", "ship it", 7) {
		t.Fatal("exactly-next Prompt aggregate version was rejected")
	}

	for _, aggregateVersion := range []uint64{7, 9} {
		mutated := result
		mutated.AggregateVersion = aggregateVersion
		data, err := json.Marshal(mutated)
		if err != nil {
			t.Fatal(err)
		}
		if validOwnedPromptAppendForVersion(data, mutated, "conversation-1", "ship it", 7) {
			t.Fatalf("non-next Prompt aggregate version %d was accepted", aggregateVersion)
		}
	}
	if validOwnedPromptAppendForVersion(valid, result, "conversation-1", "ship it", maxSafeJSONInteger) {
		t.Fatal("JSON-unsafe next Prompt aggregate version was accepted")
	}
}

func TestAppendOwnedPromptRejectsAJumpedAggregateVersionAtTheBridge(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "server-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	result := `{"prompt":{"id":"prompt-1","conversation_id":"conversation-1","role":"user","content":"ship it","created_at_ms":1},"aggregate_version":9,"replayed":false}`
	writeFake(t, executable, responseScript(database, result))
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
	if _, _, _, err := client.AppendOwnedPrompt(
		context.Background(), owner, "conversation-1", "ship it", "prompt-key", 7,
	); err == nil || err.(*Error).Code != "invalid_runtime_response" {
		t.Fatalf("jumped Prompt aggregate version was accepted: %v", err)
	}
}

func TestDecodeStrictRejectsInvalidUTF8(t *testing.T) {
	data := append([]byte(`{"content":"`), 0xff)
	data = append(data, []byte(`"}`)...)
	var decoded struct {
		Content string `json:"content"`
	}
	if err := decodeStrict(data, &decoded); err == nil {
		t.Fatal("invalid UTF-8 JSON content was accepted")
	}
}

func TestNewRejectsSharedOrInvalidStateDirectories(t *testing.T) {
	root := t.TempDir()
	appState := filepath.Join(root, "state")
	runtimeState := filepath.Join(root, "runtime")
	nestedRuntime := filepath.Join(appState, "runtime")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	makeDirectory(t, nestedRuntime)
	base := Config{
		Executable:        filepath.Join(root, "runtime-rpc"),
		AppServerStateDir: appState,
		RuntimeStateDir:   nestedRuntime,
	}
	if _, err := New(base); err == nil {
		t.Fatal("nested Runtime state directory was accepted")
	}
	base.RuntimeStateDir = "relative"
	if _, err := New(base); err == nil {
		t.Fatal("relative Runtime state directory was accepted")
	}
	base.RuntimeStateDir = runtimeState
	base.Timeout = maxTimeout + time.Millisecond
	if _, err := New(base); err == nil {
		t.Fatal("excessive Runtime timeout was accepted")
	}
	alias := filepath.Join(root, "runtime-alias")
	if err := os.Symlink(nestedRuntime, alias); err != nil {
		t.Fatal(err)
	}
	base.RuntimeStateDir = alias
	base.Timeout = 0
	if _, err := New(base); err == nil {
		t.Fatal("Runtime state symlink alias was accepted")
	}
}

func TestChildFailuresAreGenericAndCancellationIsBounded(t *testing.T) {
	root := t.TempDir()
	appState := filepath.Join(root, "server")
	runtimeState := filepath.Join(root, "runtime")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(root, "runtime-rpc")
	writeFake(t, executable, "#!/bin/sh\nprintf secret-path >&2\nexit 23\n")
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	_, err = client.SnapshotAtCursor(context.Background())
	if err == nil || strings.Contains(err.Error(), "secret-path") || strings.Contains(err.Error(), runtimeState) {
		t.Fatalf("process failure was not generic: %v", err)
	}

	pidFile := filepath.Join(root, "child.pid")
	writeFake(t, executable, fmt.Sprintf("#!/bin/sh\n/bin/sleep 5 &\nprintf '%%s' \"$!\" > %s\nwait\n", shellQuote(pidFile)))
	client, err = New(Config{
		Executable: executable, AppServerStateDir: appState,
		RuntimeStateDir: runtimeState, Timeout: 40 * time.Millisecond,
	})
	if err != nil {
		t.Fatal(err)
	}
	started := time.Now()
	_, err = client.SnapshotAtCursor(context.Background())
	elapsed := time.Since(started)
	var bridgeErr *Error
	if !errors.As(err, &bridgeErr) || bridgeErr.Code != "runtime_timeout" {
		t.Fatalf("timeout error = %v", err)
	}
	if elapsed > time.Second {
		t.Fatalf("child holding stdout/stderr prolonged timeout to %s", elapsed)
	}
	pidBytes, err := os.ReadFile(pidFile)
	if err != nil {
		t.Fatal(err)
	}
	pid, err := strconv.Atoi(string(pidBytes))
	if err != nil {
		t.Fatal(err)
	}
	child, err := os.FindProcess(pid)
	if err != nil {
		t.Fatal(err)
	}
	_ = child.Kill()
}

func TestResponseDecoderRejectsPathAndUnknownFields(t *testing.T) {
	if err := decodeStrict([]byte(`{"id":"p","name":"x","created_at_ms":1,"path":"/private/work"}`), &model.Project{}); err == nil {
		t.Fatal("model.Project path crossed the sanitized bridge type")
	}
	var duplicated struct {
		Cursor uint64 `json:"cursor"`
	}
	if err := decodeStrict([]byte(`{"cursor":0,"cursor":1}`), &duplicated); err == nil {
		t.Fatal("duplicate JSON fields were accepted")
	}
	if err := decodeStrict([]byte(`{"after_cursor":0,"next_cursor":0,"head_cursor":1,"has_more":true,"changes":[]}`), &model.ChangePage{}); err != nil {
		t.Fatal(err)
	}
	if validPage(model.ChangePage{AfterCursor: 0, NextCursor: 0, HeadCursor: 1, HasMore: true}, 0, 1) {
		t.Fatal("empty page before the observed head was accepted")
	}
	if validChangeFields([]byte(`null`), model.ChangePage{}) ||
		validChangeFields([]byte(`{"after_cursor":0,"next_cursor":0,"head_cursor":0,"has_more":false}`), model.ChangePage{Changes: []model.Change{}}) {
		t.Fatal("missing or null page fields were accepted")
	}
	var snapshot model.SnapshotAtCursor
	missingLists := []byte(`{"snapshot":{"scope":{"kind":"global"}},"cursor":0}`)
	if decodeStrict(missingLists, &snapshot) != nil || validSnapshot(missingLists, snapshot) {
		t.Fatal("snapshot with omitted fields was accepted")
	}
	for _, projection := range []string{
		`{"scope":{"kind":"global"},"projects":[{}],"conversations":[],"groups":[],"group_project_members":[]}`,
		`{"scope":{"kind":"global"},"projects":[],"conversations":[{}],"groups":[],"group_project_members":[]}`,
		`{"scope":{"kind":"global"},"projects":[],"conversations":[],"groups":[{}],"group_project_members":[]}`,
		`{"scope":{"kind":"global"},"projects":[],"conversations":[],"groups":[],"group_project_members":[{}]}`,
	} {
		data := []byte(`{"snapshot":` + projection + `,"cursor":0}`)
		if decodeStrict(data, &snapshot) != nil || validSnapshot(data, snapshot) {
			t.Fatalf("snapshot with incomplete array item was accepted: %s", projection)
		}
	}
	malformedError := []byte(`{"api_version":"` + protocolVersion + `","request_id":"q1","ok":false,"error":{"message":"failed"}}`)
	if _, err := decodeEnvelope(malformedError, "q1"); err == nil {
		t.Fatal("error envelope with missing code was accepted")
	}
	for _, framed := range [][]byte{[]byte(`{}`), []byte("{}\n\n"), []byte("{}\r\n")} {
		if _, err := unframeResponse(framed); err == nil {
			t.Fatalf("invalid response frame %q was accepted", framed)
		}
	}
}

func TestChangePageUsesJSONSafeIntegerBoundary(t *testing.T) {
	valid := model.ChangePage{
		AfterCursor: maxSafeJSONInteger - 1, NextCursor: maxSafeJSONInteger,
		HeadCursor: maxSafeJSONInteger, Changes: []model.Change{{
			Cursor: maxSafeJSONInteger, SchemaVersion: 1, ConversationID: "c-1",
			EntityID: "p-1", AggregateVersion: maxSafeJSONInteger,
			Kind: "prompt_appended", CreatedAtMS: maxSafeJSONInteger,
		}},
	}
	if !validPage(valid, maxSafeJSONInteger-1, 1) {
		t.Fatal("JSON-safe global change boundary rejected")
	}
	for name, mutate := range map[string]func(*model.ChangePage){
		"head":      func(page *model.ChangePage) { page.HeadCursor = maxSafeJSONInteger + 1 },
		"next":      func(page *model.ChangePage) { page.NextCursor = maxSafeJSONInteger + 1 },
		"cursor":    func(page *model.ChangePage) { page.Changes[0].Cursor = maxSafeJSONInteger + 1 },
		"aggregate": func(page *model.ChangePage) { page.Changes[0].AggregateVersion = maxSafeJSONInteger + 1 },
		"created":   func(page *model.ChangePage) { page.Changes[0].CreatedAtMS = maxSafeJSONInteger + 1 },
	} {
		page := valid
		mutate(&page)
		if validPage(page, maxSafeJSONInteger-1, 1) {
			t.Fatalf("unsafe global change %s accepted", name)
		}
	}
}

func TestSnapshotValidatesTaggedConversationScopes(t *testing.T) {
	cases := []struct {
		name  string
		scope string
		valid bool
	}{
		{name: "global", scope: `{"kind":"global"}`, valid: true},
		{name: "project", scope: `{"kind":"project","id":"p1"}`, valid: true},
		{name: "group", scope: `{"kind":"group","id":"g1"}`, valid: true},
		{name: "missing kind", scope: `{}`},
		{name: "unknown kind", scope: `{"kind":"workspace"}`},
		{name: "project missing id", scope: `{"kind":"project"}`},
		{name: "global has id", scope: `{"kind":"global","id":"p1"}`},
		{name: "group has empty id", scope: `{"kind":"group","id":""}`},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			data := []byte(fmt.Sprintf(`{"snapshot":{"scope":{"kind":"global"},"projects":[],"conversations":[{"id":"c1","scope":%s,"title":"t","created_at_ms":1,"updated_at_ms":1}],"groups":[],"group_project_members":[]},"cursor":0}`, test.scope))
			var snapshot model.SnapshotAtCursor
			if err := decodeStrict(data, &snapshot); err != nil {
				t.Fatalf("strict decoding rejected %s scope: %v", test.name, err)
			}
			if valid := validSnapshot(data, snapshot); valid != test.valid {
				t.Fatalf("scope validity = %v, want %v", valid, test.valid)
			}
		})
	}
}

func responseScript(database, result string) string {
	return responseScriptVersion(database, result, protocolVersion)
}

func responseScriptVersion(database, result, apiVersion string) string {
	prefix := `{"api_version":"` + apiVersion + `","request_id":"`
	suffix := `","ok":true,"result":` + result + `}`
	return fmt.Sprintf("#!/bin/sh\n[ \"$1\" = '--runtime-rpc' ] || exit 20\n[ \"$2\" = '--database' ] || exit 21\n[ \"$3\" = %s ] || exit 22\nIFS= read -r request || exit 23\nrequest_id=${request#*\\\"request_id\\\":\\\"}\nrequest_id=${request_id%%%%\\\"*}\nprintf '%%s%%s%%s\\n' %s \"$request_id\" %s\n", shellQuote(database), shellQuote(prefix), shellQuote(suffix))
}

func shellQuote(value string) string {
	return "'" + strings.ReplaceAll(value, "'", "'\\''") + "'"
}

func writeFake(t *testing.T, path, body string) {
	t.Helper()
	if err := os.WriteFile(path, []byte(body), 0o700); err != nil {
		t.Fatal(err)
	}
}

func makeDirectory(t *testing.T, path string) {
	t.Helper()
	if err := os.MkdirAll(path, 0o700); err != nil {
		t.Fatal(err)
	}
}
