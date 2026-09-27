//go:build linux && !android

package appserver

import (
	"context"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// TestSnaplinkAuthenticatedRuntimeClientInstanceChangesStreamE2EWhenConfigured
// drives the instance-filtered TUI change feed through a real JWT. The feed is
// an owner-scoped read projection: hidden rows advance the cursor but never
// become visible session state or private Prompt/Run reads.
func TestSnaplinkAuthenticatedRuntimeClientInstanceChangesStreamE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_RUNTIME_CLIENT_INSTANCE_CHANGES_STREAM_E2E") != "1" {
		t.Skip("set FORGE_RUNTIME_CLIENT_INSTANCE_CHANGES_STREAM_E2E=1 for Runtime TUI instance-filtered changes E2E")
	}
	executable := configuredRuntimeExecutable(t, "FORGE_RUNTIME_BIN", "Runtime TUI instance-filtered changes E2E")
	identity, authenticator := newMultiPrincipalConversationTestIdentity(t)
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	sessionView, resourceView := runtimeClientInstanceChangesViews(t, owner)
	backend := runtimeClientInstanceChangesBackend("instance-stream-visible", "instance-stream-hidden")
	harness := newRuntimeClientInstanceChangesHarness(t, authenticator, sessionView, resourceView, backend)
	token := tokenForIndependentClient(identity, "forge:conversations:read "+deviceInventoryReadCandidateScope, "instance-stream-tui")
	output := runForgeRuntimeClientInstanceChangesStreamTUI(t, executable, harness.serverURL, token)
	assertRuntimeClientInstanceChangesOutput(t, output, "instance-stream-visible", "instance-stream-hidden")
	assertRuntimeClientInstanceChangesHeaders(t, harness, token)
	assertRuntimeClientInstanceChangesRequests(t, harness.recorder.snapshot())
}

type runtimeClientInstanceChangesHarness struct {
	serverURL           string
	recorder            *conversationHTTPRecorder
	headerMu            *sync.Mutex
	acceptHeader        *string
	authorizationHeader *string
}

func configuredRuntimeExecutable(t *testing.T, environment, description string) string {
	t.Helper()
	configured := os.Getenv(environment)
	if configured == "" {
		t.Skip("set " + environment + " for " + description)
	}
	executable, err := exec.LookPath(configured)
	if err != nil {
		t.Fatalf("%s must name a built forge-runtime executable available to the test: %v", environment, err)
	}
	executable, err = filepath.Abs(executable)
	if err != nil {
		t.Fatalf("resolve forge-runtime executable path: %v", err)
	}
	return executable
}

func runtimeClientInstanceChangesViews(t *testing.T, owner deviceplacement.Owner) (deviceplacement.ClientInstanceSessionViewObservation, deviceplacement.ClientInstanceResourceViewObservation) {
	t.Helper()
	sessionView, err := deviceplacement.ObserveClientInstanceSessionView(deviceplacement.ClientInstanceSessionViewRequest{
		Owner: owner,
		Instances: []deviceplacement.ClientInstanceSessionViewInstance{
			{InstanceID: "client-web-001", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{"instance-stream-visible"}, ObservedAtMS: 200_500, Status: "active"},
			{InstanceID: "client-tui-001", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{"instance-stream-hidden"}, ObservedAtMS: 200_500, Status: "active"},
		},
	})
	if err != nil {
		t.Fatalf("build client-instance session view: %v", err)
	}
	resourceView := fixtureClientInstanceResourceView(owner)
	resourceView.Instances = append([]deviceplacement.ClientInstanceSessionViewInstance(nil), sessionView.Instances...)
	if err := resourceView.Validate(); err != nil {
		t.Fatalf("build client-instance resource view: %v", err)
	}
	return sessionView, resourceView
}

func runtimeClientInstanceChangesBackend(visibleID, hiddenID string) *fakeConversationBackend {
	return &fakeConversationBackend{
		listPage: model.OwnedConversationPage{Conversations: []model.OwnedConversationEntry{
			{Conversation: model.Conversation{ID: hiddenID, Scope: model.ConversationScope{Kind: "global"}, Title: "Hidden stream", CreatedAtMS: 2, UpdatedAtMS: 2}, AggregateVersion: 1},
			{Conversation: model.Conversation{ID: visibleID, Scope: model.ConversationScope{Kind: "global"}, Title: "Visible stream", CreatedAtMS: 1, UpdatedAtMS: 1}, AggregateVersion: 1},
		}},
		changePage: model.OwnedConversationChangePage{AfterCursor: 0, ScannedThroughCursor: 2, Changes: []model.Change{
			{Cursor: 1, SchemaVersion: 1, ConversationID: visibleID, EntityID: visibleID, AggregateVersion: 2, Kind: "prompt_appended", CreatedAtMS: 3},
			{Cursor: 2, SchemaVersion: 1, ConversationID: hiddenID, EntityID: hiddenID, AggregateVersion: 2, Kind: "prompt_appended", CreatedAtMS: 4},
		}},
	}
}

