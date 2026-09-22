package appserver

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"

	"forgeos/forge-core/internal/runtimebridge"
)

func assertForgeRuntimeCLIListsOwnedConversationsAcrossPages(
	t *testing.T,
	executable, apiURL string,
	identity *conversationTestIdentity,
	bridge *runtimebridge.Client,
	recorder *conversationHTTPRecorder,
) {
	t.Helper()
	const seedCount = 130
	const scopes = "forge:conversations:read forge:conversations:write"
	owner := model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	baseline, err := bridge.ListOwnedConversations(context.Background(), owner, "", conversationPageMax)
	if err != nil || baseline.HasMore || baseline.NextAfterID != nil {
		t.Fatalf("read pagination E2E baseline: page=%#v err=%v", baseline, err)
	}

	seeder := &http.Client{Timeout: 20 * time.Second}
	seederToken := tokenForIndependentClient(identity, scopes, "cli-pagination-seeder")
	seedIDs := make(map[string]struct{}, seedCount)
	const requestBody = `{"scope":{"kind":"global"},"title":"Cross-page session fixture"}`
	for index := 0; index < seedCount; index++ {
		response := doConversationClientRequest(
			t, seeder, apiURL, seederToken, http.MethodPost, conversationCollectionPath,
			"application/json", "cli-page-seed-"+strconv.Itoa(index), requestBody,
		)
		if response.StatusCode != http.StatusCreated {
			t.Fatalf("seed session %d status=%d body=%q",
				index, response.StatusCode, readConversationClientBody(t, response))
		}
		var created model.Conversation
		if err := json.NewDecoder(response.Body).Decode(&created); err != nil || created.ID == "" {
			_ = response.Body.Close()
			t.Fatalf("seed session %d response=%#v decode=%v", index, created, err)
		}
		_ = response.Body.Close()
		seedIDs[created.ID] = struct{}{}
	}

	firstPage, err := bridge.ListOwnedConversations(context.Background(), owner, "", conversationPageMax)
	if err != nil || !firstPage.HasMore || firstPage.NextAfterID == nil {
		t.Fatalf("read first page cursor for pagination E2E: page=%#v err=%v", firstPage, err)
	}
	readerHome := t.TempDir()
	readerToken := tokenForIndependentClient(identity, scopes, "cli-pagination-reader")
	firstListRequest := len(recorder.snapshot())
	output, stderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, readerToken, readerHome,
		"--json", "remote", "sessions", "list", "--all",
	)
	if err != nil {
		t.Fatalf("CLI cross-page owner list failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var page model.OwnedConversationPage
	if err := json.Unmarshal([]byte(output), &page); err != nil {
		t.Fatalf("decode CLI cross-page owner list: %v; stdout=%q", err, output)
	}
	if len(page.Conversations) != len(baseline.Conversations)+seedCount ||
		page.HasMore || page.NextAfterID != nil {
		t.Fatalf("CLI cross-page owner list count=%d has_more=%t cursor=%v, baseline=%d seed=%d",
			len(page.Conversations), page.HasMore, page.NextAfterID, len(baseline.Conversations), seedCount)
	}
	seen := make(map[string]struct{}, len(page.Conversations))
	previousID := ""
	for _, entry := range page.Conversations {
		id := entry.Conversation.ID
		if id <= previousID {
			t.Fatalf("CLI cross-page sessions are not strictly ordered: previous=%q current=%q", previousID, id)
		}
		previousID = id
		seen[id] = struct{}{}
	}
	for id := range seedIDs {
		if _, ok := seen[id]; !ok {
			t.Fatalf("CLI cross-page owner list omitted seeded session %q", id)
		}
	}
	requests := recorder.snapshot()[firstListRequest:]
	if len(requests) != 2 ||
		requests[0] != (recordedConversationRequest{
			method: http.MethodGet, path: conversationCollectionPath, query: "limit=128",
		}) ||
		requests[1] != (recordedConversationRequest{
			method: http.MethodGet, path: conversationCollectionPath,
			query: "limit=128&after_id=" + *firstPage.NextAfterID,
		}) {
		t.Fatalf("CLI cross-page list requests=%#v; expected exactly two cursor-linked owner-page reads", requests)
	}

	foreignHome := t.TempDir()
	foreignToken := tokenForPrincipal(identity, "account-foreign", scopes, "cli-pagination-foreign")
	firstForeignRequest := len(recorder.snapshot())
	foreignOutput, foreignStderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, foreignToken, foreignHome,
		"--json", "remote", "sessions", "list", "--all",
	)
	if err != nil {
		t.Fatalf("foreign CLI all-pages list failed: stderr=%q stdout=%q err=%v",
			foreignStderr, foreignOutput, err)
	}
	var foreignPage model.OwnedConversationPage
	if err := json.Unmarshal([]byte(foreignOutput), &foreignPage); err != nil ||
		len(foreignPage.Conversations) != 0 || foreignPage.HasMore || foreignPage.NextAfterID != nil {
		t.Fatalf("foreign CLI all-pages list exposed owner sessions: page=%#v decode=%v", foreignPage, err)
	}
	foreignRequests := recorder.snapshot()[firstForeignRequest:]
	if len(foreignRequests) != 1 || foreignRequests[0] != (recordedConversationRequest{
		method: http.MethodGet, path: conversationCollectionPath, query: "limit=128",
	}) {
		t.Fatalf("foreign CLI all-pages list requests=%#v; expected one empty owner-scoped page", foreignRequests)
	}
}

