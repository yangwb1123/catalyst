package appserver

// This test proves that the authenticated Rust TUI carries its owner-bound
// Conversation change-feed cursor across two independent processes. It uses
// the same saved credential file path as the normal CLI/TUI client, while the
// Forge server is a bounded recorder fixture. The test remains read-only: no
// Prompt, Run, device, inventory, or execution request is available.

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestSavedCredentialRustTUIChangeCursorResumesAcrossProcessesWhenConfigured(t *testing.T) {
	executable, ptyScript := savedCredentialTUIExecutables(t)
	issuer, _, _, accessToken, closeIssuer := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeIssuer)
	home := savedCredentialTUIHome(t)
	writeForgeSavedCredential(t, home, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant, accessToken)

	recorder := &conversationHTTPRecorder{}
	server := newSavedCredentialTUIChangeFeedServer(recorder)
	t.Cleanup(server.Close)

	first := runSavedCredentialTUI(t, ptyScript, executable, server.URL, issuer, home)
	if !strings.Contains(first, "Synced 1 owner-visible changes through cursor 1") {
		t.Fatalf("first TUI process did not commit cursor 1: %q", first)
	}
	second := runSavedCredentialTUI(t, ptyScript, executable, server.URL, issuer, home)
	if !strings.Contains(second, "Synced 0 owner-visible changes through cursor 1") ||
		strings.Contains(second, "Synced 1 owner-visible changes through cursor 1") {
		t.Fatalf("second TUI process did not resume the saved cursor: %q", second)
	}

	assertSavedCredentialTUIRequests(t, recorder.snapshot())
	assertSavedChangeCursorBinding(t, home, server.URL, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant, 1)
	if strings.Contains(first+second, "/devices") || strings.Contains(first+second, "Run started") {
		t.Fatalf("TUI output exposed an execution/device path: %q", first+second)
	}
}

func savedCredentialTUIExecutables(t *testing.T) (string, string) {
	t.Helper()
	executable := os.Getenv("FORGE_RUNTIME_BIN")
	if executable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for the saved-credential Rust TUI change cursor integration")
	}
	if _, err := os.Stat(executable); err != nil {
		t.Fatalf("FORGE_RUNTIME_BIN=%q: %v", executable, err)
	}
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Skipf("saved-credential Rust TUI change cursor integration requires script: %v", err)
	}
	return executable, ptyScript
}

func savedCredentialTUIHome(t *testing.T) string {
	t.Helper()
	userHome, err := os.UserHomeDir()
	if err != nil {
		t.Fatalf("locate private user home for TUI cursor: %v", err)
	}
	home, err := os.MkdirTemp(userHome, ".forge-tui-change-cursor-")
	if err != nil {
		t.Fatalf("create private TUI cursor home: %v", err)
	}
	t.Cleanup(func() { _ = os.RemoveAll(home) })
	return home
}

func newSavedCredentialTUIChangeFeedServer(recorder *conversationHTTPRecorder) *httptest.Server {
	return httptest.NewServer(recorder.wrap(http.HandlerFunc(serveSavedCredentialTUIChangeFeed)))
}

func serveSavedCredentialTUIChangeFeed(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	switch {
	case r.Method == http.MethodGet && r.URL.Path == conversationCollectionPath:
		_, _ = w.Write([]byte(`{"conversations":[{"conversation":{"id":"tui-change-cursor-conversation","scope":{"kind":"global"},"title":"TUI change cursor fixture","created_at_ms":1,"updated_at_ms":1},"aggregate_version":1}],"has_more":false}`))
	case r.Method == http.MethodGet && r.URL.Path == conversationChangesPath:
		serveSavedCredentialTUIChanges(w, r.URL.Query().Get("after_cursor"))
	case r.Method == http.MethodGet && r.URL.Path == conversationCollectionPath+"/tui-change-cursor-conversation/prompts":
		_, _ = w.Write([]byte(`{"conversation_id":"tui-change-cursor-conversation","prompts":[],"has_more":false}`))
	default:
		http.Error(w, "unexpected Forge request", http.StatusNotFound)
	}
}

