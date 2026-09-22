package appserver

import (
	"context"
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

// runRunTimelineResumeTUI proves that the Rust TUI can persist a private,
// owner-bound Run timeline checkpoint and resume it in a separate process.
// The first process reads from sequence zero; the second process reads only
// after the persisted positive cursor and therefore renders no duplicate
// metadata markers.
func runRunTimelineResumeTUI(
	t *testing.T,
	executable, apiURL, issuer, subject, tenant, accessToken, conversationID, runID string,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("the Run timeline resume TUI integration requires a PTY launcher named script: %v", err)
	}
	userHome, err := os.UserHomeDir()
	if err != nil {
		t.Fatalf("locate private user home for Run timeline checkpoint: %v", err)
	}
	home, err := os.MkdirTemp(userHome, ".forge-run-timeline-resume-")
	if err != nil {
		t.Fatalf("create private Run timeline checkpoint home: %v", err)
	}
	t.Cleanup(func() { _ = os.RemoveAll(home) })
	writeForgeSavedCredential(t, home, issuer, subject, tenant, accessToken)

	run := func(input string) string {
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		defer cancel()
		command := exec.CommandContext(ctx, ptyScript,
			"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
		command.Dir = home
		command.Env = forgeRuntimeSavedCredentialEnvironment(apiURL, issuer, subject, tenant, home)
		command.Stdin = strings.NewReader(input)
		var stdoutBuffer, stderrBuffer boundedCLIOutput
		command.Stdout = &stdoutBuffer
		command.Stderr = &stderrBuffer
		if err := command.Run(); err != nil {
			t.Fatalf("Run timeline resume TUI failed: stdout=%q stderr=%q err=%v",
				stdoutBuffer.String(), stderrBuffer.String(), err)
		}
		if stdoutBuffer.exceeded || stderrBuffer.exceeded {
			t.Fatal("Run timeline resume TUI output exceeded the size limit")
		}
		return stdoutBuffer.String()
	}

	first := run(fmt.Sprintf("open %s\nruns\ntimeline %q --resume\nquit\n", conversationID, runID))
	for _, want := range []string{"Event seq=1", "type=\"run_started\"", "type=\"run_finished\""} {
		if !strings.Contains(first, want) {
			t.Fatalf("initial resumed TUI output omitted %q: %q", want, first)
		}
	}

	second := run(fmt.Sprintf("open %s\nruns\ntimeline %q --resume\nquit\n", conversationID, runID))
	if !strings.Contains(second, "No Run timeline events on this page.") {
		t.Fatalf("second resumed TUI output omitted empty-page marker: %q", second)
	}
	if strings.Contains(second, "Event seq=") || strings.Contains(second, "run_started") || strings.Contains(second, "run_finished") {
		t.Fatalf("second resumed TUI replayed timeline markers: %q", second)
	}
}

func forgeRuntimeSavedCredentialEnvironment(apiURL, issuer, subject, tenant, home string) []string {
	env := forgeRuntimeCLIEnvironment(apiURL, "", home)
	filtered := env[:0]
	for _, value := range env {
		if !strings.HasPrefix(value, "FORGE_ACCESS_TOKEN=") {
			filtered = append(filtered, value)
		}
	}
	return append(filtered,
		"SNAPLINK_ISSUER_URL="+issuer,
		"SNAPLINK_CLIENT_ID=forge-cli",
		"SNAPLINK_SUBJECT="+subject,
		"SNAPLINK_TENANT_ID="+tenant,
	)
}

func writeForgeSavedCredential(t *testing.T, home, issuer, subject, tenant, accessToken string) {
	t.Helper()
	credentialDirectory := filepath.Join(home, "config", "forge-runtime", "credentials")
	if err := os.MkdirAll(credentialDirectory, 0o700); err != nil {
		t.Fatal(err)
	}
	credential := map[string]any{
		"issuer":          issuer,
		"client_id":       "forge-cli",
		"subject":         subject,
		"tenant_id":       tenant,
		"access_token":    accessToken,
		"expires_at_unix": uint64(time.Now().Add(time.Hour).Unix()),
	}
	encoded, err := json.Marshal(credential)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.New()
	for _, field := range []string{issuer, "forge-cli", tenant, subject} {
		var length [8]byte
		binary.BigEndian.PutUint64(length[:], uint64(len(field)))
		_, _ = digest.Write(length[:])
		_, _ = digest.Write([]byte(field))
	}
	path := filepath.Join(credentialDirectory, hex.EncodeToString(digest.Sum(nil))+".json")
	if err := os.WriteFile(path, encoded, 0o600); err != nil {
		t.Fatal(err)
	}
	if info, err := os.Stat(path); err != nil || info.Mode().Perm() != 0o600 {
		t.Fatalf("saved Forge credential has unsafe mode: info=%v err=%v", info, err)
	}
}

func assertRunTimelineResumeTUIRequests(t *testing.T, requests []recordedConversationRequest, conversationID, runID string) {
	t.Helper()
	base := conversationCollectionPath + "/" + conversationID
	wantFixed := []recordedConversationRequest{
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: base + "/prompts", query: "limit=128"},
		{method: http.MethodGet, path: base + "/runs", query: "limit=25"},
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: base + "/prompts", query: "limit=128"},
		{method: http.MethodGet, path: base + "/runs", query: "limit=25"},
	}
	if len(requests) != 8 {
		t.Fatalf("Run timeline resume TUI issued %d requests; want 8 got %#v", len(requests), requests)
	}
	for _, index := range []int{0, 1, 2, 4, 5, 6} {
		fixedIndex := index
		if index > 3 {
			fixedIndex -= 1
		}
		if requests[index] != wantFixed[fixedIndex] {
			t.Fatalf("Run timeline resume TUI request[%d]=%#v want=%#v", index, requests[index], wantFixed[fixedIndex])
		}
	}
	timelinePath := base + "/runs/" + runID + "/timeline"
	if requests[3].method != http.MethodGet || requests[3].path != timelinePath {
		t.Fatalf("Run timeline resume initial request=%#v", requests[3])
	}
	initial, ok := parseCursorQuery(requests[3].query, "after_sequence")
	if !ok || initial != 0 {
		t.Fatalf("Run timeline resume initial cursor=%#v", requests[3])
	}
	if requests[7].method != http.MethodGet || requests[7].path != timelinePath {
		t.Fatalf("Run timeline resume follow-up request=%#v", requests[7])
	}
	resumed, ok := parseCursorQuery(requests[7].query, "after_sequence")
	if !ok || resumed == 0 {
		t.Fatalf("Run timeline resume follow-up cursor=%#v", requests[7])
	}
}
