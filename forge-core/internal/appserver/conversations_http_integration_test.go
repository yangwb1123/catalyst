package appserver

import (
	"context"
	"encoding/json"
	"fmt"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/auditprojection"
	"forgeos/forge-core/internal/runtimebridge"
)

func TestConversationHTTPToRustRPCWhenConfigured(t *testing.T) {
	executable := os.Getenv("FORGE_RUNTIME_BIN")
	if executable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for Go HTTP to Rust Hub integration")
	}
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	if err := os.Mkdir(runtimeState, 0o700); err != nil {
		t.Fatal(err)
	}
	initializeRuntimeHubForIntegration(t, executable, runtimeState)
	projectPath, projectID, groupID := seedRuntimeConversationScopes(t, executable, runtimeState)
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
	created := createIntegrationConversations(t, handler, identity, projectPath, projectID, groupID)
	conversation := created[0]
	promptID := appendIntegrationPrompts(t, handler, identity, conversation.ID)
	assertIntegrationPromptHistory(t, handler, identity, conversation.ID, promptID)
	assertIntegrationOwnerList(t, handler, identity, created, projectPath)
	assertIntegrationChangeFeed(t, handler, identity, conversation.ID)
	assertIntegrationNoNormalRun(t, handler, identity, conversation.ID)
	assertIntegrationForeignAccess(t, handler, identity, projectID, conversation.ID)
}

func createIntegrationConversations(
	t *testing.T,
	handler http.Handler,
	identity *conversationTestIdentity,
	projectPath, projectID, groupID string,
) []model.Conversation {
	t.Helper()
	scopes := []model.ConversationScope{
		{Kind: "global"},
		{Kind: "project", ID: projectID},
		{Kind: "group", ID: groupID},
	}
	created := make([]model.Conversation, 0, len(scopes))
	for index, scope := range scopes {
		body, err := json.Marshal(map[string]any{
			"scope": scope, "title": fmt.Sprintf("cross-process shared session %d", index+1),
		})
		if err != nil {
			t.Fatal(err)
		}
		response := requestConversationAPI(t, handler, identity, http.MethodPost, conversationCollectionPath,
			"forge:conversations:write", "application/json", fmt.Sprintf("create-cross-process-%d", index+1), string(body))
		if response.Code != http.StatusCreated {
			t.Fatalf("%s scope create status=%d body=%q", scope.Kind, response.Code, response.Body.String())
		}
		var conversation model.Conversation
		if err := json.Unmarshal(response.Body.Bytes(), &conversation); err != nil ||
			conversation.ID == "" || conversation.Scope != scope {
			t.Fatalf("created %s Conversation=%#v decode=%v", scope.Kind, conversation, err)
		}
		if strings.Contains(response.Body.String(), projectPath) || strings.Contains(response.Body.String(), "run_id") {
			t.Fatalf("%s scope response leaked a path or Run field: %q", scope.Kind, response.Body.String())
		}
		created = append(created, conversation)
	}
	return created
}

func appendIntegrationPrompts(t *testing.T, handler http.Handler, identity *conversationTestIdentity, conversationID string) string {
	t.Helper()
	promptPath := conversationCollectionPath + "/" + conversationID + "/prompts"
	promptBody := `{"content":"run the bounded task","expected_version":1}`
	response := requestConversationAPI(t, handler, identity, http.MethodPost, promptPath,
		"forge:conversations:write", "application/json", "cross-process-prompt-1", promptBody)
	var firstAppend struct {
		Prompt struct {
			ID string `json:"id"`
		} `json:"prompt"`
		AggregateVersion uint64 `json:"aggregate_version"`
		Replayed         bool   `json:"replayed"`
	}
	if err := json.Unmarshal(response.Body.Bytes(), &firstAppend); err != nil ||
		response.Code != http.StatusCreated || firstAppend.Prompt.ID == "" ||
		firstAppend.AggregateVersion != 2 || firstAppend.Replayed {
		t.Fatalf("append status=%d body=%q", response.Code, response.Body.String())
	}
	secondAppend := requestConversationAPI(t, handler, identity, http.MethodPost, promptPath,
		"forge:conversations:write", "application/json", "cross-process-prompt-2",
		`{"content":"follow-up storage only","expected_version":2}`)
	if secondAppend.Code != http.StatusCreated {
		t.Fatalf("second append status=%d body=%q", secondAppend.Code, secondAppend.Body.String())
	}
	assertIntegrationPromptReplayAndConflicts(t, handler, identity, promptPath, promptBody, firstAppend.Prompt.ID)
	return firstAppend.Prompt.ID
}

