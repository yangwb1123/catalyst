//go:build linux && !android

package appserver

// This opt-in acceptance test crosses a real Snaplink JWT through the
// Runtime TUI's selected client-instance boundary. The TUI must read the
// owner-bound inventory/resource and session/resource observations before
// posting the inert Run/Attempt/lease preflight candidate. A hidden selected
// instance fails closed before POST and the ordinary production constructor
// remains closed.

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

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
)

func TestSnaplinkAuthenticatedClientInstanceRunAttemptLeaseDispatchPreflightTUIConvergenceE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_TUI_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_TUI_E2E=1 for Runtime TUI Run/Attempt/lease preflight convergence E2E")
	}
	configuredExecutable := os.Getenv("FORGE_RUNTIME_BIN")
	if configuredExecutable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for Runtime TUI Run/Attempt/lease preflight convergence E2E")
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

	owner := deviceplacement.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	request := canonicalRunAttemptLeaseDispatchPreflightRequest(t, owner)
	conversationID, runID := request.ConversationID, request.RunID
	visibleSession := preflightTUISessionView(t, owner, conversationID)
	visibleResource := preflightProjectionResourceView(t, owner, visibleSession.Instances)
	visibleInventory := preflightProjectionInventory(t, owner, visibleResource)
	sessionSource := &fixtureClientInstanceSessionViewSource{value: visibleSession}
	resourceSource := &fixtureClientInstanceResourceViewSource{value: visibleResource}
	inventorySource := &fixtureDeviceInventoryReadV2Source{value: visibleInventory}
	backend := preflightProjectionConversationBackend(conversationID, runID)

	baseRoutes := newAuthenticatedSessionRoutesWithObservationCandidatesBackend(backend, nil)
	preflightPath := "/api/v1/conversations/" + conversationID + "/runs/" + runID + "/attempt-lease-dispatch-preflight/preview"
	preflightCandidate := newRunAttemptLeaseDispatchPreflightRoutes()
	recorder := &conversationHTTPRecorder{}
	testRoutes := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.EscapedPath() {
		case clientInstanceSessionViewCandidatePath:
			newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
				Enabled: true, Source: sessionSource,
			}).ServeHTTP(w, r)
		case clientInstanceResourceViewCandidatePath:
			newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
				Enabled: true, Source: resourceSource,
			}).ServeHTTP(w, r)
		case deviceInventoryReadCandidateV2Path:
			newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
				Enabled: true, Source: inventorySource,
			}).ServeHTTP(w, r)
		case preflightPath:
			preflightCandidate.ServeHTTP(w, r)
		default:
			baseRoutes.ServeHTTP(w, r)
		}
	})
	server := httptest.NewServer(authenticator.Handler(recorder.wrap(testRoutes)))
	t.Cleanup(server.Close)

	requestBytes, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	inputPath := filepath.Join(t.TempDir(), "run-attempt-lease-dispatch-preflight-tui-request.json")
	if err := os.WriteFile(inputPath, requestBytes, 0o600); err != nil {
		t.Fatal(err)
	}

	visibleStart := len(recorder.snapshot())
	visibleOutput := runForgeRuntimeClientInstancePreflightTUI(
		t, executable, server.URL, token, inputPath, "client-tui-001",
	)
	for _, want := range []string{
		"Run/Attempt/lease dispatch preflight [forge.run-attempt-lease-dispatch-preflight/v1]",
		"conversation=conversation-001 run=run-001",
		"dispatch_performed=false",
	} {
		if !strings.Contains(visibleOutput, want) {
			t.Fatalf("visible TUI output omitted %q: %q", want, visibleOutput)
		}
	}
	assertTUIPreflightRequestOrder(t, recorder.snapshot()[visibleStart:], preflightPath, true)

	// The selected TUI instance now hides the requested Conversation. The TUI
	// must refresh the same owner-bound pair and inventory image, then stop
	// locally without sending a candidate request.
	hiddenSession := preflightTUISessionView(t, owner, "conversation-hidden")
	hiddenResource := preflightProjectionResourceView(t, owner, hiddenSession.Instances)
	hiddenInventory := preflightProjectionInventory(t, owner, hiddenResource)
	sessionSource.value = hiddenSession
	resourceSource.value = hiddenResource
	inventorySource.value = hiddenInventory
	hiddenStart := len(recorder.snapshot())
	hiddenOutput := runForgeRuntimeClientInstancePreflightTUI(
		t, executable, server.URL, token, inputPath, "client-tui-001",
	)
	if !strings.Contains(hiddenOutput, "requires the selected session to match conversation conversation-001") &&
		!strings.Contains(hiddenOutput, "no Run/Attempt/lease preflight request was sent") {
		t.Fatalf("hidden TUI projection did not fail closed: %q", hiddenOutput)
	}
	assertTUIPreflightRequestOrder(t, recorder.snapshot()[hiddenStart:], preflightPath, false)
	if sessionSource.calls < 2 || resourceSource.calls < 2 || inventorySource.calls < 2 {
		t.Fatalf("TUI did not re-read owner projections for visible and hidden runs: session=%d resource=%d inventory=%d", sessionSource.calls, resourceSource.calls, inventorySource.calls)
	}

	// The ordinary production constructor remains closed for this candidate,
	// even when presented with the full-scope real Snaplink JWT.
	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	productionRequest := httptest.NewRequest(http.MethodPost, preflightPath, strings.NewReader(string(requestBytes)))
	productionRequest.Header.Set("Authorization", "Bearer "+token)
	productionRequest.Header.Set("Content-Type", "application/json")
	productionResponse := httptest.NewRecorder()
	production.ServeHTTP(productionResponse, productionRequest)
	if productionResponse.Code != http.StatusNotFound || productionResponse.Body.String() != string(notFoundBody) {
		t.Fatalf("default production TUI preflight status=%d body=%q", productionResponse.Code, productionResponse.Body.String())
	}
}

