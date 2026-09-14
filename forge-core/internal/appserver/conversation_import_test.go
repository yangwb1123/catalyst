package appserver

import (
	"encoding/json"
	"fmt"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/runtimebridge"
)

const conversationImportPath = conversationCollectionPath + "/import"

func TestConversationImportRouteForwardsVerifiedOwnerAndGlobalTranscript(t *testing.T) {
	prompts := []model.ConversationImportPrompt{
		{Role: "user", Content: "first prompt"},
		{Role: "assistant", Content: "first answer"},
	}
	backend := &fakeConversationBackend{importResult: model.OwnedConversationImportResult{
		Conversation: model.Conversation{
			ID: "conversation-imported", Scope: model.ConversationScope{Kind: "global"},
			Title: "Imported transcript", CreatedAtMS: 10, UpdatedAtMS: 10,
		},
		AggregateVersion: 3, ImportedPromptCount: len(prompts),
	}}
	identity, handler := conversationTestHandler(t, backend)
	body, err := json.Marshal(importConversationRequest{Title: "Imported transcript", Prompts: prompts})
	if err != nil {
		t.Fatal(err)
	}
	response := requestConversationAPI(t, handler, identity, http.MethodPost, conversationImportPath,
		"forge:conversations:write", "application/json", "import-key-1", string(body))
	if response.Code != http.StatusCreated {
		t.Fatalf("import status=%d body=%q", response.Code, response.Body.String())
	}
	wantOwner := model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	if backend.importCalls != 1 || backend.importOwner != wantOwner || backend.importTitle != "Imported transcript" ||
		backend.importKey != "import-key-1" || len(backend.importPrompts) != 2 || backend.importPrompts[1] != prompts[1] {
		t.Fatalf("import call = %#v", backend)
	}
	var result model.OwnedConversationImportResult
	if err := json.Unmarshal(response.Body.Bytes(), &result); err != nil || result.Conversation.ID != "conversation-imported" ||
		result.Conversation.Scope.Kind != "global" || result.AggregateVersion != 3 ||
		result.ImportedPromptCount != len(prompts) || result.Replayed {
		t.Fatalf("import result=%#v decode=%v body=%q", result, err, response.Body.String())
	}

	backend.importResult.Replayed = true
	response = requestConversationAPI(t, handler, identity, http.MethodPost, conversationImportPath,
		"forge:conversations:write", "application/json", "import-key-1", string(body))
	if response.Code != http.StatusOK || backend.importCalls != 2 {
		t.Fatalf("replayed import status=%d calls=%d body=%q", response.Code, backend.importCalls, response.Body.String())
	}
}

func TestConversationImportRequiresWriteScopeAndIdempotencyKey(t *testing.T) {
	backend := &fakeConversationBackend{}
	identity, handler := conversationTestHandler(t, backend)
	body := `{"title":"Imported transcript","prompts":[]}`
	response := requestConversationAPI(t, handler, identity, http.MethodPost, conversationImportPath,
		"forge:conversations:read", "application/json", "import-key-1", body)
	if response.Code != http.StatusForbidden || backend.importCalls != 0 {
		t.Fatalf("wrong scope status=%d calls=%d body=%q", response.Code, backend.importCalls, response.Body.String())
	}
	response = requestConversationAPI(t, handler, identity, http.MethodPost, conversationImportPath,
		"forge:conversations:write", "application/json", "", body)
	if response.Code != http.StatusBadRequest || backend.importCalls != 0 {
		t.Fatalf("missing Idempotency-Key status=%d calls=%d body=%q", response.Code, backend.importCalls, response.Body.String())
	}
	response = requestConversationAPI(t, handler, identity, http.MethodGet, conversationImportPath,
		"forge:conversations:write", "", "", "")
	if response.Code != http.StatusMethodNotAllowed || response.Header().Get("Allow") != http.MethodPost {
		t.Fatalf("wrong method status=%d allow=%q", response.Code, response.Header().Get("Allow"))
	}
}