func assertForgeRuntimeCLIClientsShareOwnedConversationAndPrompts(
	t *testing.T,
	executable, apiURL string,
	identity *conversationTestIdentity,
	changesAfter uint64,
) model.Conversation {
	t.Helper()
	const scopes = "forge:conversations:read forge:conversations:write"
	tokenA := tokenForIndependentClient(identity, scopes, "cli-client-a")
	tokenB := tokenForIndependentClient(identity, scopes, "cli-client-b")
	tokenC := tokenForPrincipal(identity, "account-foreign", scopes, "cli-client-c")
	clientAHome := t.TempDir()
	clientBHome := t.TempDir()
	clientCHome := t.TempDir()

	createOutput, createStderr, err := runForgeRuntimeCLI(t, executable, apiURL, tokenA, clientAHome,
		"--json", "--idempotency-key", "cli-client-a-create",
		"remote", "sessions", "create", "--scope", "global", "--title", "Shared from CLI client A")
	if err != nil {
		t.Fatalf("CLI client A create failed: stderr=%q stdout=%q err=%v", createStderr, createOutput, err)
	}
	var created model.Conversation
	if err := json.Unmarshal([]byte(createOutput), &created); err != nil || created.ID == "" || created.Title != "Shared from CLI client A" {
		t.Fatalf("CLI client A create response=%q conversation=%#v decode=%v", createOutput, created, err)
	}

	listOutput, listStderr, err := runForgeRuntimeCLI(t, executable, apiURL, tokenB, clientBHome,
		"--json", "remote", "sessions", "list")
	if err != nil {
		t.Fatalf("CLI client B list failed: stderr=%q stdout=%q err=%v", listStderr, listOutput, err)
	}
	var page model.OwnedConversationPage
	if err := json.Unmarshal([]byte(listOutput), &page); err != nil || len(page.Conversations) != 2 || page.HasMore {
		t.Fatalf("CLI client B owner page=%#v stdout=%q decode=%v", page, listOutput, err)
	}
	if !ownedConversationPageContains(page, created.ID) {
		t.Fatalf("CLI client B could not see CLI client A Conversation %q: %#v", created.ID, page)
	}

	promptContent := "Prompt submitted by remote CLI client B"
	appendOutput, appendStderr, err := runForgeRuntimeCLI(t, executable, apiURL, tokenB, clientBHome,
		"--json", "--idempotency-key", "cli-client-b-prompt",
		"remote", "prompts", "add", created.ID, "--expected-version", "1", promptContent)
	if err != nil {
		t.Fatalf("CLI client B Prompt append failed: stderr=%q stdout=%q err=%v", appendStderr, appendOutput, err)
	}

	historyOutput, historyStderr, err := runForgeRuntimeCLI(t, executable, apiURL, tokenA, clientAHome,
		"--json", "remote", "prompts", "list", created.ID)
	if err != nil {
		t.Fatalf("CLI client A Prompt history read failed: stderr=%q stdout=%q err=%v", historyStderr, historyOutput, err)
	}
	var history model.ConversationPromptPage
	if err := json.Unmarshal([]byte(historyOutput), &history); err != nil || history.ConversationID != created.ID ||
		len(history.Prompts) != 1 || history.Prompts[0].Role != "user" || history.Prompts[0].Content != promptContent {
		t.Fatalf("CLI client A Prompt history=%#v stdout=%q decode=%v", history, historyOutput, err)
	}

	changesOutput, changesStderr, err := runForgeRuntimeCLI(t, executable, apiURL, tokenA, clientAHome,
		"--json", "remote", "changes", "list", "--after-cursor", strconv.FormatUint(changesAfter, 10))
	if err != nil {
		t.Fatalf("CLI client A change feed read failed: stderr=%q stdout=%q err=%v", changesStderr, changesOutput, err)
	}
	var changes model.OwnedConversationChangePage
	if err := json.Unmarshal([]byte(changesOutput), &changes); err != nil || changes.AfterCursor != changesAfter ||
		changes.ScannedThroughCursor != changesAfter+2 || changes.HasMore || len(changes.Changes) != 2 ||
		changes.Changes[0].ConversationID != created.ID || changes.Changes[0].Kind != "conversation_created" ||
		changes.Changes[1].ConversationID != created.ID || changes.Changes[1].Kind != "prompt_appended" {
		t.Fatalf("CLI client A change feed=%#v stdout=%q decode=%v", changes, changesOutput, err)
	}

	foreignListOutput, foreignListStderr, err := runForgeRuntimeCLI(t, executable, apiURL, tokenC, clientCHome,
		"--json", "remote", "sessions", "list")
	if err != nil {
		t.Fatalf("foreign CLI client list failed: stderr=%q stdout=%q err=%v", foreignListStderr, foreignListOutput, err)
	}
	var foreignPage model.OwnedConversationPage
	if err := json.Unmarshal([]byte(foreignListOutput), &foreignPage); err != nil || len(foreignPage.Conversations) != 0 || foreignPage.HasMore {
		t.Fatalf("foreign CLI client received owner data: page=%#v stdout=%q decode=%v", foreignPage, foreignListOutput, err)
	}
	foreignHistoryOutput, foreignHistoryStderr, err := runForgeRuntimeCLI(t, executable, apiURL, tokenC, clientCHome,
		"--json", "remote", "prompts", "list", created.ID)
	if err == nil || !strings.Contains(foreignHistoryStderr, "HTTP 404 (not_found)") {
		t.Fatalf("foreign CLI client Conversation read should be hidden: stderr=%q stdout=%q err=%v",
			foreignHistoryStderr, foreignHistoryOutput, err)
	}

	runsOutput, runsStderr, err := runForgeRuntimeCLI(t, executable, apiURL, tokenA, clientAHome,
		"--json", "remote", "runs", "list", created.ID, "--limit", "25")
	if err != nil {
		t.Fatalf("CLI client A Run observation failed: stderr=%q stdout=%q err=%v", runsStderr, runsOutput, err)
	}
	var runPage runmodel.OwnedRunPage
	if err := json.Unmarshal([]byte(runsOutput), &runPage); err != nil || runPage.ConversationID != created.ID ||
		runPage.Runs == nil || len(runPage.Runs) != 0 || runPage.HasMore {
		t.Fatalf("Prompt append unexpectedly started a Run: page=%#v stdout=%q decode=%v", runPage, runsOutput, err)
	}
	return created
}

