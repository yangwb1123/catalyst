//go:build linux && !android

package appserver

// This opt-in test crosses the real Snaplink JWT verifier and Chromium for
// the owner-scoped Conversation SSE boundary. The browser request is a
// metadata-only read and cannot create Prompts, Runs, devices, leases, or
// Runner work.

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"sync"
	"testing"
	"time"

	"forgeos/forge-core/internal/authn"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
)

func TestSnaplinkAuthenticatedConsoleConversationChangesStreamBrowserE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_BROWSER_E2E") != "1" ||
		os.Getenv("FORGE_CONVERSATION_CHANGES_STREAM_BROWSER_E2E") != "1" {
		t.Skip("set FORGE_BROWSER_E2E=1 and FORGE_CONVERSATION_CHANGES_STREAM_BROWSER_E2E=1 for Console Web SSE E2E")
	}

	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)
	authenticator, err := authn.New(authn.Config{
		Issuer: issuer, Audience: snaplinkForgeTestAudience,
		JWKSURL:          issuer + "/.well-known/jwks.json",
		ExpectedTenantID: snaplinkForgeTestTenant, ExpectedSubjectID: snaplinkForgeTestUser,
		JWKSHTTPClient: ssoClient, JWKSRefreshInterval: 24 * time.Hour,
	})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(authenticator.Close)

	conversationID := "console-browser-stream-conversation"
	owner := model.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	backend := &fakeConversationBackend{
		listPage: model.OwnedConversationPage{
			Conversations: []model.OwnedConversationEntry{{
				Conversation: model.Conversation{
					ID: conversationID, Scope: model.ConversationScope{Kind: "global"},
					Title: "Console browser stream fixture", CreatedAtMS: 1, UpdatedAtMS: 1,
				},
				AggregateVersion: 1,
			}},
		},
		promptPage: model.ConversationPromptPage{
			ConversationID: conversationID, Prompts: []model.ConversationPrompt{}, HasMore: false,
		},
		runPage: runmodel.OwnedRunPage{
			ConversationID: conversationID, Runs: []runmodel.OwnedRunSummary{}, HasMore: false,
		},
		changePage: model.OwnedConversationChangePage{
			AfterCursor: 0, ScannedThroughCursor: 1, HasMore: false,
			Changes: []model.Change{{
				Cursor: 1, SchemaVersion: 1,
				ConversationID: conversationID, EntityID: conversationID,
				AggregateVersion: 1, Kind: "conversation_created", CreatedAtMS: 1,
			}},
		},
	}
	var mu sync.Mutex
	var streamAccept, streamAuthorization string
	var postCount int
	baseRoutes := newConversationRoutesWithBackend(backend)
	routes := http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		if request.URL.Path == conversationChangesStreamPath {
			mu.Lock()
			streamAccept = request.Header.Get("Accept")
			streamAuthorization = request.Header.Get("Authorization")
			mu.Unlock()
		}
		if request.Method == http.MethodPost {
			mu.Lock()
			postCount++
			mu.Unlock()
		}
		baseRoutes.ServeHTTP(writer, request)
	})
	serverHandler := http.Handler(authenticator.Handler(routes))
	webBuildDir := os.Getenv("FORGE_WEB_BUILD_DIR")
	if webBuildDir == "" {
		t.Fatal("FORGE_WEB_BUILD_DIR is required when FORGE_BROWSER_E2E=1")
	}
	serverHandler = serveForgeConsoleWebAssets(webBuildDir, serverHandler)
	server := httptest.NewServer(serverHandler)
	t.Cleanup(server.Close)

	expectedPage, err := json.Marshal(backend.changePage)
	if err != nil {
		t.Fatal(err)
	}
	input := struct {
		PageURL     string          `json:"page_url"`
		AccessToken string          `json:"access_token"`
		Expected    json.RawMessage `json:"expected_page"`
	}{
		PageURL: server.URL + "/forge/", AccessToken: token, Expected: expectedPage,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatal(err)
	}
	inputPath := filepath.Join(t.TempDir(), "console-browser-conversation-changes-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatal(err)
	}

	pythonBinary := os.Getenv("FORGE_BROWSER_PYTHON")
	if pythonBinary == "" {
		pythonBinary = "python3"
	}
	pythonExecutable, err := exec.LookPath(pythonBinary)
	if err != nil {
		t.Fatalf("Python is required for Console Web SSE E2E: %v", err)
	}
	workingDirectory, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	repoRoot := filepath.Clean(filepath.Join(workingDirectory, "..", "..", ".."))
	runnerPath := filepath.Join(repoRoot, "scripts", "forge_console_browser_conversation_changes_stream_e2e.py")
	if _, err := os.Stat(runnerPath); err != nil {
		t.Fatalf("Console Web Conversation SSE runner not found at %s: %v", runnerPath, err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, pythonExecutable, runnerPath, inputPath)
	env := os.Environ()
	env = append(env, "FORGE_BROWSER_PYTHON="+pythonExecutable)
	if browserExecutable := os.Getenv("FORGE_BROWSER_EXECUTABLE"); browserExecutable != "" {
		env = append(env, "FORGE_BROWSER_EXECUTABLE="+browserExecutable)
	}
	command.Env = env
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Forge Web Conversation SSE E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("Forge Web Conversation SSE E2E output exceeded the size limit")
	}

	mu.Lock()
	gotAccept, gotAuthorization, gotPosts := streamAccept, streamAuthorization, postCount
	mu.Unlock()
	if gotAccept != "text/event-stream" || gotAuthorization != "Bearer "+token {
		t.Fatalf("Console Web SSE headers accept=%q authorization=%q", gotAccept, gotAuthorization)
	}
	if gotPosts != 0 {
		t.Fatalf("Console Web SSE issued unexpected POST count=%d", gotPosts)
	}
	if backend.changeCalls != 1 || backend.changeOwner != owner || backend.changeAfter != 0 || backend.changeLimit != 1 {
		t.Fatalf("Console Web SSE backend calls=%d owner=%#v after=%d limit=%d want owner=%#v after=0 limit=1", backend.changeCalls, backend.changeOwner, backend.changeAfter, backend.changeLimit, owner)
	}

	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	productionRequest, err := http.NewRequest(http.MethodGet,
		server.URL+conversationChangesStreamPath+"?after_cursor=0&limit=1&wait_ms=0", nil)
	if err != nil {
		t.Fatal(err)
	}
	productionRequest.Header.Set("Authorization", "Bearer "+token)
	productionResponse := httptest.NewRecorder()
	production.ServeHTTP(productionResponse, productionRequest)
	if productionResponse.Code != http.StatusServiceUnavailable {
		t.Fatalf("default production Console Web SSE status=%d body=%q, want fail-closed 503", productionResponse.Code, productionResponse.Body.String())
	}
}