func TestConversationImportMapsRuntimeValidationAndConflictErrors(t *testing.T) {
	identity, handler := conversationTestHandler(t, &fakeConversationBackend{
		err: &runtimebridge.Error{Code: "invalid_owned_conversation_import"},
	})
	body := `{"title":"Imported transcript","prompts":[]}`
	response := requestConversationAPI(t, handler, identity, http.MethodPost, conversationImportPath,
		"forge:conversations:write", "application/json", "import-key-1", body)
	if response.Code != http.StatusBadRequest {
		t.Fatalf("runtime validation status=%d body=%q", response.Code, response.Body.String())
	}
	backend := &fakeConversationBackend{err: &runtimebridge.Error{Code: "conflict"}}
	identity, handler = conversationTestHandler(t, backend)
	response = requestConversationAPI(t, handler, identity, http.MethodPost, conversationImportPath,
		"forge:conversations:write", "application/json", "import-key-1", body)
	if response.Code != http.StatusConflict {
		t.Fatalf("idempotency conflict status=%d body=%q", response.Code, response.Body.String())
	}
}

func TestConversationImportRejectsMalformedAndOverBudgetRequests(t *testing.T) {
	backend := &fakeConversationBackend{}
	identity, handler := conversationTestHandler(t, backend)
	tooMuchContent := strings.Repeat("y", conversationImportContentMaxBytes+1)
	tooManyPrompts := make([]model.ConversationImportPrompt, conversationImportPromptMax+1)
	for i := range tooManyPrompts {
		tooManyPrompts[i] = model.ConversationImportPrompt{Role: "user", Content: "x"}
	}
	cases := []struct {
		name string
		body string
	}{
		{name: "unknown top-level field", body: `{"title":"Imported transcript","prompts":[],"scope":{"kind":"project","id":"p1"}}`},
		{name: "missing title", body: `{"prompts":[]}`},
		{name: "null prompts", body: `{"title":"Imported transcript","prompts":null}`},
		{name: "unknown prompt field", body: `{"title":"Imported transcript","prompts":[{"role":"user","content":"x","id":"source-id"}]}`},
		{name: "unsupported role", body: `{"title":"Imported transcript","prompts":[{"role":"system","content":"x"}]}`},
		{name: "blank content", body: `{"title":"Imported transcript","prompts":[{"role":"user","content":"  "}]}`},
		{name: "oversized aggregate content", body: fmt.Sprintf(`{"title":"Imported transcript","prompts":[{"role":"user","content":%q}]}`, tooMuchContent)},
		{name: "too many prompts", body: mustMarshalImport(t, importConversationRequest{Title: "Imported transcript", Prompts: tooManyPrompts})},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(t, handler, identity, http.MethodPost, conversationImportPath,
				"forge:conversations:write", "application/json", "import-invalid-1", test.body)
			if response.Code != http.StatusBadRequest {
				t.Fatalf("status=%d body=%q", response.Code, response.Body.String())
			}
		})
	}
	if backend.importCalls != 0 {
		t.Fatalf("invalid imports reached backend %d times", backend.importCalls)
	}
}

