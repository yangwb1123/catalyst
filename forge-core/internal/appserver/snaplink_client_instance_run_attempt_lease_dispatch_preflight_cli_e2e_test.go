package appserver

// This opt-in acceptance test crosses a real Snaplink JWT through the Rust
// Runtime CLI's client-instance boundary.  The CLI must read the owner-bound
// session/resource pair before posting the inert Run/Attempt/lease preflight;
// a hidden Conversation is rejected before POST and production wiring remains
// closed.

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
)

func TestSnaplinkAuthenticatedClientInstanceRunAttemptLeaseDispatchPreflightCLIProjectionE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_CLI_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_CLI_E2E=1 for Runtime CLI Run/Attempt/lease preflight projection E2E")
	}
	configuredExecutable := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredExecutable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for Runtime CLI Run/Attempt/lease preflight projection E2E")
	}
	executable, err := exec.LookPath(configuredExecutable)
	if err != nil {
		t.Fatalf("FORGE_RUNTIME_BIN must name a built forge-runtime executable available to the test: %v", err)
	}
	executable, err = filepath.Abs(executable)
	if err != nil {
		t.Fatal(err)
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

	owner := deviceplacement.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	request := canonicalRunAttemptLeaseDispatchPreflightRequest(t, owner)
	visibleSession := cliPreflightProjectionSessionView(t, owner, request.ConversationID)
	visibleResource := preflightProjectionResourceView(t, owner, visibleSession.Instances)
	sessionSource := &fixtureClientInstanceSessionViewSource{value: visibleSession}
	resourceSource := &fixtureClientInstanceResourceViewSource{value: visibleResource}
	backend := preflightProjectionConversationBackend(request.ConversationID, request.RunID)
	baseRoutes := newAuthenticatedSessionRoutesWithObservationCandidatesBackend(backend, nil)
	conversationRoutes := newConversationRoutesWithBackend(backend)
	preflightPath := "/api/v1/conversations/" + request.ConversationID + "/runs/" + request.RunID + "/attempt-lease-dispatch-preflight/preview"
	preflightCandidate := newRunAttemptLeaseDispatchPreflightRoutes()
	testRoutes := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch {
		case r.URL.EscapedPath() == clientInstanceSessionViewCandidatePath:
			newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{Enabled: true, Source: sessionSource}).ServeHTTP(w, r)
		case r.URL.EscapedPath() == clientInstanceResourceViewCandidatePath:
			newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{Enabled: true, Source: resourceSource}).ServeHTTP(w, r)
		case r.URL.EscapedPath() == preflightPath:
			preflightCandidate.ServeHTTP(w, r)
		case r.URL.EscapedPath() == conversationCollectionPath || strings.HasPrefix(r.URL.EscapedPath(), conversationCollectionPath+"/"):
			conversationRoutes.ServeHTTP(w, r)
		default:
			baseRoutes.ServeHTTP(w, r)
		}
	})
	recorder := &conversationHTTPRecorder{}
	server := httptest.NewServer(authenticator.Handler(recorder.wrap(testRoutes)))
	t.Cleanup(server.Close)

	inputPath := filepath.Join(t.TempDir(), "run-attempt-lease-dispatch-preflight-request.json")
	requestBytes, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(inputPath, requestBytes, 0o600); err != nil {
		t.Fatal(err)
	}
	visibleStart := len(recorder.snapshot())
	output, stderr, err := runForgeRuntimeCLI(t, executable, server.URL, token, t.TempDir(),
		"--json", "remote", "run-attempt-lease-dispatch-preflight-preview", "--input", inputPath, "--instance", "client-cli-001")
	if err != nil {
		t.Fatalf("authenticated Runtime CLI preflight projection failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var observation deviceplacement.RunAttemptLeaseDispatchPreflightObservation
	if err := json.Unmarshal([]byte(output), &observation); err != nil {
		t.Fatalf("decode Runtime CLI preflight projection: %v stdout=%q", err, output)
	}
	if err := observation.Validate(); err != nil || observation.Owner != owner || observation.ConversationID != request.ConversationID || observation.RunID != request.RunID || observation.SelectedTargetID != nil || observation.Authority != (deviceplacement.RunAttemptLeaseDispatchPreflightAuthority{}) {
		t.Fatalf("Runtime CLI preflight projection=%#v err=%v", observation, err)
	}
	visibleRequests := recorder.snapshot()[visibleStart:]
	assertCLIProjectionPairBeforePreflight(t, visibleRequests, preflightPath)

	hiddenSession := cliPreflightProjectionSessionView(t, owner, "conversation-hidden")
	hiddenResource := preflightProjectionResourceView(t, owner, hiddenSession.Instances)
	sessionSource.value = hiddenSession
	resourceSource.value = hiddenResource
	hiddenStart := len(recorder.snapshot())
	_, hiddenStderr, hiddenErr := runForgeRuntimeCLI(t, executable, server.URL, token, t.TempDir(),
		"--json", "remote", "run-attempt-lease-dispatch-preflight-preview", "--input", inputPath, "--instance", "client-cli-001")
	if hiddenErr == nil || !strings.Contains(hiddenStderr, "no Run/Attempt/lease preflight request was sent") {
		t.Fatalf("hidden Runtime CLI preflight was not rejected before POST: stderr=%q err=%v", hiddenStderr, hiddenErr)
	}
	hiddenRequests := recorder.snapshot()[hiddenStart:]
	for _, observed := range hiddenRequests {
		if observed.method == http.MethodPost && observed.path == preflightPath {
			t.Fatalf("hidden Runtime CLI preflight escaped the instance guard: %#v", hiddenRequests)
		}
	}
	if sessionSource.calls < 2 || resourceSource.calls < 2 {
		t.Fatalf("hidden Runtime CLI did not re-read session/resource pair: session=%d resource=%d", sessionSource.calls, resourceSource.calls)
	}

	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	for _, testCase := range []struct {
		method string
		path   string
		body   string
	}{
		{method: http.MethodGet, path: clientInstanceSessionViewCandidatePath},
		{method: http.MethodGet, path: clientInstanceResourceViewCandidatePath},
		{method: http.MethodPost, path: preflightPath, body: string(requestBytes)},
	} {
		contentType := ""
		if testCase.method == http.MethodPost {
			contentType = "application/json"
		}
		response := requestConversationAPIToken(t, production, token, testCase.method, testCase.path, contentType, "", testCase.body)
		if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
			t.Fatalf("default production %s %s status=%d body=%q", testCase.method, testCase.path, response.Code, response.Body.String())
		}
	}
}