func assertIntegrationPromptReplayAndConflicts(
	t *testing.T,
	handler http.Handler,
	identity *conversationTestIdentity,
	promptPath, promptBody, firstPromptID string,
) {
	t.Helper()
	replay := requestConversationAPI(t, handler, identity, http.MethodPost, promptPath,
		"forge:conversations:write", "application/json", "cross-process-prompt-1", promptBody)
	var replayAppend struct {
		Prompt struct {
			ID string `json:"id"`
		} `json:"prompt"`
		AggregateVersion uint64 `json:"aggregate_version"`
		Replayed         bool   `json:"replayed"`
	}
	if err := json.Unmarshal(replay.Body.Bytes(), &replayAppend); err != nil ||
		replay.Code != http.StatusOK || !replayAppend.Replayed ||
		replayAppend.Prompt.ID != firstPromptID || replayAppend.AggregateVersion != 3 {
		t.Fatalf("idempotent replay status=%d result=%#v body=%q decode=%v",
			replay.Code, replayAppend, replay.Body.String(), err)
	}
	changedBody := requestConversationAPI(t, handler, identity, http.MethodPost, promptPath,
		"forge:conversations:write", "application/json", "cross-process-prompt-1",
		`{"content":"changed body","expected_version":3}`)
	if changedBody.Code != http.StatusConflict {
		t.Fatalf("same key with changed body status=%d body=%q", changedBody.Code, changedBody.Body.String())
	}
	staleVersion := requestConversationAPI(t, handler, identity, http.MethodPost, promptPath,
		"forge:conversations:write", "application/json", "cross-process-prompt-stale",
		`{"content":"stale write","expected_version":1}`)
	if staleVersion.Code != http.StatusConflict {
		t.Fatalf("stale expected_version status=%d body=%q", staleVersion.Code, staleVersion.Body.String())
	}
}

func assertIntegrationPromptHistory(t *testing.T, handler http.Handler, identity *conversationTestIdentity, conversationID, promptID string) {
	t.Helper()
	promptPath := conversationCollectionPath + "/" + conversationID + "/prompts"
	response := requestConversationAPI(t, handler, identity, http.MethodGet, promptPath,
		"forge:conversations:read", "", "", "")
	var history model.ConversationPromptPage
	if err := json.Unmarshal(response.Body.Bytes(), &history); err != nil || response.Code != http.StatusOK ||
		len(history.Prompts) != 2 || history.Prompts[0].Content != "follow-up storage only" ||
		history.Prompts[1].ID != promptID || history.Prompts[1].Content != "run the bounded task" {
		t.Fatalf("history status=%d body=%q", response.Code, response.Body.String())
	}
}

func assertIntegrationOwnerList(
	t *testing.T,
	handler http.Handler,
	identity *conversationTestIdentity,
	created []model.Conversation,
	projectPath string,
) {
	t.Helper()
	listResponse := requestConversationAPI(t, handler, identity, http.MethodGet, conversationCollectionPath,
		"forge:conversations:read", "", "", "")
	var ownerPage model.OwnedConversationPage
	if err := json.Unmarshal(listResponse.Body.Bytes(), &ownerPage); err != nil ||
		listResponse.Code != http.StatusOK || len(ownerPage.Conversations) != len(created) {
		t.Fatalf("owner list status=%d page=%#v body=%q decode=%v",
			listResponse.Code, ownerPage, listResponse.Body.String(), err)
	}
	for _, conversation := range created {
		found := false
		for _, entry := range ownerPage.Conversations {
			if entry.Conversation.ID == conversation.ID && entry.Conversation.Scope == conversation.Scope {
				found = true
			}
		}
		if !found {
			t.Errorf("owner list did not return %s scope Conversation %s", conversation.Scope.Kind, conversation.ID)
		}
	}
	if strings.Contains(listResponse.Body.String(), projectPath) {
		t.Fatalf("owner list leaked local Project path: %q", listResponse.Body.String())
	}
}