func TestConversationImportBodyLimitAllowsWorstCaseEscapedJSON(t *testing.T) {
	backend := &fakeConversationBackend{importResult: model.OwnedConversationImportResult{
		Conversation: model.Conversation{
			ID: "conversation-imported", Scope: model.ConversationScope{Kind: "global"},
			Title: "Imported transcript", CreatedAtMS: 10, UpdatedAtMS: 10,
		},
		AggregateVersion: 1, ImportedPromptCount: 1,
	}}
	identity, handler := conversationTestHandler(t, backend)
	content := strings.Repeat("\x01", conversationImportContentMaxBytes)
	body, err := json.Marshal(importConversationRequest{
		Title:   "Imported transcript",
		Prompts: []model.ConversationImportPrompt{{Role: "user", Content: content}},
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(body) <= 1024*1024 || len(body) > conversationBodyMaxBytes {
		t.Fatalf("escaped request size=%d, expected between 1 MiB and body limit %d", len(body), conversationBodyMaxBytes)
	}
	response := requestConversationAPI(t, handler, identity, http.MethodPost, conversationImportPath,
		"forge:conversations:write", "application/json", "import-key-large", string(body))
	if response.Code != http.StatusCreated || backend.importCalls != 1 || len(backend.importPrompts) != 1 ||
		len(backend.importPrompts[0].Content) != conversationImportContentMaxBytes {
		t.Fatalf("escaped import status=%d calls=%d content bytes=%d body=%q",
			response.Code, backend.importCalls, len(backend.importPrompts[0].Content), response.Body.String())
	}
}

func TestConversationImportHTTPToRustRPCWhenConfigured(t *testing.T) {
	executable := os.Getenv("FORGE_RUNTIME_BIN")
	if executable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for Go HTTP to Rust Hub import integration")
	}
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	if err := os.Mkdir(runtimeState, 0o700); err != nil {
		t.Fatal(err)
	}
	initializeRuntimeHubForIntegration(t, executable, runtimeState)
	appState := filepath.Join(t.TempDir(), "app-server-state")
	if err := os.Mkdir(appState, 0o700); err != nil {
		t.Fatal(err)
	}
	client, err := runtimebridge.New(runtimebridge.Config{
		Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState,
	})
	if err != nil {
		t.Fatal(err)
	}
	identity, auth := newConversationTestIdentity(t)
	handler := auth.Handler(newConversationRoutes(client))
	prompts := []model.ConversationImportPrompt{
		{Role: "user", Content: "original question"},
		{Role: "assistant", Content: "original answer"},
	}
	body, err := json.Marshal(importConversationRequest{Title: "Imported transcript", Prompts: prompts})
	if err != nil {
		t.Fatal(err)
	}
	request := func(body string) *httptest.ResponseRecorder {
		t.Helper()
		return requestConversationAPI(t, handler, identity, http.MethodPost, conversationImportPath,
			"forge:conversations:write", "application/json", "http-import-key", body)
	}
	created := request(string(body))
	if created.Code != http.StatusCreated {
		t.Fatalf("import status=%d body=%q", created.Code, created.Body.String())
	}
	var first model.OwnedConversationImportResult
	if err := json.Unmarshal(created.Body.Bytes(), &first); err != nil ||
		first.Conversation.Scope.Kind != "global" || first.AggregateVersion == 0 ||
		first.ImportedPromptCount != len(prompts) || first.Replayed {
		t.Fatalf("first import=%#v decode=%v body=%q", first, err, created.Body.String())
	}
	assertImportedConversationReplayAndHistory(t, request, handler, identity, first, string(body))
}

func assertImportedConversationReplayAndHistory(
	t *testing.T,
	request func(string) *httptest.ResponseRecorder,
	handler http.Handler,
	identity *conversationTestIdentity,
	first model.OwnedConversationImportResult,
	body string,
) {
	t.Helper()
	replayed := request(string(body))
	var retry model.OwnedConversationImportResult
	if err := json.Unmarshal(replayed.Body.Bytes(), &retry); err != nil || replayed.Code != http.StatusOK ||
		!retry.Replayed || retry.Conversation.ID != first.Conversation.ID || retry.AggregateVersion != first.AggregateVersion {
		t.Fatalf("import retry status=%d result=%#v decode=%v body=%q", replayed.Code, retry, err, replayed.Body.String())
	}
	changedBody, err := json.Marshal(importConversationRequest{Title: "Imported transcript", Prompts: []model.ConversationImportPrompt{
		{Role: "user", Content: "changed question"},
	}})
	if err != nil {
		t.Fatal(err)
	}
	conflict := request(string(changedBody))
	if conflict.Code != http.StatusConflict {
		t.Fatalf("idempotency conflict status=%d body=%q", conflict.Code, conflict.Body.String())
	}
	history := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationCollectionPath+"/"+first.Conversation.ID+"/prompts",
		"forge:conversations:read", "", "", "")
	if history.Code != http.StatusOK || !strings.Contains(history.Body.String(), "original question") ||
		!strings.Contains(history.Body.String(), "original answer") {
		t.Fatalf("imported history status=%d body=%q", history.Code, history.Body.String())
	}
}

func TestConversationImportBodyLimitErrorRemainsBounded(t *testing.T) {
	backend := &fakeConversationBackend{}
	identity, handler := conversationTestHandler(t, backend)
	oversized := strings.Repeat("x", conversationBodyMaxBytes+1)
	response := requestConversationAPI(t, handler, identity, http.MethodPost, conversationImportPath,
		"forge:conversations:write", "application/json", "import-key-too-large", oversized)
	if response.Code != http.StatusRequestEntityTooLarge || backend.importCalls != 0 {
		t.Fatalf("oversized import status=%d calls=%d body=%q", response.Code, backend.importCalls, response.Body.String())
	}
}

func mustMarshalImport(t *testing.T, request importConversationRequest) string {
	t.Helper()
	data, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}
