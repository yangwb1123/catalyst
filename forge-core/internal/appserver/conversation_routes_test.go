package appserver

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"forgeos/forge-core/internal/executionprofile"
	"forgeos/forge-core/internal/runtimebridge"
)

func TestConversationRoutesForwardVerifiedOwnerAndCollectionPage(t *testing.T) {
	var listPage model.OwnedConversationPage
	if err := json.Unmarshal([]byte(`{"conversations":[]}`), &listPage); err != nil {
		t.Fatal(err)
	}
	backend := &fakeConversationBackend{listPage: listPage}
	identity, handler := conversationTestHandler(t, backend)
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationCollectionPath+"?after_id=conversation-previous&limit=12", "forge:conversations:read", "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("GET status=%d body=%q", response.Code, response.Body.String())
	}
	wantOwner := model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	if backend.listCalls != 1 || backend.listOwner != wantOwner || backend.listAfter != "conversation-previous" || backend.listLimit != 12 {
		t.Fatalf("list call = %#v", backend)
	}
	if !strings.Contains(response.Body.String(), `"conversations":[]`) {
		t.Fatalf("unexpected page body %q", response.Body.String())
	}

	for i, scope := range []model.ConversationScope{
		{Kind: "project", ID: "project-9"},
		{Kind: "group", ID: "group-3"},
		{Kind: "global"},
	} {
		body, err := json.Marshal(map[string]any{"scope": scope, "title": "Build tests"})
		if err != nil {
			t.Fatal(err)
		}
		key := fmt.Sprintf("create-key-%d", i+1)
		response = requestConversationAPI(t, handler, identity, http.MethodPost, conversationCollectionPath,
			"forge:conversations:write", "application/json", key, string(body))
		if response.Code != http.StatusCreated {
			t.Fatalf("scope %s POST status=%d body=%q", scope.Kind, response.Code, response.Body.String())
		}
		if backend.createCalls != i+1 || backend.createOwner != wantOwner ||
			backend.createScope != scope || backend.createTitle != "Build tests" || backend.createKey != key {
			t.Fatalf("create call for %s = %#v", scope.Kind, backend)
		}
	}
}

func TestConversationDetailRouteForwardsVerifiedOwnerAndHidesForeignIDs(t *testing.T) {
	backend := &fakeConversationBackend{detail: model.OwnedConversationEntry{
		Conversation: model.Conversation{
			ID: "conversation-17", Scope: model.ConversationScope{Kind: "global"},
			Title: "Shared", CreatedAtMS: 10, UpdatedAtMS: 20,
		},
		AggregateVersion: 3,
	}}
	identity, handler := conversationTestHandler(t, backend)
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationCollectionPath+"/conversation-17", "forge:conversations:read", "", "", "")
	if response.Code != http.StatusOK || backend.detailCalls != 1 {
		t.Fatalf("detail status=%d calls=%d body=%q", response.Code, backend.detailCalls, response.Body.String())
	}
	wantOwner := model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	if backend.detailOwner != wantOwner || backend.detailID != "conversation-17" ||
		!strings.Contains(response.Body.String(), `"aggregate_version":3`) {
		t.Fatalf("detail call=%#v body=%q", backend, response.Body.String())
	}

	backend.err = &runtimebridge.Error{Code: "not_found"}
	response = requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationCollectionPath+"/conversation-foreign", "forge:conversations:read", "", "", "")
	if response.Code != http.StatusNotFound || !strings.Contains(response.Body.String(), `"code":"not_found"`) {
		t.Fatalf("foreign detail status=%d body=%q", response.Code, response.Body.String())
	}
	response = requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationCollectionPath+"/conversation-17?unexpected=yes", "forge:conversations:read", "", "", "")
	if response.Code != http.StatusBadRequest || backend.detailCalls != 2 {
		t.Fatalf("detail query status=%d calls=%d body=%q", response.Code, backend.detailCalls, response.Body.String())
	}
}

