package runtimebridge

import (
	"context"
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