func assertNoPendingRunIntentAfterPrompt(
	t *testing.T,
	client *runtimebridge.Client,
	identity *conversationTestIdentity,
	conversationID string,
) {
	t.Helper()
	owner := model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	page, err := client.OwnedConversationPendingRunIntents(context.Background(), owner, conversationID, nil, 25)
	if err != nil || page.ConversationID != conversationID || len(page.Intents) != 0 || page.NextCursor != nil || page.HasMore {
		t.Fatalf("storage-only Prompt created a pending Run intent: page=%#v err=%v", page, err)
	}
}

func ownedConversationPageContains(page model.OwnedConversationPage, conversationID string) bool {
	for _, entry := range page.Conversations {
		if entry.Conversation.ID == conversationID {
			return true
		}
	}
	return false
}

func runForgeRuntimeCLI(
	t *testing.T,
	executable, apiURL, accessToken, home string,
	args ...string,
) (stdout, stderr string, err error) {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, executable, args...)
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	err = command.Run()
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		return stdoutBuffer.String(), stderrBuffer.String(), errors.New("forge-runtime CLI output exceeded the size limit")
	}
	return stdoutBuffer.String(), stderrBuffer.String(), err
}

func runForgeRuntimeTUI(t *testing.T, executable, apiURL, accessToken, conversationID string) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("the live TUI integration requires a PTY launcher named script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("open " + conversationID + "\nprompt " + forgeRuntimeTUIPromptContent + "\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Forge remote TUI failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Forge remote TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"Shared from CLI client A",
		"prompt sent from client B",
		"Prompt stored. No Run was started.",
		forgeRuntimeTUIPromptContent,
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("Forge remote TUI output omitted %q: %q", want, output)
		}
	}
}