func TestConversationPromptRoutesForwardCursorAndVersion(t *testing.T) {
	backend := &fakeConversationBackend{
		promptPage:   model.ConversationPromptPage{ConversationID: "conversation-17", Prompts: []model.ConversationPrompt{}},
		appendPrompt: model.ConversationPrompt{ID: "prompt-1", ConversationID: "conversation-17", Role: "user", Content: "calculate", CreatedAtMS: 21},
		appendAggVer: 8,
	}
	identity, handler := conversationTestHandler(t, backend)
	path := conversationCollectionPath + "/conversation-17/prompts"
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		path+"?limit=8&before_created_at_ms=99&before_prompt_id=prompt-7", "forge:conversations:read", "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("GET prompts status=%d body=%q", response.Code, response.Body.String())
	}
	wantOwner := model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	if backend.promptCalls != 1 || backend.promptOwner != wantOwner || backend.promptID != "conversation-17" ||
		backend.promptCursor == nil || *backend.promptCursor != (model.PromptPageCursor{CreatedAtMS: 99, PromptID: "prompt-7"}) || backend.promptLimit != 8 {
		t.Fatalf("prompt page call = %#v", backend)
	}
	response = requestConversationAPI(t, handler, identity, http.MethodGet,
		path+fmt.Sprintf("?before_created_at_ms=%d&before_prompt_id=prompt-7", maxSafeJSONInteger+1),
		"forge:conversations:read", "", "", "")
	if response.Code != http.StatusBadRequest || backend.promptCalls != 1 {
		t.Fatalf("unsafe Prompt cursor status=%d calls=%d", response.Code, backend.promptCalls)
	}

	body := `{"content":"calculate","expected_version":7}`
	response = requestConversationAPI(t, handler, identity, http.MethodPost, path,
		"forge:conversations:write", "application/json", "prompt-key-1", body)
	if response.Code != http.StatusCreated {
		t.Fatalf("POST prompt status=%d body=%q", response.Code, response.Body.String())
	}
	if backend.appendCalls != 1 || backend.appendOwner != wantOwner || backend.appendID != "conversation-17" ||
		backend.appendContent != "calculate" || backend.appendKey != "prompt-key-1" || backend.appendVersion != 7 {
		t.Fatalf("append call = %#v", backend)
	}

	backend.appendReplay = true
	response = requestConversationAPI(t, handler, identity, http.MethodPost, path,
		"forge:conversations:write", "application/json", "prompt-key-1", body)
	if response.Code != http.StatusOK {
		t.Fatalf("idempotent replay status=%d body=%q", response.Code, response.Body.String())
	}
}

func TestConversationPromptRouteRejectsUnsafeBackendPage(t *testing.T) {
	cases := []struct {
		name string
		page model.ConversationPromptPage
	}{
		{
			name: "foreign conversation",
			page: model.ConversationPromptPage{
				ConversationID: "conversation-other",
				Prompts:        []model.ConversationPrompt{},
			},
		},
		{
			name: "unsafe timestamp",
			page: model.ConversationPromptPage{
				ConversationID: "conversation-17",
				Prompts: []model.ConversationPrompt{{
					ID: "prompt-1", ConversationID: "conversation-17", Role: "user",
					Content: "safe", CreatedAtMS: maxSafeJSONInteger + 1,
				}},
			},
		},
		{
			name: "cursor does not match page tail",
			page: model.ConversationPromptPage{
				ConversationID: "conversation-17",
				Prompts: []model.ConversationPrompt{{
					ID: "prompt-1", ConversationID: "conversation-17", Role: "user",
					Content: "safe", CreatedAtMS: 2,
				}},
				NextCursor: &model.PromptPageCursor{CreatedAtMS: 1, PromptID: "prompt-0"},
				HasMore:    true,
			},
		},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			backend := &fakeConversationBackend{promptPage: test.page}
			identity, handler := conversationTestHandler(t, backend)
			response := requestConversationAPI(t, handler, identity, http.MethodGet,
				conversationCollectionPath+"/conversation-17/prompts?limit=8",
				"forge:conversations:read", "", "", "")
			if response.Code != http.StatusBadGateway || backend.promptCalls != 1 {
				t.Fatalf("unsafe Prompt page status=%d calls=%d body=%q", response.Code, backend.promptCalls, response.Body.String())
			}
			if !strings.Contains(response.Body.String(), `"code":"conversation_service_error"`) {
				t.Fatalf("unsafe Prompt page error=%q", response.Body.String())
			}
		})
	}
}

