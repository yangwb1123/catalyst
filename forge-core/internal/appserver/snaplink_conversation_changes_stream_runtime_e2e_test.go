//go:build linux && !android

package appserver

// This opt-in test drives both Runtime clients through the real Snaplink JWT
// verifier and the owner-scoped Conversation SSE route. It remains metadata
// only: the fixture exposes no Prompt body, Run, device, lease, Runner, or
// Audit effect.

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"forgeos/forge-core/internal/authn"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestSnaplinkAuthenticatedRuntimeConversationChangesStreamE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_RUNTIME_CONVERSATION_CHANGES_STREAM_E2E") != "1" {
		t.Skip("set FORGE_RUNTIME_CONVERSATION_CHANGES_STREAM_E2E=1 for Runtime CLI/TUI SSE E2E")
	}
	configuredExecutable := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredExecutable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for Runtime CLI/TUI SSE E2E")
	}
	executable, err := exec.LookPath(configuredExecutable)
	if err != nil {
		t.Fatalf("FORGE_RUNTIME_BIN must name a built forge-runtime executable available to the test: %v", err)
	}
	executable, err = filepath.Abs(executable)
	if err != nil {
		t.Fatalf("resolve forge-runtime executable path: %v", err)
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

	owner := model.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	conversationID := "runtime-stream-conversation"
	backend := &fakeConversationBackend{
		listPage: model.OwnedConversationPage{
			Conversations: []model.OwnedConversationEntry{{
				Conversation: model.Conversation{
					ID: conversationID, Scope: model.ConversationScope{Kind: "global"},
					Title: "Runtime stream fixture", CreatedAtMS: 1, UpdatedAtMS: 1,
				},
				AggregateVersion: 1,
			}},
		},
		changePage: model.OwnedConversationChangePage{
			AfterCursor: 0, ScannedThroughCursor: 1,
			Changes: []model.Change{{
				Cursor: 1, SchemaVersion: 1, ConversationID: conversationID,
				EntityID: conversationID, AggregateVersion: 1,
				Kind: "conversation_created", CreatedAtMS: 1,
			}},
		},
	}
	recorder := &conversationHTTPRecorder{}
	var mu sync.Mutex
	var streamAccept, streamAuthorization string
	baseRoutes := newConversationRoutesWithBackend(backend)
	serverRoutes := http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		if request.URL.EscapedPath() == conversationChangesStreamPath {
			mu.Lock()
			streamAccept = request.Header.Get("Accept")
			streamAuthorization = request.Header.Get("Authorization")
			mu.Unlock()
		}
		baseRoutes.ServeHTTP(writer, request)
	})
	server := httptest.NewServer(authenticator.Handler(recorder.wrap(serverRoutes)))
	t.Cleanup(server.Close)

	cliOutput, cliStderr, err := runForgeRuntimeCLI(
		t, executable, server.URL, token, t.TempDir(),
		"--json", "remote", "changes", "stream", "--after-cursor", "0", "--wait-ms", "0",
	)
	if err != nil {
		t.Fatalf("authenticated Runtime CLI change stream failed: stdout=%q stderr=%q err=%v", cliOutput, cliStderr, err)
	}
	var cliPage map[string]any
	if err := json.Unmarshal([]byte(cliOutput), &cliPage); err != nil {
		t.Fatalf("decode authenticated Runtime CLI change stream: %v stdout=%q", err, cliOutput)
	}
	if cliPage["start_cursor"] != float64(0) || cliPage["scanned_through_cursor"] != float64(1) ||
		cliPage["timed_out"] != false {
		t.Fatalf("authenticated Runtime CLI change stream page=%#v stdout=%q", cliPage, cliOutput)
	}

	tuiOutput := runForgeRuntimeConversationChangesStreamTUI(t, executable, server.URL, token)
	if !strings.Contains(tuiOutput, "Changes stream start_cursor=0 scanned_through_cursor=1") ||
		!strings.Contains(tuiOutput, "Change cursor=1 conversation=\"runtime-stream-conversation\"") {
		t.Fatalf("authenticated Runtime TUI change stream output omitted validated page: %q", tuiOutput)
	}

	mu.Lock()
	gotAccept, gotAuthorization := streamAccept, streamAuthorization
	mu.Unlock()
	if gotAccept != "text/event-stream" || gotAuthorization != "Bearer "+token {
		t.Fatalf("Runtime SSE headers accept=%q authorization=%q", gotAccept, gotAuthorization)
	}
	requests := recorder.snapshot()
	streamRequests := 0
	for _, request := range requests {
		if request.path == conversationChangesStreamPath {
			streamRequests++
		}
		if request.method == http.MethodPost {
			t.Fatalf("Runtime change stream emitted an unexpected POST: %#v", requests)
		}
	}
	if streamRequests != 2 {
		t.Fatalf("Runtime CLI/TUI stream request count=%d requests=%#v", streamRequests, requests)
	}
	if backend.changeOwner != owner || backend.changeAfter != 0 || backend.changeLimit != 128 {
		t.Fatalf("Runtime stream backend owner=%#v after=%d limit=%d want owner=%#v after=0 limit=128", backend.changeOwner, backend.changeAfter, backend.changeLimit, owner)
	}
}

func runForgeRuntimeConversationChangesStreamTUI(
	t *testing.T,
	executable, apiURL, accessToken string,
) string {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("Runtime change stream TUI E2E requires a PTY launcher named script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("changes stream --after-cursor 0 --wait-ms 0\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated Runtime TUI change stream failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("Runtime TUI change stream output exceeded the size limit")
	}
	return stdoutBuffer.String()
}