func cliPreflightProjectionSessionView(t *testing.T, owner deviceplacement.Owner, conversationID string) deviceplacement.ClientInstanceSessionViewObservation {
	t.Helper()
	view, err := deviceplacement.ObserveClientInstanceSessionView(deviceplacement.ClientInstanceSessionViewRequest{
		Owner: owner,
		Instances: []deviceplacement.ClientInstanceSessionViewInstance{
			{InstanceID: "client-cli-001", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{conversationID}, ObservedAtMS: 200_500, Status: "active"},
		},
	})
	if err != nil {
		t.Fatalf("observe CLI preflight projection session view: %v", err)
	}
	return view
}

func assertCLIProjectionPairBeforePreflight(t *testing.T, requests []recordedConversationRequest, preflightPath string) {
	t.Helper()
	postIndex := -1
	sessionIndex, resourceIndex := -1, -1
	for index, request := range requests {
		if request.method == http.MethodGet && request.path == clientInstanceSessionViewCandidatePath && sessionIndex < 0 {
			sessionIndex = index
		}
		if request.method == http.MethodGet && request.path == clientInstanceResourceViewCandidatePath && resourceIndex < 0 {
			resourceIndex = index
		}
		if request.method == http.MethodPost && request.path == preflightPath {
			postIndex = index
		}
	}
	if sessionIndex < 0 || resourceIndex < 0 || postIndex < 0 || sessionIndex > postIndex || resourceIndex > postIndex {
		t.Fatalf("Runtime CLI preflight POST did not follow session/resource pair: %#v", requests)
	}
}
