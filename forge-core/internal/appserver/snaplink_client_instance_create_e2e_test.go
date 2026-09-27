package appserver

import (
	"context"
	"encoding/json"
	"net/http"
	"os/exec"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/runtimebridge/model"
)

// assertTUIInstanceCreatePreflight drives the interactive Runtime TUI through
// the same authenticated candidate mux as the CLI create journey. The TUI
// reads a converged pair once to select the display projection and refreshes
// that pair again immediately before its owner-wide Conversation POST. The
// created Conversation is intentionally absent from the fixture declaration,
// so the TUI must keep it outside the selected instance projection.
func assertTUIInstanceCreatePreflight(
	t *testing.T,
	executable, apiURL, accessToken string,
	recorder *conversationHTTPRecorder,
) {
	t.Helper()
	requestStart := len(recorder.snapshot())
	output := runForgeRuntimeInstanceCreateTUI(t, executable, apiURL, accessToken)
	for _, want := range []string{
		"Created session ",
		"outside the selected client-instance display projection; it remains unselected.",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("instance-scoped TUI create omitted %q: %q", want, output)
		}
	}
	requests := recorder.snapshot()[requestStart:]
	want := []recordedConversationRequest{
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: clientInstanceSessionViewCandidatePath},
		{method: http.MethodGet, path: clientInstanceResourceViewCandidatePath},
		{method: http.MethodGet, path: clientInstanceSessionViewCandidatePath},
		{method: http.MethodGet, path: clientInstanceResourceViewCandidatePath},
		{method: http.MethodPost, path: conversationCollectionPath},
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
	}
	if len(requests) != len(want) {
		t.Fatalf("instance-scoped TUI create issued %d requests; want %#v got %#v", len(requests), want, requests)
	}
	for index, request := range requests {
		if request != want[index] {
			t.Fatalf("instance-scoped TUI create request[%d]=%#v want=%#v", index, request, want[index])
		}
	}
	assertClientInstanceProjectionRequestsHaveNoExecutionOrDeviceEffects(t, requests)
}

func runForgeRuntimeInstanceCreateTUI(
	t *testing.T,
	executable, apiURL, accessToken string,
) string {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("instance-scoped TUI create E2E requires script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader(
		"client-instances show-converged\n" +
			"instance client-tui-001\n" +
			"create TUI instance preflight create\n" +
			"quit\n",
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("instance-scoped TUI Conversation create failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("instance-scoped TUI Conversation create output exceeded the size limit")
	}
	return stdoutBuffer.String()
}

// assertCLIInstanceCreatePreflight drives the real Runtime CLI through the
// authenticated candidate mux used by the five-client projection journey.
// Creation remains an owner-wide storage write; the instance declaration is a
// fail-closed display preflight, not a membership writer.
func assertCLIInstanceCreatePreflight(
	t *testing.T,
	executable, apiURL, accessToken string,
	recorder *conversationHTTPRecorder,
) {
	t.Helper()
	requestStart := len(recorder.snapshot())
	output, stderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "--idempotency-key", "projection-cli-instance-create",
		"remote", "sessions", "create", "--instance", "client-cli-001",
		"--title", "CLI instance preflight create",
	)
	if err != nil {
		t.Fatalf("CLI instance Conversation create failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var created model.Conversation
	if err := json.Unmarshal([]byte(output), &created); err != nil || created.ID == "" ||
		created.Title != "CLI instance preflight create" {
		t.Fatalf("decode CLI instance Conversation create: conversation=%#v stdout=%q err=%v", created, output, err)
	}
	requests := recorder.snapshot()[requestStart:]
	if len(requests) != 3 ||
		requests[0].method != http.MethodGet || requests[0].path != clientInstanceSessionViewCandidatePath ||
		requests[1].method != http.MethodGet || requests[1].path != clientInstanceResourceViewCandidatePath ||
		requests[2].method != http.MethodPost || requests[2].path != conversationCollectionPath {
		t.Fatalf("CLI instance Conversation create request order=%#v", requests)
	}
	assertClientInstanceProjectionRequestsHaveNoExecutionOrDeviceEffects(t, requests)
}

func assertCLIInstanceCreateRequiresResourceScope(
	t *testing.T,
	executable, apiURL, accessToken string,
	recorder *conversationHTTPRecorder,
) {
	t.Helper()
	requestStart := len(recorder.snapshot())
	output, stderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "--idempotency-key", "projection-cli-instance-no-device-scope",
		"remote", "sessions", "create", "--instance", "client-cli-001",
		"--title", "Must not create without resource scope",
	)
	if err == nil || output != "" || !strings.Contains(stderr, "HTTP 403") {
		t.Fatalf("resource-scope-less CLI create was not rejected: stdout=%q stderr=%q err=%v", output, stderr, err)
	}
	requests := recorder.snapshot()[requestStart:]
	if len(requests) != 2 ||
		requests[0].method != http.MethodGet || requests[0].path != clientInstanceSessionViewCandidatePath ||
		requests[1].method != http.MethodGet || requests[1].path != clientInstanceResourceViewCandidatePath {
		t.Fatalf("resource-scope-less CLI create escaped the pair authorization boundary: %#v", requests)
	}
	for _, request := range requests {
		if request.method == http.MethodPost && request.path == conversationCollectionPath {
			t.Fatalf("resource-scope-less CLI create sent Conversation POST: %#v", requests)
		}
	}
}