func TestConversationExecutionProfileUsesHubOwnedProjectIdentity(t *testing.T) {
	profile := intentmodel.ServerExecutionProfile{ID: "profile-1", SHA256: [32]byte{1, 2, 3}}
	catalog, err := executionprofile.New([]executionprofile.Binding{{ProjectID: "project-1", Profile: profile}})
	if err != nil {
		t.Fatal(err)
	}
	backend := &fakeConversationBackend{projectIdentity: model.OwnedProjectConversationIdentity{
		ConversationID: "conversation-1", ProjectID: "project-1",
	}}
	routes := conversationRoutes{backend: backend, profiles: catalog}
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-1", TenantID: "tenant-1"}
	resolved, err := routes.resolveExecutionProfile(context.Background(), owner, "conversation-1")
	if err != nil || resolved != profile || backend.projectIdentityCalls != 1 ||
		backend.projectIdentityOwner != owner || backend.projectIdentityID != "conversation-1" {
		t.Fatalf("profile=%#v backend=%#v error=%v", resolved, backend, err)
	}
	backend.projectIdentity.ProjectID = "project-caller-selected"
	if _, err := routes.resolveExecutionProfile(context.Background(), owner, "conversation-1"); !errors.Is(err, executionprofile.ErrProfileUnavailable) {
		t.Fatalf("unconfigured Hub Project error=%v", err)
	}
	backend.err = &runtimebridge.Error{Code: "not_found"}
	if _, err := routes.resolveExecutionProfile(context.Background(), owner, "conversation-1"); !errors.Is(err, backend.err) {
		t.Fatalf("Hub owner-filter error was not preserved: %v", err)
	}
	withoutCatalog := conversationRoutes{backend: backend}
	beforeCalls := backend.projectIdentityCalls
	if _, err := withoutCatalog.resolveExecutionProfile(context.Background(), owner, "conversation-1"); !errors.Is(err, executionprofile.ErrProfileUnavailable) || backend.projectIdentityCalls != beforeCalls {
		t.Fatalf("missing catalog error=%v Hub calls=%d", err, backend.projectIdentityCalls-beforeCalls)
	}
}

func TestConversationRoutesRejectMissingScopeAndMalformedPaths(t *testing.T) {
	backend := &fakeConversationBackend{}
	identity, handler := conversationTestHandler(t, backend)
	response := requestConversationAPI(t, handler, identity, http.MethodGet, conversationCollectionPath,
		"forge:conversations:write", "", "", "")
	if response.Code != http.StatusForbidden || backend.listCalls != 0 {
		t.Fatalf("wrong-scope response = %d %q calls=%d", response.Code, response.Body.String(), backend.listCalls)
	}
	for _, target := range []string{
		conversationCollectionPath + "/",
		conversationCollectionPath + "/one/two/prompts",
		conversationCollectionPath + "/a%2Fb/prompts",
		conversationCollectionPath + "/conversation-17/prompts/",
		conversationCollectionPath + "/conversation-17/run-intents",
		conversationCollectionPath + "/conversation-17/execution-consents",
	} {
		response := requestConversationAPI(t, handler, identity, http.MethodGet, target,
			"forge:conversations:read", "", "", "")
		if response.Code != http.StatusNotFound {
			t.Errorf("path %q status=%d body=%q, want 404", target, response.Code, response.Body.String())
		}
	}
	response = requestConversationAPI(t, handler, identity, http.MethodDelete, conversationCollectionPath,
		"forge:conversations:read", "", "", "")
	if response.Code != http.StatusMethodNotAllowed || response.Header().Get("Allow") != "GET, POST" {
		t.Fatalf("method response = %d allow=%q", response.Code, response.Header().Get("Allow"))
	}
}