func serveSavedCredentialTUIChanges(w http.ResponseWriter, cursor string) {
	switch cursor {
	case "0":
		_, _ = w.Write([]byte(`{"after_cursor":0,"scanned_through_cursor":1,"has_more":false,"changes":[{"cursor":1,"schema_version":1,"conversation_id":"tui-change-cursor-conversation","entity_id":"tui-change-cursor-conversation","aggregate_version":1,"kind":"conversation_created","created_at_ms":1}]}`))
	case "1":
		_, _ = w.Write([]byte(`{"after_cursor":1,"scanned_through_cursor":1,"has_more":false,"changes":[]}`))
	default:
		http.Error(w, "unexpected change cursor", http.StatusBadRequest)
	}
}

func runSavedCredentialTUI(t *testing.T, ptyScript, executable, apiURL, issuer, home string) string {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeSavedCredentialEnvironment(
		apiURL, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant, home,
	)
	command.Stdin = strings.NewReader("sync\nquit\n")
	var stdout, stderr boundedCLIOutput
	command.Stdout = &stdout
	command.Stderr = &stderr
	if err := command.Run(); err != nil {
		t.Fatalf("saved-credential Rust TUI failed: stdout=%q stderr=%q err=%v", stdout.String(), stderr.String(), err)
	}
	if stdout.exceeded || stderr.exceeded {
		t.Fatal("saved-credential Rust TUI output exceeded the size limit")
	}
	return stdout.String()
}

func assertSavedCredentialTUIRequests(t *testing.T, requests []recordedConversationRequest) {
	t.Helper()
	want := []recordedConversationRequest{
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: conversationChangesPath, query: "after_cursor=0&limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath + "/tui-change-cursor-conversation/prompts", query: "limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: conversationChangesPath, query: "after_cursor=1&limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath + "/tui-change-cursor-conversation/prompts", query: "limit=128"},
	}
	if len(requests) != len(want) {
		t.Fatalf("saved-credential TUI issued %d requests, want %#v got %#v", len(requests), want, requests)
	}
	for index := range want {
		if requests[index] != want[index] {
			t.Fatalf("saved-credential TUI request[%d]=%#v want=%#v", index, requests[index], want[index])
		}
	}
}

func assertSavedChangeCursorBinding(t *testing.T, home, coordinator, issuer, subject, tenant string, wantCursor uint64) {
	t.Helper()
	directory := filepath.Join(home, "config", "forge-runtime", "credentials")
	entries, err := os.ReadDir(directory)
	if err != nil {
		t.Fatalf("read saved credential directory: %v", err)
	}
	found := false
	for _, entry := range entries {
		if entry.IsDir() || filepath.Ext(entry.Name()) != ".cursor" {
			continue
		}
		bytes, err := os.ReadFile(filepath.Join(directory, entry.Name()))
		if err != nil {
			t.Fatalf("read saved change cursor: %v", err)
		}
		var value struct {
			SchemaVersion uint8  `json:"schema_version"`
			Coordinator   string `json:"coordinator"`
			Issuer        string `json:"issuer"`
			ClientID      string `json:"client_id"`
			Subject       string `json:"subject"`
			TenantID      string `json:"tenant_id"`
			Cursor        uint64 `json:"cursor"`
		}
		if err := json.Unmarshal(bytes, &value); err != nil {
			t.Fatalf("decode saved change cursor: %v", err)
		}
		coordinatorMatches := strings.TrimRight(value.Coordinator, "/") == strings.TrimRight(coordinator, "/")
		if value.SchemaVersion == 1 && coordinatorMatches && value.Issuer == issuer &&
			value.ClientID == "forge-cli" && value.Subject == subject && value.TenantID == tenant && value.Cursor == wantCursor {
			found = true
		}
	}
	if !found {
		t.Fatalf("saved change cursor did not retain coordinator/owner binding or cursor %d", wantCursor)
	}
}
