//go:build linux && !android

package appserver

import (
	"encoding/json"
	"net/http"
	"os"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceplacement"
)

// TestSnaplinkAuthenticatedRuntimeClientInstanceChangesStreamCLIE2EWhenConfigured
// drives the instance-filtered CLI change feed through a real JWT. The CLI
// refreshes the owner-bound pair before the owner-wide SSE read; hidden rows
// advance the scanned cursor while remaining outside the selected projection.
func TestSnaplinkAuthenticatedRuntimeClientInstanceChangesStreamCLIE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_RUNTIME_CLIENT_INSTANCE_CHANGES_CLI_E2E") != "1" {
		t.Skip("set FORGE_RUNTIME_CLIENT_INSTANCE_CHANGES_CLI_E2E=1 for Runtime CLI instance-filtered changes E2E")
	}
	executable := configuredRuntimeExecutable(t, "FORGE_RUNTIME_BIN", "Runtime CLI instance-filtered changes E2E")
	identity, authenticator := newMultiPrincipalConversationTestIdentity(t)
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	sessionView, resourceView := runtimeClientInstanceChangesViews(t, owner)
	backend := runtimeClientInstanceChangesBackend("instance-stream-visible", "instance-stream-hidden")
	harness := newRuntimeClientInstanceChangesHarness(t, authenticator, sessionView, resourceView, backend)
	token := tokenForIndependentClient(identity, "forge:conversations:read "+deviceInventoryReadCandidateScope, "instance-stream-cli")

	output, stderr, err := runForgeRuntimeCLI(
		t, executable, harness.serverURL, token, t.TempDir(),
		"--json", "remote", "changes", "stream", "--after-cursor", "0", "--wait-ms", "0",
		"--instance", "client-web-001",
	)
	if err != nil {
		t.Fatalf("instance-filtered CLI change stream failed: stdout=%q stderr=%q err=%v", output, stderr, err)
	}
	assertRuntimeCLIInstanceChangesOutput(t, output, "instance-stream-visible", "instance-stream-hidden")
	assertRuntimeClientInstanceChangesHeaders(t, harness, token)
	assertRuntimeCLIInstanceChangesRequests(t, harness.recorder.snapshot())
}

type runtimeCLIInstanceChangesPage struct {
	StartCursor          uint64                     `json:"start_cursor"`
	ScannedThroughCursor uint64                     `json:"scanned_through_cursor"`
	TimedOut             bool                       `json:"timed_out"`
	Changes              []runtimeCLIInstanceChange `json:"changes"`
}

type runtimeCLIInstanceChange struct {
	ConversationID string `json:"conversation_id"`
}

func assertRuntimeCLIInstanceChangesOutput(t *testing.T, output, visibleID, hiddenID string) {
	t.Helper()
	var page runtimeCLIInstanceChangesPage
	if err := json.Unmarshal([]byte(output), &page); err != nil {
		t.Fatalf("decode instance-filtered CLI change stream: %v stdout=%q", err, output)
	}
	if page.StartCursor != 0 || page.ScannedThroughCursor != 2 || page.TimedOut || len(page.Changes) != 1 || page.Changes[0].ConversationID != visibleID {
		t.Fatalf("instance-filtered CLI change stream page=%#v stdout=%q", page, output)
	}
	if strings.Contains(output, hiddenID) {
		t.Fatalf("hidden instance change escaped CLI projection: stdout=%q", output)
	}
}

func assertRuntimeCLIInstanceChangesRequests(t *testing.T, requests []recordedConversationRequest) {
	t.Helper()
	want := []recordedConversationRequest{
		{method: http.MethodGet, path: clientInstanceSessionViewCandidatePath},
		{method: http.MethodGet, path: clientInstanceResourceViewCandidatePath},
		{method: http.MethodGet, path: conversationChangesStreamPath, query: "after_cursor=0&limit=128&wait_ms=0"},
	}
	if len(requests) != len(want) {
		t.Fatalf("instance-filtered CLI change stream requests=%#v want=%#v", requests, want)
	}
	for index, request := range requests {
		if request != want[index] {
			t.Fatalf("instance-filtered CLI change stream request[%d]=%#v want=%#v", index, request, want[index])
		}
	}
	assertClientInstanceProjectionRequestsHaveNoExecutionOrDeviceEffects(t, requests)
}