func newRuntimeClientInstanceChangesHarness(t *testing.T, authenticator interface {
	Handler(http.Handler) http.Handler
}, sessionView deviceplacement.ClientInstanceSessionViewObservation, resourceView deviceplacement.ClientInstanceResourceViewObservation, backend *fakeConversationBackend) *runtimeClientInstanceChangesHarness {
	t.Helper()
	sessions := newAuthenticatedSessionRoutesWithObservationCandidatesBackend(backend, nil)
	testRoutes := http.NewServeMux()
	testRoutes.Handle(clientInstanceSessionViewCandidatePath, newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{Enabled: true, Source: staticClientInstanceSessionViewSource{value: sessionView}}))
	testRoutes.Handle(clientInstanceResourceViewCandidatePath, newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{Enabled: true, Source: &fixtureClientInstanceResourceViewSource{value: resourceView}}))
	testRoutes.Handle("/", sessions)
	headerMu := &sync.Mutex{}
	acceptHeader, authorizationHeader := "", ""
	recorder := &conversationHTTPRecorder{}
	server := httptest.NewServer(authenticator.Handler(recorder.wrap(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		if request.URL.EscapedPath() == conversationChangesStreamPath {
			headerMu.Lock()
			acceptHeader, authorizationHeader = request.Header.Get("Accept"), request.Header.Get("Authorization")
			headerMu.Unlock()
		}
		testRoutes.ServeHTTP(writer, request)
	}))))
	t.Cleanup(server.Close)
	return &runtimeClientInstanceChangesHarness{serverURL: server.URL, recorder: recorder, headerMu: headerMu, acceptHeader: &acceptHeader, authorizationHeader: &authorizationHeader}
}

func assertRuntimeClientInstanceChangesOutput(t *testing.T, output, visibleID, hiddenID string) {
	t.Helper()
	if !strings.Contains(output, `Client-instance filter set to "client-web-001"`) || !strings.Contains(output, "Changes stream start_cursor=0 scanned_through_cursor=2 wait_ms=0 changes=1") || !strings.Contains(output, `conversation="`+visibleID+`"`) || strings.Contains(output, `entity="`+hiddenID+`"`) {
		t.Fatalf("instance-filtered TUI change stream output=%q", output)
	}
}

func assertRuntimeClientInstanceChangesHeaders(t *testing.T, harness *runtimeClientInstanceChangesHarness, token string) {
	t.Helper()
	harness.headerMu.Lock()
	acceptHeader, authorizationHeader := *harness.acceptHeader, *harness.authorizationHeader
	harness.headerMu.Unlock()
	if acceptHeader != "text/event-stream" || authorizationHeader != "Bearer "+token {
		t.Fatalf("TUI change stream headers accept=%q authorization=%q", acceptHeader, authorizationHeader)
	}
}

func assertRuntimeClientInstanceChangesRequests(t *testing.T, requests []recordedConversationRequest) {
	t.Helper()
	want := []recordedConversationRequest{
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: clientInstanceSessionViewCandidatePath},
		{method: http.MethodGet, path: clientInstanceResourceViewCandidatePath},
		{method: http.MethodGet, path: clientInstanceSessionViewCandidatePath},
		{method: http.MethodGet, path: clientInstanceResourceViewCandidatePath},
		{method: http.MethodGet, path: conversationChangesStreamPath, query: "after_cursor=0&limit=128&wait_ms=0"},
	}
	if len(requests) != len(want) {
		t.Fatalf("instance-filtered TUI change stream requests=%#v want=%#v", requests, want)
	}
	for index, request := range requests {
		if request != want[index] {
			t.Fatalf("instance-filtered TUI change stream request[%d]=%#v want=%#v", index, request, want[index])
		}
	}
	assertClientInstanceProjectionRequestsHaveNoExecutionOrDeviceEffects(t, requests)
}

func runForgeRuntimeClientInstanceChangesStreamTUI(t *testing.T, executable, apiURL, token string) string {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("instance-filtered TUI change stream E2E requires script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript, "-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, token, home)
	command.Stdin = strings.NewReader("client-instances show-converged\ninstance client-web-001\nchanges stream --after-cursor 0 --wait-ms 0 --instance client-web-001\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout, command.Stderr = &stdoutBuffer, &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("instance-filtered TUI change stream failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("instance-filtered TUI change stream output exceeded the size limit")
	}
	return stdoutBuffer.String()
}