func TestConversationCreateRejectsInvalidJSONAndIdempotency(t *testing.T) {
	backend := &fakeConversationBackend{}
	identity, handler := conversationTestHandler(t, backend)
	valid := `{"scope":{"kind":"global"},"title":"Review"}`
	tests := []struct {
		name        string
		contentType string
		key         string
		body        string
		status      int
	}{
		{"missing content type", "", "key-1", valid, http.StatusUnsupportedMediaType},
		{"wrong content type", "text/plain", "key-1", valid, http.StatusUnsupportedMediaType},
		{"missing idempotency key", "application/json", "", valid, http.StatusBadRequest},
		{"oversized idempotency key", "application/json", strings.Repeat("k", idempotencyKeyMaxBytes+1), valid, http.StatusBadRequest},
		{"duplicate key", "application/json", "key-1", `{"scope":{"kind":"global"},"title":"one","title":"two"}`, http.StatusBadRequest},
		{"unknown field", "application/json", "key-1", `{"scope":{"kind":"global"},"title":"Review","owner":"caller"}`, http.StatusBadRequest},
		{"missing required field", "application/json", "key-1", `{"scope":{"kind":"global"}}`, http.StatusBadRequest},
		{"wrong global scope shape", "application/json", "key-1", `{"scope":{"kind":"global","id":""},"title":"Review"}`, http.StatusBadRequest},
		{"nested duplicate", "application/json", "key-1", `{"scope":{"kind":"global","kind":"project"},"title":"Review"}`, http.StatusBadRequest},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(t, handler, identity, http.MethodPost, conversationCollectionPath,
				"forge:conversations:write", test.contentType, test.key, test.body)
			if response.Code != test.status {
				t.Fatalf("status=%d body=%q, want %d", response.Code, response.Body.String(), test.status)
			}
		})
	}
	large := strings.Repeat("x", conversationBodyMaxBytes+1)
	response := requestConversationAPI(t, handler, identity, http.MethodPost, conversationCollectionPath,
		"forge:conversations:write", "application/json", "key-1", large)
	if response.Code != http.StatusRequestEntityTooLarge {
		t.Fatalf("oversize status=%d body=%q", response.Code, response.Body.String())
	}
	if backend.createCalls != 0 {
		t.Fatalf("backend received %d invalid create requests", backend.createCalls)
	}
	assertDuplicateCreateIdempotencyHeaderRejected(t, handler, identity, backend, valid)
}

func assertDuplicateCreateIdempotencyHeaderRejected(
	t *testing.T,
	handler http.Handler,
	identity *conversationTestIdentity,
	backend *fakeConversationBackend,
	valid string,
) {
	t.Helper()
	request := httptest.NewRequest(http.MethodPost, "http://127.0.0.1:7467"+conversationCollectionPath, strings.NewReader(valid))
	request.Header.Set("Authorization", "Bearer "+identity.token("forge:conversations:write"))
	request.Header.Set("Content-Type", "application/json")
	request.Header.Add("Idempotency-Key", "key-1")
	request.Header.Add("Idempotency-Key", "key-2")
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, request)
	if response.Code != http.StatusBadRequest || backend.createCalls != 0 {
		t.Fatalf("duplicate idempotency header = %d calls=%d body=%q", response.Code, backend.createCalls, response.Body.String())
	}
}

func TestConversationPromptRejectsMissingVersionAndDuplicateContentType(t *testing.T) {
	backend := &fakeConversationBackend{}
	identity, handler := conversationTestHandler(t, backend)
	path := conversationCollectionPath + "/conversation-1/prompts"
	for _, body := range []string{
		`{"content":"do work"}`,
		`{"content":"do work","expected_version":null}`,
		`{"content":"do work","expected_version":-1}`,
		`{"content":"do work","expected_version":0,"extra":true}`,
	} {
		response := requestConversationAPI(t, handler, identity, http.MethodPost, path,
			"forge:conversations:write", "application/json", "prompt-key", body)
		if response.Code != http.StatusBadRequest {
			t.Errorf("body %s status=%d body=%q", body, response.Code, response.Body.String())
		}
	}
	unsafeVersion := fmt.Sprintf(`{"content":"do work","expected_version":%d}`, maxSafeJSONInteger+1)
	response := requestConversationAPI(t, handler, identity, http.MethodPost, path,
		"forge:conversations:write", "application/json", "prompt-key-unsafe", unsafeVersion)
	if response.Code != http.StatusBadRequest || backend.appendCalls != 0 {
		t.Fatalf("unsafe expected version = %d calls=%d body=%q", response.Code, backend.appendCalls, response.Body.String())
	}
	request := httptest.NewRequest(http.MethodPost, "http://127.0.0.1:7467"+path,
		strings.NewReader(`{"content":"do work","expected_version":0}`))
	request.Header.Set("Authorization", "Bearer "+identity.token("forge:conversations:write"))
	request.Header.Add("Content-Type", "application/json")
	request.Header.Add("Content-Type", "application/json")
	request.Header.Set("Idempotency-Key", "prompt-key")
	response = httptest.NewRecorder()
	handler.ServeHTTP(response, request)
	if response.Code != http.StatusUnsupportedMediaType || backend.appendCalls != 0 {
		t.Fatalf("duplicate content type = %d calls=%d body=%q", response.Code, backend.appendCalls, response.Body.String())
	}
}