func assertIntegrationChangeFeed(t *testing.T, handler http.Handler, identity *conversationTestIdentity, conversationID string) {
	t.Helper()
	changesTarget := conversationChangesPath + "?after_cursor=0&limit=50"
	response := requestConversationAPI(t, handler, identity, http.MethodGet, changesTarget,
		"forge:conversations:read", "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("owner change feed status=%d body=%q", response.Code, response.Body.String())
	}
	var changes model.OwnedConversationChangePage
	if err := json.Unmarshal(response.Body.Bytes(), &changes); err != nil ||
		len(changes.Changes) != 5 || changes.AfterCursor != 0 ||
		changes.ScannedThroughCursor != changes.Changes[len(changes.Changes)-1].Cursor ||
		changes.Changes[0].Cursor != 1 || changes.Changes[4].Cursor != 5 ||
		changes.HasMore || strings.Contains(response.Body.String(), "run the bounded task") {
		t.Fatalf("owner change page=%#v body=%q decode=%v", changes, response.Body.String(), err)
	}
	if changes.Changes[0].Kind != "conversation_created" ||
		changes.Changes[1].Kind != "conversation_created" ||
		changes.Changes[2].Kind != "conversation_created" ||
		changes.Changes[3].Kind != "prompt_appended" || changes.Changes[4].Kind != "prompt_appended" {
		t.Fatalf("unexpected owner change kinds: %#v", changes.Changes)
	}
	if changes.Changes[0].ConversationID == "" || changes.Changes[0].ConversationID != conversationID {
		t.Fatalf("first change does not identify the created conversation: %#v", changes.Changes[0])
	}
	owner := model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	projectedPromptCount := 0
	for _, change := range changes.Changes {
		if change.Kind != "prompt_appended" {
			continue
		}
		event, err := auditprojection.Project(owner, change)
		if err != nil {
			t.Fatalf("project committed Prompt change %#v: %v", change, err)
		}
		replayed, err := auditprojection.Project(owner, change)
		if err != nil || replayed != event {
			t.Fatalf("repeat projection changed event identity: first=%#v repeat=%#v err=%v", event, replayed, err)
		}
		encoded, err := json.Marshal(event)
		if err != nil {
			t.Fatal(err)
		}
		for _, privateValue := range []string{
			"run the bounded task", "follow-up storage only", "cross-process-prompt-1",
			"cross-process-prompt-2", "access_token", `"content"`, `"token"`,
		} {
			if strings.Contains(string(encoded), privateValue) {
				t.Fatalf("audit projection leaked %q: %s", privateValue, encoded)
			}
		}
		if event.Actor.ID == owner.Subject || len(event.Actor.ID) != 64 || event.Actor.Type != "user" ||
			event.TenantID != owner.TenantID ||
			event.AggregateID != conversationID || event.OperationID != change.EntityID ||
			event.AggregateVersion != int64(change.AggregateVersion) ||
			event.EventID == "" || event.EventID != event.IdempotencyKey || event.Payload.ContentIncluded {
			t.Fatalf("projection did not preserve only verified, minimized change metadata: %#v", event)
		}
		projectedPromptCount++
	}
	if projectedPromptCount != 2 {
		t.Fatalf("projected prompt change count=%d, want 2", projectedPromptCount)
	}
}

func assertIntegrationNoNormalRun(t *testing.T, handler http.Handler, identity *conversationTestIdentity, conversationID string) {
	t.Helper()
	runListResponse := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationCollectionPath+"/"+conversationID+"/runs?limit=25",
		"forge:conversations:read", "", "", "")
	var runPage runmodel.OwnedRunPage
	if err := json.Unmarshal(runListResponse.Body.Bytes(), &runPage); err != nil ||
		runListResponse.Code != http.StatusOK || runPage.ConversationID != conversationID ||
		runPage.Runs == nil || len(runPage.Runs) != 0 || runPage.HasMore ||
		strings.Contains(runListResponse.Body.String(), "execution_json") {
		t.Fatalf("owner Run page status=%d page=%#v body=%q decode=%v",
			runListResponse.Code, runPage, runListResponse.Body.String(), err)
	}
}

