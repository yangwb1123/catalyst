package runtimebridge

import (
	"context"
	"encoding/json"
	"fmt"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"path/filepath"
	"strings"
	"testing"
)

func TestOwnedConversationChangesAfterUsesRuntimeV2AndAcceptsDenseOwnerCursors(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "app-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
	result := `{"after_cursor":4,"scanned_through_cursor":6,"has_more":false,"changes":[{"cursor":5,"schema_version":1,"conversation_id":"c-1","entity_id":"p-1","aggregate_version":2,"kind":"prompt_appended","created_at_ms":17},{"cursor":6,"schema_version":1,"conversation_id":"c-2","entity_id":"c-2","aggregate_version":1,"kind":"conversation_created","created_at_ms":19}]}`
	writeFake(t, executable, responseScriptCheckingRequest(database, result, writeProtocolVersion,
		`"api_version":"forgeos.runtime-bridge/v2"`,
		`"operation":"owned_conversation_changes_after"`, `"after_cursor":4`, `"limit":2`,
		`"owner":{"issuer":"https://identity.example","subject":"account-42","tenant_id":"tenant-slate"}`))
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	page, err := client.OwnedConversationChangesAfter(context.Background(), owner, 4, 2)
	if err != nil {
		t.Fatal(err)
	}
	if page.AfterCursor != 4 || page.ScannedThroughCursor != 6 || page.HasMore || len(page.Changes) != 2 ||
		page.Changes[0].Cursor != 5 || page.Changes[1].Cursor != 6 {
		t.Fatalf("owned change page = %#v", page)
	}
	if _, err := client.OwnedConversationChangesAfter(context.Background(), owner, maxSQLiteInteger+1, 2); err == nil {
		t.Fatal("out-of-range cursor was accepted")
	}
	if _, err := client.OwnedConversationChangesAfter(context.Background(), owner, 4, maxChangeLimit+1); err == nil {
		t.Fatal("over-limit request was accepted")
	}
	if _, err := client.OwnedConversationChangesAfter(context.Background(), model.Owner{}, 4, 2); err == nil {
		t.Fatal("empty owner was accepted")
	}
}

func TestOwnedConversationChangePageRejectsMalformedOrExpandedResponses(t *testing.T) {
	valid := `{"after_cursor":4,"scanned_through_cursor":6,"has_more":true,"changes":[{"cursor":5,"schema_version":1,"conversation_id":"c-1","entity_id":"p-1","aggregate_version":2,"kind":"prompt_appended","created_at_ms":17},{"cursor":6,"schema_version":1,"conversation_id":"c-2","entity_id":"p-2","aggregate_version":1,"kind":"prompt_appended","created_at_ms":18}]}`
	cases := []struct {
		name string
		data string
	}{
		{name: "head or count metadata", data: strings.Replace(valid, `"has_more":true`, `"has_more":true,"head_cursor":10`, 1)},
		{name: "foreign owner metadata", data: strings.Replace(valid, `"cursor":6`, `"owner_id":"foreign","cursor":6`, 1)},
		{name: "cursor gap", data: strings.Replace(valid, `"cursor":5`, `"cursor":7`, 1)},
		{name: "cursor beyond scanned position", data: strings.Replace(valid, `"cursor":5`, `"cursor":7`, 1)},
		{name: "scan behind after", data: strings.Replace(valid, `"scanned_through_cursor":6`, `"scanned_through_cursor":3`, 1)},
		{name: "scanned cursor skips owner events", data: strings.Replace(valid, `"scanned_through_cursor":6`, `"scanned_through_cursor":7`, 1)},
		{name: "has more on short page", data: strings.Replace(valid,
			`},{"cursor":6,"schema_version":1,"conversation_id":"c-2","entity_id":"p-2","aggregate_version":1,"kind":"prompt_appended","created_at_ms":18}`, ``, 1)},
		{name: "wrong echoed cursor", data: strings.Replace(valid, `"after_cursor":4`, `"after_cursor":3`, 1)},
		{name: "creation entity mismatch", data: strings.Replace(valid, `"kind":"prompt_appended"`, `"kind":"conversation_created"`, 1)},
		{name: "duplicate cursors", data: strings.Replace(valid, `"cursor":6`, `"cursor":5`, 1)},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			var page model.OwnedConversationChangePage
			data := []byte(test.data)
			if err := decodeStrict(data, &page); err != nil {
				return
			}
			if validOwnedConversationChangePage(data, page, 4, 2) {
				t.Fatalf("malformed response was accepted: %s", test.data)
			}
		})
	}
	var page model.OwnedConversationChangePage
	if err := json.Unmarshal([]byte(valid), &page); err != nil || !validOwnedConversationChangePage([]byte(valid), page, 4, 2) {
		t.Fatalf("valid dense response rejected: %#v, %v", page, err)
	}
	empty := []byte(`{"after_cursor":4,"scanned_through_cursor":4,"has_more":false,"changes":[]}`)
	if err := json.Unmarshal(empty, &page); err != nil || !validOwnedConversationChangePage(empty, page, 4, 2) {
		t.Fatalf("valid empty response rejected: %#v, %v", page, err)
	}
	for _, raw := range []string{
		`{"after_cursor":4,"scanned_through_cursor":5,"has_more":false,"changes":[]}`,
		`{"after_cursor":4,"scanned_through_cursor":4,"has_more":true,"changes":[]}`,
	} {
		if err := json.Unmarshal([]byte(raw), &page); err == nil && validOwnedConversationChangePage([]byte(raw), page, 4, 2) {
			t.Fatalf("invalid empty response was accepted: %s", raw)
		}
	}
}

func responseScriptCheckingRequest(database, result, apiVersion string, fragments ...string) string {
	var script strings.Builder
	script.WriteString(fmt.Sprintf("#!/bin/sh\n[ \"$1\" = '--runtime-rpc' ] || exit 20\n[ \"$2\" = '--database' ] || exit 21\n[ \"$3\" = %s ] || exit 22\nIFS= read -r request || exit 23\n", shellQuote(database)))
	for _, fragment := range fragments {
		script.WriteString(`printf '%s' "$request" | grep -F -- ` + shellQuote(fragment) + " >/dev/null || exit 24\n")
	}
	prefix := `{"api_version":"` + apiVersion + `","request_id":"`
	suffix := `","ok":true,"result":` + result + `}`
	script.WriteString("request_id=${request#*\\\"request_id\\\":\\\"}\n")
	script.WriteString("request_id=${request_id%%\\\"*}\n")
	script.WriteString(fmt.Sprintf("printf '%%s%%s%%s\\n' %s \"$request_id\" %s\n", shellQuote(prefix), shellQuote(suffix)))
	return script.String()
}