func TestConversationQueriesAreStrictAndBounded(t *testing.T) {
	backend := &fakeConversationBackend{}
	identity, handler := conversationTestHandler(t, backend)
	targets := []string{
		conversationCollectionPath + "?limit=1&limit=2",
		conversationCollectionPath + "?unknown=1",
		conversationCollectionPath + "?limit=129",
		conversationCollectionPath + "?limit=+1",
		conversationCollectionPath + "?after_id=",
		conversationCollectionPath + "/c-1/prompts?before_created_at_ms=1",
		conversationCollectionPath + "/c-1/prompts?before_created_at_ms=-1&before_prompt_id=p-1",
		conversationCollectionPath + "/c-1/prompts?before_created_at_ms=1&before_prompt_id=p-1&extra=yes",
	}
	for _, target := range targets {
		response := requestConversationAPI(t, handler, identity, http.MethodGet, target,
			"forge:conversations:read", "", "", "")
		if response.Code != http.StatusBadRequest {
			t.Errorf("query %q status=%d body=%q", target, response.Code, response.Body.String())
		}
	}
	response := requestConversationAPI(t, handler, identity, http.MethodGet, conversationCollectionPath,
		"forge:conversations:read", "application/json", "", `{}`)
	if response.Code != http.StatusBadRequest {
		t.Fatalf("GET with body status=%d body=%q", response.Code, response.Body.String())
	}
	if backend.listCalls != 0 || backend.promptCalls != 0 {
		t.Fatalf("invalid pagination reached backend: %#v", backend)
	}
}

func TestConversationBackendErrorsDoNotLeakDetails(t *testing.T) {
	backend := &fakeConversationBackend{err: errors.New("secret token and private body")}
	identity, handler := conversationTestHandler(t, backend)
	response := requestConversationAPI(t, handler, identity, http.MethodGet, conversationCollectionPath,
		"forge:conversations:read", "", "", "")
	if response.Code != http.StatusBadGateway || strings.Contains(response.Body.String(), "secret") || strings.Contains(response.Body.String(), "private body") {
		t.Fatalf("backend error response = %d %q", response.Code, response.Body.String())
	}
	var decoded conversationErrorResponse
	if err := json.Unmarshal(response.Body.Bytes(), &decoded); err != nil || decoded.Code != "conversation_service_error" {
		t.Fatalf("sanitized error response %#v, decode error %v", decoded, err)
	}
}

func TestConversationBackendErrorsKeepConflictAndOwnershipSemantics(t *testing.T) {
	tests := []struct {
		name       string
		code       string
		wantStatus int
		wantCode   string
	}{
		{name: "hidden ownership", code: "not_found", wantStatus: http.StatusNotFound, wantCode: "not_found"},
		{name: "version conflict", code: "conflict", wantStatus: http.StatusConflict, wantCode: "conflict"},
		{name: "unavailable store", code: "storage_unavailable", wantStatus: http.StatusServiceUnavailable, wantCode: "conversation_service_unavailable"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			backend := &fakeConversationBackend{err: &runtimebridge.Error{Code: test.code}}
			identity, handler := conversationTestHandler(t, backend)
			response := requestConversationAPI(t, handler, identity, http.MethodGet,
				conversationCollectionPath, "forge:conversations:read", "", "", "")
			if response.Code != test.wantStatus || !strings.Contains(response.Body.String(), `"code":"`+test.wantCode+`"`) {
				t.Fatalf("status=%d body=%q", response.Code, response.Body.String())
			}
		})
	}
}

func TestConversationPromptCursorRejectsOverflowBeforeBackend(t *testing.T) {
	backend := &fakeConversationBackend{}
	identity, handler := conversationTestHandler(t, backend)
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationCollectionPath+"/c-1/prompts?before_created_at_ms="+fmt.Sprint(uint64(1<<63)),
		"forge:conversations:read", "", "", "")
	if response.Code != http.StatusBadRequest || backend.promptCalls != 0 {
		t.Fatalf("overflow response = %d %q calls=%d", response.Code, response.Body.String(), backend.promptCalls)
	}
}