func assertIntegrationForeignAccess(t *testing.T, handler http.Handler, identity *conversationTestIdentity, projectID, conversationID string) {
	t.Helper()
	promptPath := conversationCollectionPath + "/" + conversationID + "/prompts"
	changesTarget := conversationChangesPath + "?after_cursor=0&limit=50"
	otherCreate := requestConversationAPIAs(t, handler, identity, http.MethodPost, conversationCollectionPath,
		"forge:conversations:write", "account-other", "tenant-slate", "application/json", "create-other-owner-1",
		`{"scope":{"kind":"project","id":"`+projectID+`"},"title":"Other owner's work"}`)
	if otherCreate.Code != http.StatusForbidden || strings.Contains(otherCreate.Body.String(), "project") {
		t.Fatalf("unconfigured account Project scope create status=%d body=%q", otherCreate.Code, otherCreate.Body.String())
	}

	ownerTail := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationChangesPath+"?after_cursor=5&limit=50", "forge:conversations:read", "", "", "")
	var ownerTailPage model.OwnedConversationChangePage
	if err := json.Unmarshal(ownerTail.Body.Bytes(), &ownerTailPage); err != nil ||
		ownerTail.Code != http.StatusOK || len(ownerTailPage.Changes) != 0 || ownerTailPage.ScannedThroughCursor != 5 {
		t.Fatalf("owner cursor advanced on foreign writes: status=%d page=%#v body=%q decode=%v",
			ownerTail.Code, ownerTailPage, ownerTail.Body.String(), err)
	}

	foreignToken := identity.tokenAs("forge:conversations:read", "account-other", "tenant-slate")
	request := httptest.NewRequest(http.MethodGet, "http://127.0.0.1:7467"+promptPath, nil)
	request.Header.Set("Authorization", "Bearer "+foreignToken)
	responseRecorder := httptest.NewRecorder()
	handler.ServeHTTP(responseRecorder, request)
	if responseRecorder.Code != http.StatusForbidden {
		t.Fatalf("foreign owner status=%d body=%q", responseRecorder.Code, responseRecorder.Body.String())
	}
	foreignChanges := httptest.NewRequest(http.MethodGet, "http://127.0.0.1:7467"+changesTarget, nil)
	foreignChanges.Header.Set("Authorization", "Bearer "+foreignToken)
	foreignResponse := httptest.NewRecorder()
	handler.ServeHTTP(foreignResponse, foreignChanges)
	if foreignResponse.Code != http.StatusForbidden {
		t.Fatalf("unconfigured owner change feed status=%d body=%q", foreignResponse.Code, foreignResponse.Body.String())
	}
}

func seedRuntimeConversationScopes(t *testing.T, executable, runtimeState string) (string, string, string) {
	t.Helper()
	projectPath := t.TempDir()
	projectCommand := exec.Command(executable, "--state-dir", runtimeState, "--json", "-C", projectPath,
		"session", "new", "--title", "local project seed")
	projectCommand.Env = []string{}
	projectOutput, err := projectCommand.CombinedOutput()
	if err != nil {
		t.Fatalf("seed local Project scope: %v: %s", err, projectOutput)
	}
	var projectResult struct {
		Session model.Conversation `json:"session"`
	}
	if err := json.Unmarshal(projectOutput, &projectResult); err != nil ||
		projectResult.Session.Scope.Kind != "project" || projectResult.Session.Scope.ID == "" {
		t.Fatalf("local Project seed=%#v output=%q decode=%v", projectResult, projectOutput, err)
	}
	groupCommand := exec.Command(executable, "--state-dir", runtimeState, "--json", "group", "create", "local group seed")
	groupCommand.Env = []string{}
	groupOutput, err := groupCommand.CombinedOutput()
	if err != nil {
		t.Fatalf("seed local Group scope: %v: %s", err, groupOutput)
	}
	var groupResult struct {
		Group model.SessionGroup `json:"group"`
	}
	if err := json.Unmarshal(groupOutput, &groupResult); err != nil || groupResult.Group.ID == "" {
		t.Fatalf("local Group seed=%#v output=%q decode=%v", groupResult, groupOutput, err)
	}
	return projectPath, projectResult.Session.Scope.ID, groupResult.Group.ID
}

func initializeRuntimeHubForIntegration(t *testing.T, executable, runtimeState string) {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, executable, "--state-dir", runtimeState, "session", "list")
	command.Env = []string{}
	_, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("initialize Rust Hub integration state: %v", err)
	}
}

func requestConversationAPI(
	t *testing.T,
	handler http.Handler,
	identity *conversationTestIdentity,
	method, target, scopes, contentType, idempotencyKey string,
	body string,
) *httptest.ResponseRecorder {
	return requestConversationAPIToken(t, handler, identity.token(scopes), method, target, contentType, idempotencyKey, body)
}

func requestConversationAPIAs(
	t *testing.T,
	handler http.Handler,
	identity *conversationTestIdentity,
	method, target, scopes, subject, tenant, contentType, idempotencyKey, body string,
) *httptest.ResponseRecorder {
	return requestConversationAPIToken(t, handler, identity.tokenAs(scopes, subject, tenant),
		method, target, contentType, idempotencyKey, body)
}

func requestConversationAPIToken(
	t *testing.T,
	handler http.Handler,
	token, method, target, contentType, idempotencyKey, body string,
) *httptest.ResponseRecorder {
	t.Helper()
	var requestBody io.Reader
	if body != "" {
		requestBody = strings.NewReader(body)
	}
	request := httptest.NewRequest(method, "http://127.0.0.1:7467"+target, requestBody)
	request.Header.Set("Authorization", "Bearer "+token)
	if contentType != "" {
		request.Header.Set("Content-Type", contentType)
	}
	if idempotencyKey != "" {
		request.Header.Set("Idempotency-Key", idempotencyKey)
	}
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, request)
	return response
}
