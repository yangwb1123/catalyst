package appserver

// This opt-in test crosses the real Snaplink JWT boundary for the injected
// Runner dispatch-plan preview candidate. The candidate compares only caller
// supplied declarations; it never selects, reserves, or dispatches a target.

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
)

func TestSnaplinkAuthenticatedRunnerDispatchPlanPreviewWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_RUNNER_DISPATCH_PLAN_PREVIEW_E2E") != "1" {
		t.Skip("set FORGE_RUNNER_DISPATCH_PLAN_PREVIEW_E2E=1 for the Runner dispatch-plan preview E2E")
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
	owner := deviceplacement.Owner{
		Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant,
	}
	preflight := runAttemptLeaseDispatchPreflightRequest(owner, "conversation-001", "run-001")
	body, err := json.Marshal(preflight.DispatchPlan)
	if err != nil {
		t.Fatal(err)
	}
	requestEnvelope, err := json.Marshal(preflight)
	if err != nil {
		t.Fatal(err)
	}
	candidate := newConversationRoutesWithInertExecutionAPI(nil, nil)
	handler := authenticator.Handler(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// The TUI performs its normal owner-scoped session refresh before the
		// explicit preview command. Keep those reads on the same JWT mux.
		switch {
		case r.Method == http.MethodGet && r.URL.Path == conversationCollectionPath:
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"conversations":[{"conversation":{"id":"conversation-001","scope":{"kind":"global"},"title":"Dispatch-plan preview fixture","created_at_ms":1,"updated_at_ms":1},"aggregate_version":1}],"next_after_id":null,"has_more":false}`))
		case r.Method == http.MethodGet && r.URL.Path == conversationCollectionPath+"/conversation-001/prompts":
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"conversation_id":"conversation-001","prompts":[{"id":"prompt-001","conversation_id":"conversation-001","role":"user","content":"Dispatch-plan fixture","created_at_ms":100}],"next_cursor":null,"has_more":false}`))
		case r.Method == http.MethodGet && r.URL.Path == conversationChangesPath:
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"after_cursor":0,"scanned_through_cursor":0,"has_more":false,"changes":[]}`))
		default:
			candidate.ServeHTTP(w, r)
		}
	}))
	server := httptest.NewServer(handler)
	t.Cleanup(server.Close)
	path := "/api/v1/conversations/conversation-001/runs/run-001/runner-dispatch-plan-preview"
	response := snaplinkConversationRequest(t, server.Client(), server.URL, token,
		http.MethodPost, path, "", string(body))
	if response.StatusCode != http.StatusOK {
		t.Fatalf("Snaplink Runner dispatch-plan preview status=%d body=%q", response.StatusCode,
			readConversationClientBody(t, response))
	}
	responseBody := readConversationClientBody(t, response)
	var observation deviceplacement.RunnerDispatchPlanPreviewObservation
	if err := json.Unmarshal([]byte(responseBody), &observation); err != nil {
		t.Fatalf("decode Snaplink Runner dispatch-plan preview: %v body=%q", err, responseBody)
	}
	expected, err := deviceplacement.ObserveRunnerDispatchPlanPreview(preflight.DispatchPlan)
	if err != nil {
		t.Fatal(err)
	}
	if err := observation.Validate(); err != nil || !reflect.DeepEqual(observation, expected) ||
		observation.SelectedTargetID != nil || observation.Authority != (deviceplacement.RunnerDispatchPlanPreviewAuthority{}) {
		t.Fatalf("Snaplink Runner dispatch-plan preview=%#v expected=%#v err=%v", observation, expected, err)
	}
	if strings.Contains(responseBody, preflight.DispatchPlan.Lease.FencingToken) {
		t.Fatalf("Snaplink dispatch-plan preview leaked lease fencing material: %q", responseBody)
	}
	requestPath := filepath.Join(t.TempDir(), "runner-dispatch-plan-preview-request.json")
	if err := os.WriteFile(requestPath, requestEnvelope, 0o600); err != nil {
		t.Fatal(err)
	}
	if executable := os.Getenv("FORGE_RUNTIME_BIN"); executable != "" {
		runForgeRuntimeRunnerDispatchPlanPreviewRemoteCLI(t, executable, server.URL, token, requestPath, preflight.DispatchPlan)
		runForgeRuntimeRunnerDispatchPlanPreviewRemoteTUI(t, executable, server.URL, token, requestPath, owner)
	}

	// The normal Coordinator constructor must stay closed for the same path.
	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	productionRequest := httptest.NewRequest(http.MethodPost, server.URL+path, strings.NewReader(string(body)))
	productionRequest.Header.Set("Authorization", "Bearer "+token)
	productionRequest.Header.Set("Content-Type", "application/json")
	productionResponse := httptest.NewRecorder()
	production.ServeHTTP(productionResponse, productionRequest)
	if productionResponse.Code != http.StatusNotFound || productionResponse.Body.String() != string(notFoundBody) {
		t.Fatalf("production Runner dispatch-plan preview status=%d body=%q", productionResponse.Code, productionResponse.Body.String())
	}
}

func runForgeRuntimeRunnerDispatchPlanPreviewRemoteCLI(
	t *testing.T, executable, apiURL, accessToken, inputPath string,
	request deviceplacement.RunnerDispatchPlanPreviewRequest,
) {
	t.Helper()
	output, stderr, err := runForgeRuntimeCLI(t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "remote", "runner-dispatch-plan-preview", "--input", inputPath)
	if err != nil {
		t.Fatalf("authenticated Rust Runner dispatch-plan preview CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var observation deviceplacement.RunnerDispatchPlanPreviewObservation
	if err := json.Unmarshal([]byte(output), &observation); err != nil {
		t.Fatalf("decode authenticated Rust Runner dispatch-plan preview CLI: %v stdout=%q", err, output)
	}
	expected, err := deviceplacement.ObserveRunnerDispatchPlanPreview(request)
	if err != nil {
		t.Fatal(err)
	}
	if err := observation.Validate(); err != nil || !reflect.DeepEqual(observation, expected) || observation.SelectedTargetID != nil ||
		observation.Authority != (deviceplacement.RunnerDispatchPlanPreviewAuthority{}) {
		t.Fatalf("authenticated Rust Runner dispatch-plan preview CLI=%#v expected=%#v err=%v", observation, expected, err)
	}
}

func runForgeRuntimeRunnerDispatchPlanPreviewRemoteTUI(
	t *testing.T, executable, apiURL, accessToken, inputPath string, owner deviceplacement.Owner,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("authenticated Rust Runner dispatch-plan preview TUI requires script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader(
		"runner-dispatch-plan-remote-preview --input " + inputPath + "\nquit\n",
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated Rust Runner dispatch-plan preview TUI failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("authenticated Rust Runner dispatch-plan preview TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"Runner dispatch-plan preview [forge.runner-dispatch-plan-preview/v1]",
		"owner=" + owner.Issuer + "/" + owner.Subject + "/" + owner.TenantID + " conversation=conversation-001 run=run-001",
		"selected_target=none",
		"dispatch_performed=false",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("authenticated Rust Runner dispatch-plan preview TUI omitted %q: %q", want, output)
		}
	}
	if strings.Contains(output, "fence-001") || strings.Contains(output, "/api/v1/devices") {
		t.Fatalf("authenticated Rust Runner dispatch-plan preview TUI leaked private data or device request: %q", output)
	}
}
