package runtimebridge

import (
	"context"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"path/filepath"
	"strings"
	"testing"
)

func TestImportOwnedConversationUsesRuntimeV2AndExactOwner(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "app-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
	prompts := []model.ConversationImportPrompt{
		{Role: "user", Content: "question"},
		{Role: "assistant", Content: "answer"},
	}
	result := `{"conversation":{"id":"c-import","scope":{"kind":"global"},"title":"Imported transcript","created_at_ms":10,"updated_at_ms":10},"aggregate_version":3,"imported_prompt_count":2,"replayed":false}`
	writeFake(t, executable, responseScriptCheckingRequest(database, result, writeProtocolVersion,
		`"api_version":"forgeos.runtime-bridge/v2"`,
		`"operation":"import_owned_conversation"`,
		`"owner":{"issuer":"https://identity.example","subject":"account-42","tenant_id":"tenant-slate"}`,
		`"title":"Imported transcript"`, `"idempotency_key":"import-key-1"`,
		`"prompts":[{"role":"user","content":"question"},{"role":"assistant","content":"answer"}]`))
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	imported, err := client.ImportOwnedConversation(context.Background(), owner, "Imported transcript", prompts, "import-key-1")
	if err != nil || imported.Conversation.ID != "c-import" || imported.Conversation.Scope.Kind != "global" ||
		imported.AggregateVersion != 3 || imported.ImportedPromptCount != 2 || imported.Replayed {
		t.Fatalf("imported result=%#v error=%v", imported, err)
	}
}

func TestImportOwnedConversationSendsEmptyPromptArray(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "app-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	result := `{"conversation":{"id":"c-empty-import","scope":{"kind":"global"},"title":"Empty import","created_at_ms":10,"updated_at_ms":10},"aggregate_version":1,"imported_prompt_count":0,"replayed":false}`
	writeFake(t, executable, responseScriptCheckingRequest(database, result, writeProtocolVersion,
		`"operation":"import_owned_conversation"`, `"prompts":[]`))
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	imported, err := client.ImportOwnedConversation(context.Background(),
		model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"},
		"Empty import", nil, "empty-import-key")
	if err != nil || imported.ImportedPromptCount != 0 || imported.Conversation.ID != "c-empty-import" {
		t.Fatalf("empty import result=%#v error=%v", imported, err)
	}
}

func TestImportOwnedConversationAcceptsEscapedPayloadAboveOneMiB(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "app-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	result := `{"conversation":{"id":"c-import","scope":{"kind":"global"},"title":"Import","created_at_ms":10,"updated_at_ms":10},"aggregate_version":1,"imported_prompt_count":1,"replayed":false}`
	writeFake(t, executable, responseScriptCheckingRequest(database, result, writeProtocolVersion,
		`"operation":"import_owned_conversation"`))
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	content := strings.Repeat("\x01", maxConversationImportContentBytes)
	imported, err := client.ImportOwnedConversation(context.Background(),
		model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"},
		"Import", []model.ConversationImportPrompt{{Role: "user", Content: content}}, "large-import-key")
	if err != nil || imported.ImportedPromptCount != 1 {
		t.Fatalf("large escaped import result=%#v error=%v", imported, err)
	}
}

func TestOwnedConversationImportValidationRejectsOutOfContractInput(t *testing.T) {
	client := &Client{}
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
	for _, test := range []struct {
		name    string
		owner   model.Owner
		title   string
		prompts []model.ConversationImportPrompt
		key     string
	}{
		{name: "missing owner", title: "Import", key: "key"},
		{name: "blank title", owner: owner, title: "  ", key: "key"},
		{name: "blank key", owner: owner, title: "Import"},
		{name: "unsupported role", owner: owner, title: "Import", key: "key", prompts: []model.ConversationImportPrompt{{Role: "system", Content: "x"}}},
		{name: "blank prompt", owner: owner, title: "Import", key: "key", prompts: []model.ConversationImportPrompt{{Role: "user", Content: "  "}}},
		{name: "too many prompts", owner: owner, title: "Import", key: "key", prompts: make([]model.ConversationImportPrompt, maxConversationImportPromptCount+1)},
		{name: "aggregate content over limit", owner: owner, title: "Import", key: "key", prompts: []model.ConversationImportPrompt{
			{Role: "user", Content: strings.Repeat("x", maxConversationImportContentBytes)},
			{Role: "assistant", Content: "y"},
		}},
	} {
		t.Run(test.name, func(t *testing.T) {
			_, err := client.ImportOwnedConversation(context.Background(), test.owner, test.title, test.prompts, test.key)
			if err == nil || err.(*Error).Code != "invalid_owned_conversation_import_request" {
				t.Fatalf("invalid import error=%v", err)
			}
		})
	}
}

func TestOwnedConversationImportResponseMustBeExactAndConsistent(t *testing.T) {
	valid := `{"conversation":{"id":"c-import","scope":{"kind":"global"},"title":"Import","created_at_ms":1,"updated_at_ms":1},"aggregate_version":3,"imported_prompt_count":1,"replayed":false}`
	cases := []struct {
		name string
		data string
	}{
		{name: "owner leakage", data: strings.Replace(valid, `"aggregate_version":3`, `"aggregate_version":3,"owner":{"subject":"account-42"}`, 1)},
		{name: "non-global scope", data: strings.Replace(valid, `"kind":"global"`, `"kind":"project","id":"p1"`, 1)},
		{name: "prompt count mismatch", data: strings.Replace(valid, `"imported_prompt_count":1`, `"imported_prompt_count":2`, 1)},
		{name: "extra conversation field", data: strings.Replace(valid, `"title":"Import"`, `"title":"Import","project_path":"/secret"`, 1)},
		{name: "wrong title", data: strings.Replace(valid, `"title":"Import"`, `"title":"Other"`, 1)},
		{name: "zero aggregate version", data: strings.Replace(valid, `"aggregate_version":3`, `"aggregate_version":0`, 1)},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			data := []byte(test.data)
			var result model.OwnedConversationImportResult
			if err := decodeStrict(data, &result); err == nil &&
				validOwnedConversationImport(data, result, "Import", 1) {
				t.Fatalf("malformed import response accepted: %s", test.data)
			}
		})
	}
	var result model.OwnedConversationImportResult
	data := []byte(valid)
	if err := decodeStrict(data, &result); err != nil || !validOwnedConversationImport(data, result, "Import", 1) {
		t.Fatalf("valid import response rejected: %#v, %v", result, err)
	}
	if validOwnedConversationImport(data, result, "Import", 0) {
		t.Fatalf("import response accepted wrong expected prompt count: %#v", result)
	}
}
