package runtimebridge

import (
	"context"
	"encoding/json"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestOwnedProjectConversationIdentityUsesRuntimeV2AndExactRequest(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "app-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	requestPath := filepath.Join(t.TempDir(), "request.json")
	result := `{"conversation_id":"conversation-7","project_id":"project-42"}`
	script := responseScriptCheckingRequest(database, result, writeProtocolVersion,
		`"api_version":"forgeos.runtime-bridge/v2"`,
		`"operation":"owned_project_conversation_identity"`,
		`"conversation_id":"conversation-7"`,
		`"owner":{"issuer":"https://identity.example","subject":"account-42","tenant_id":"tenant-slate"}`)
	const readRequest = "IFS= read -r request || exit 23\n"
	if !strings.Contains(script, readRequest) {
		t.Fatal("fake Runtime script no longer contains its request-read boundary")
	}
	script = strings.Replace(script, readRequest,
		readRequest+"printf '%s' \"$request\" > "+shellQuote(requestPath)+"\n", 1)
	writeFake(t, executable, script)
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
	identity, err := client.OwnedProjectConversationIdentity(context.Background(), owner, "conversation-7")
	if err != nil || identity.ConversationID != "conversation-7" || identity.ProjectID != "project-42" {
		t.Fatalf("identity=%#v error=%v", identity, err)
	}
	requestBytes, err := os.ReadFile(requestPath)
	if err != nil {
		t.Fatal(err)
	}
	var requestFields map[string]json.RawMessage
	if err := json.Unmarshal(requestBytes, &requestFields); err != nil {
		t.Fatal(err)
	}
	if len(requestFields) != 5 {
		t.Fatalf("identity request included unexpected fields: %s", requestBytes)
	}
	for _, forbidden := range []string{`"project_id"`, `"profile_id"`, `"path"`, `"title"`, `"content"`} {
		if strings.Contains(string(requestBytes), forbidden) {
			t.Fatalf("identity request contained caller-owned or private field %s: %s", forbidden, requestBytes)
		}
	}
}

func TestOwnedProjectConversationIdentityRejectsExpandedOrConfusedResponse(t *testing.T) {
	for _, malformed := range []struct {
		name   string
		result string
	}{
		{name: "path", result: `{"conversation_id":"conversation-7","project_id":"project-42","path":"/private/workspace"}`},
		{name: "title", result: `{"conversation_id":"conversation-7","project_id":"project-42","title":"private title"}`},
		{name: "prompt", result: `{"conversation_id":"conversation-7","project_id":"project-42","prompt":"private prompt"}`},
		{name: "wrong conversation", result: `{"conversation_id":"conversation-8","project_id":"project-42"}`},
		{name: "blank project", result: `{"conversation_id":"conversation-7","project_id":" "}`},
		{name: "missing project", result: `{"conversation_id":"conversation-7"}`},
	} {
		t.Run(malformed.name, func(t *testing.T) {
			appState := filepath.Join(t.TempDir(), "app-state")
			runtimeState := filepath.Join(t.TempDir(), "runtime-state")
			makeDirectory(t, appState)
			makeDirectory(t, runtimeState)
			executable := filepath.Join(t.TempDir(), "runtime-rpc")
			database := filepath.Join(runtimeState, "hub.sqlite3")
			writeFake(t, executable, responseScriptCheckingRequest(database, malformed.result, writeProtocolVersion))
			client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
			if err != nil {
				t.Fatal(err)
			}
			_, err = client.OwnedProjectConversationIdentity(context.Background(),
				model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"},
				"conversation-7")
			if err == nil || err.(*Error).Code != "invalid_runtime_response" {
				t.Fatalf("malformed model.Project identity response accepted: %v", err)
			}
		})
	}
}

func TestOwnedProjectConversationIdentityPreservesNotFoundAndConflict(t *testing.T) {
	for _, code := range []string{"not_found", "conflict"} {
		t.Run(code, func(t *testing.T) {
			appState := filepath.Join(t.TempDir(), "app-state")
			runtimeState := filepath.Join(t.TempDir(), "runtime-state")
			makeDirectory(t, appState)
			makeDirectory(t, runtimeState)
			executable := filepath.Join(t.TempDir(), "runtime-rpc")
			database := filepath.Join(runtimeState, "hub.sqlite3")
			writeFake(t, executable, errorResponseScript(database, code))
			client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
			if err != nil {
				t.Fatal(err)
			}
			_, err = client.OwnedProjectConversationIdentity(context.Background(),
				model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"},
				"conversation-7")
			if err == nil || err.(*Error).Code != code {
				t.Fatalf("Runtime error code %q was not retained: %v", code, err)
			}
		})
	}
}

func TestOwnedProjectConversationIdentityRejectsInvalidInputBeforeProcess(t *testing.T) {
	client := &Client{executable: "/path/that/must/not/be/run"}
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
	for _, test := range []struct {
		name           string
		owner          model.Owner
		conversationID string
	}{
		{name: "owner", owner: model.Owner{}, conversationID: "conversation-7"},
		{name: "conversation", owner: owner, conversationID: " "},
		{name: "conversation too long", owner: owner, conversationID: strings.Repeat("x", maxEntityIDBytes+1)},
	} {
		t.Run(test.name, func(t *testing.T) {
			_, err := client.OwnedProjectConversationIdentity(context.Background(), test.owner, test.conversationID)
			if err == nil || err.(*Error).Code != "invalid_owned_conversation_request" {
				t.Fatalf("invalid input error=%v", err)
			}
		})
	}
}
