//go:build linux && !android

package appserver

// This opt-in test drives the shared Flutter Console API through the real
// Snaplink JWT verifier and the owner-scoped Conversation SSE route. The
// fixture is metadata-only: it exposes no Prompt body, Run, device, lease,
// Runner, or Audit effect.

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
)

func TestSnaplinkAuthenticatedConsoleConversationChangesStreamE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CONVERSATION_CHANGES_STREAM_CONSOLE_E2E") != "1" {
		t.Skip("set FORGE_CONVERSATION_CHANGES_STREAM_CONSOLE_E2E=1 for Console SSE E2E")
	}
	consoleRoot := snaplinkConversationChangesStreamConsoleRoot(t)
	flutterExecutable := snaplinkConversationChangesStreamFlutter(t)

	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)
	authenticator, err := authn.New(authn.Config{
		Issuer:              issuer,
		Audience:            snaplinkForgeTestAudience,
		JWKSURL:             issuer + "/.well-known/jwks.json",
		ExpectedTenantID:    snaplinkForgeTestTenant,
		ExpectedSubjectID:   snaplinkForgeTestUser,
		JWKSHTTPClient:      ssoClient,
		JWKSRefreshInterval: 24 * time.Hour,
	})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(authenticator.Close)

	conversationID := "console-stream-conversation"
	backend := &fakeConversationBackend{changePage: model.OwnedConversationChangePage{
		AfterCursor: 0, ScannedThroughCursor: 1,
		Changes: []model.Change{{
			Cursor: 1, SchemaVersion: 1,
			ConversationID: conversationID, EntityID: conversationID,
			AggregateVersion: 1, Kind: "conversation_created", CreatedAtMS: 1,
		}},
	}}
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

	input := struct {
		APIURL      string `json:"api_url"`
		AccessToken string `json:"access_token"`
	}{APIURL: server.URL, AccessToken: token}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatal(err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "conversation-changes-stream-console-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatal(err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatal(err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub",
		"--dart-define=FORGE_CONVERSATIONS_API_ORIGIN="+server.URL,
		"test/forge_conversation_changes_stream_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_CONVERSATION_CHANGES_STREAM_CONSOLE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated Flutter Console Conversation SSE failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("Flutter Console Conversation SSE E2E output exceeded the size limit")
	}

	mu.Lock()
	gotAccept, gotAuthorization := streamAccept, streamAuthorization
	mu.Unlock()
	if gotAccept != "text/event-stream" || gotAuthorization != "Bearer "+token {
		t.Fatalf("Console SSE headers accept=%q authorization=%q", gotAccept, gotAuthorization)
	}
	requests := recorder.snapshot()
	if len(requests) != 1 || requests[0] != (recordedConversationRequest{
		method: http.MethodGet,
		path:   conversationChangesStreamPath,
		query:  "after_cursor=0&limit=128&wait_ms=0",
	}) {
		t.Fatalf("Console SSE requests=%#v; expected one owner-scoped stream GET", requests)
	}
	wantOwner := model.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	if backend.changeOwner != wantOwner || backend.changeAfter != 0 || backend.changeLimit != 128 {
		t.Fatalf("Console SSE backend owner=%#v after=%d limit=%d want owner=%#v after=0 limit=128",
			backend.changeOwner, backend.changeAfter, backend.changeLimit, wantOwner)
	}
}

func snaplinkConversationChangesStreamConsoleRoot(t *testing.T) string {
	t.Helper()
	consoleRoot := os.Getenv("SNAPLINK_CONSOLE_ROOT")
	if consoleRoot == "" {
		workingDirectory, err := os.Getwd()
		if err != nil {
			t.Fatalf("resolve Console repository: %v", err)
		}
		repoRoot := filepath.Clean(filepath.Join(workingDirectory, "..", "..", ".."))
		consoleRoot = filepath.Join(filepath.Dir(repoRoot), "workspace", "demo", "snaplink-console")
	}
	if _, err := os.Stat(filepath.Join(consoleRoot, "pubspec.yaml")); err != nil {
		t.Fatalf("SNAPLINK_CONSOLE_ROOT must name the Flutter Console repository: %v", err)
	}
	return consoleRoot
}

func snaplinkConversationChangesStreamFlutter(t *testing.T) string {
	t.Helper()
	flutterBinary := os.Getenv("FLUTTER_BIN")
	if flutterBinary == "" {
		flutterBinary = "flutter"
	}
	flutterExecutable, err := exec.LookPath(flutterBinary)
	if err != nil {
		t.Fatalf("Flutter is required for the Console Conversation SSE E2E: %v", err)
	}
	return flutterExecutable
}