func preflightTUISessionView(t *testing.T, owner deviceplacement.Owner, conversationID string) deviceplacement.ClientInstanceSessionViewObservation {
	t.Helper()
	view, err := deviceplacement.ObserveClientInstanceSessionView(deviceplacement.ClientInstanceSessionViewRequest{
		Owner: owner,
		Instances: []deviceplacement.ClientInstanceSessionViewInstance{
			{InstanceID: "client-tui-001", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{conversationID}, ObservedAtMS: 200_500, Status: "active"},
			{InstanceID: "client-web-001", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{"conversation-002"}, ObservedAtMS: 200_500, Status: "idle"},
		},
	})
	if err != nil {
		t.Fatalf("observe TUI preflight session view: %v", err)
	}
	return view
}

func runForgeRuntimeClientInstancePreflightTUI(
	t *testing.T, executable, apiURL, accessToken, inputPath, instanceID string,
) string {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("client-instance preflight TUI E2E requires script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader(
		"inventory read-v2\n" +
			"client-instances show-converged\n" +
			"instance " + instanceID + "\n" +
			"run-attempt-lease-dispatch-preflight-remote-preview --input " + inputPath + "\n" +
			"quit\n",
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated client-instance preflight TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("authenticated client-instance preflight TUI output exceeded the size limit")
	}
	return stdoutBuffer.String()
}

func assertTUIPreflightRequestOrder(t *testing.T, requests []recordedConversationRequest, preflightPath string, expectPreflight bool) {
	t.Helper()
	want := []string{
		http.MethodGet + " " + conversationCollectionPath,
		http.MethodGet + " " + deviceInventoryReadCandidateV2Path,
		http.MethodGet + " " + clientInstanceSessionViewCandidatePath,
		http.MethodGet + " " + clientInstanceResourceViewCandidatePath,
	}
	if expectPreflight {
		want = append(want, http.MethodPost+" "+preflightPath)
	}
	got := make([]string, 0, len(requests))
	for _, request := range requests {
		got = append(got, request.method+" "+request.path)
	}
	if len(got) != len(want) {
		t.Fatalf("client-instance preflight TUI request order=%#v, want %#v", got, want)
	}
	for index := range want {
		if got[index] != want[index] {
			t.Fatalf("client-instance preflight TUI request[%d]=%q, want %q (pair/inventory must precede preflight POST); full=%#v", index, got[index], want[index], got)
		}
	}
}