func forgeRuntimeCLIEnvironment(apiURL, accessToken, home string) []string {
	env := make([]string, 0, 8)
	for _, value := range os.Environ() {
		key, _, _ := strings.Cut(value, "=")
		switch strings.ToUpper(key) {
		case "PATH", "SYSTEMROOT", "WINDIR", "TMP", "TEMP", "TMPDIR":
			env = append(env, value)
		}
	}
	return append(env,
		"FORGE_API_URL="+apiURL,
		"FORGE_ACCESS_TOKEN="+accessToken,
		"HOME="+home,
		"USERPROFILE="+home,
		"XDG_CONFIG_HOME="+filepath.Join(home, "config"),
		"XDG_STATE_HOME="+filepath.Join(home, "state"),
	)
}

const maxForgeRuntimeCLIOutputBytes = 2 * 1024 * 1024
const forgeConsoleWidgetPromptContent = "Prompt submitted from Flutter Console screen"
const forgeRuntimeTUIPromptContent = "Prompt submitted from Forge remote TUI"
const forgeConsoleBrowserPromptContent = "  Prompt submitted from Forge Console browser\n"

type boundedCLIOutput struct {
	buffer   bytes.Buffer
	exceeded bool
}

func (output *boundedCLIOutput) Write(value []byte) (int, error) {
	remaining := maxForgeRuntimeCLIOutputBytes - output.buffer.Len()
	if remaining <= 0 {
		output.exceeded = true
		return len(value), nil
	}
	if len(value) > remaining {
		_, _ = output.buffer.Write(value[:remaining])
		output.exceeded = true
		return len(value), nil
	}
	return output.buffer.Write(value)
}

func (output *boundedCLIOutput) String() string {
	return output.buffer.String()
}

type recordedConversationRequest struct {
	method string
	path   string
	query  string
}

type conversationHTTPRecorder struct {
	mu       sync.Mutex
	requests []recordedConversationRequest
}

func (recorder *conversationHTTPRecorder) wrap(next http.Handler) http.Handler {
	return http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		recorder.mu.Lock()
		recorder.requests = append(recorder.requests, recordedConversationRequest{
			method: request.Method,
			path:   request.URL.EscapedPath(),
			query:  request.URL.RawQuery,
		})
		recorder.mu.Unlock()
		next.ServeHTTP(writer, request)
	})
}

func (recorder *conversationHTTPRecorder) snapshot() []recordedConversationRequest {
	recorder.mu.Lock()
	defer recorder.mu.Unlock()
	return append([]recordedConversationRequest(nil), recorder.requests...)
}

func assertForgeRuntimeCLIRequestsHaveNoExecutionOrDeviceEffects(
	t *testing.T,
	requests []recordedConversationRequest,
	changesAfter uint64,
) {
	t.Helper()
	if len(requests) != 8 {
		t.Fatalf("CLI issued %d HTTP requests; expected only the 8 shared-session/read-only verification calls: %#v", len(requests), requests)
	}
	conversationID, ok := conversationIDFromPromptPath(requests[2].path)
	if !ok {
		t.Fatalf("CLI append request path is invalid: %#v", requests[2])
	}
	want := []recordedConversationRequest{
		{method: http.MethodPost, path: conversationCollectionPath},
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodPost, path: conversationCollectionPath + "/" + conversationID + "/prompts"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/prompts", query: "limit=128"},
		{method: http.MethodGet, path: conversationChangesPath, query: "after_cursor=" + strconv.FormatUint(changesAfter, 10) + "&limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/prompts", query: "limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/runs", query: "limit=25"},
	}
	for index, request := range requests {
		if request != want[index] {
			t.Fatalf("CLI request[%d]=%#v want=%#v (device and Run effect routes must remain untouched)", index, request, want[index])
		}
	}
}

func assertForgeRuntimeTUIRequestsHaveNoExecutionOrDeviceEffects(
	t *testing.T,
	requests []recordedConversationRequest,
	conversationID string,
) {
	t.Helper()
	promptPath := conversationCollectionPath + "/" + conversationID + "/prompts"
	want := []recordedConversationRequest{
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: promptPath, query: "limit=128"},
		{method: http.MethodPost, path: promptPath},
		{method: http.MethodGet, path: promptPath, query: "limit=128"},
	}
	if len(requests) != len(want) {
		t.Fatalf("TUI issued %d HTTP requests; expected only the 4 shared-session calls: %#v", len(requests), requests)
	}
	for index, request := range requests {
		if request != want[index] {
			t.Fatalf("TUI request[%d]=%#v want=%#v (Run effect and device routes must remain untouched)", index, request, want[index])
		}
	}
}
