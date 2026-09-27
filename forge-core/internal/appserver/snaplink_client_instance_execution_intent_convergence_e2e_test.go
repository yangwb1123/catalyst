package appserver

// This opt-in integration test joins the real Snaplink JWT boundary, the
// owner-bound client-instance pair, and the planning-only Runner
// execution-intent candidate. The pair is read before the execution-intent
// POST; no client registration, lease, target selection, Runner transport, or
// command execution is mounted.

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
	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestSnaplinkAuthenticatedClientInstanceRunnerExecutionIntentConvergenceE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_RUNNER_EXECUTION_INTENT_CONVERGENCE_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_RUNNER_EXECUTION_INTENT_CONVERGENCE_E2E=1 for paired Runner execution-intent E2E")
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
	instances := []deviceplacement.ClientInstanceSessionViewInstance{
		{InstanceID: "client-cli", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{"conversation-001", "conversation-002"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-tui", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{"conversation-002"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-web", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{"conversation-001"}, ObservedAtMS: 200_500, Status: "idle"},
		{InstanceID: "client-app", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{"conversation-001"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-mobile", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{"conversation-002"}, ObservedAtMS: 200_500, Status: "idle"},
	}
	sessionView, err := deviceplacement.ObserveClientInstanceSessionView(
		deviceplacement.ClientInstanceSessionViewRequest{Owner: owner, Instances: instances},
	)
	if err != nil {
		t.Fatalf("observe paired session view: %v", err)
	}
	resourceView := fixtureClientInstanceResourceView(owner)
	resourceView.Instances = append([]deviceplacement.ClientInstanceSessionViewInstance(nil), sessionView.Instances...)
	if err := resourceView.Validate(); err != nil {
		t.Fatalf("validate paired resource view: %v", err)
	}
	sessionSource := &fixtureClientInstanceSessionViewSource{value: sessionView}
	resourceSource := &fixtureClientInstanceResourceViewSource{value: resourceView}
	inventorySource := &fixtureDeviceInventoryReadV2Source{
		value: preflightProjectionInventory(t, owner, resourceView),
	}
	recorder := &conversationHTTPRecorder{}

	testRoutes := http.NewServeMux()
	testRoutes.Handle(clientInstanceSessionViewCandidatePath,
		newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
			Enabled: true, Source: sessionSource,
		}))
	testRoutes.Handle(clientInstanceResourceViewCandidatePath,
		newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
			Enabled: true, Source: resourceSource,
		}))
	testRoutes.Handle(deviceInventoryReadCandidateV2Path,
		newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
			Enabled: true, Source: inventorySource,
		}))
	testRoutes.Handle(
		"/api/v1/conversations/conversation-001/runs/run-001/runner-execution-intent/preview",
		newRunnerExecutionIntentPreviewRoutes(),
	)
	server := httptest.NewServer(authenticator.Handler(recorder.wrap(testRoutes)))
	t.Cleanup(server.Close)

	client := server.Client()
	gotSession := readSnaplinkClientInstanceSessionView(t, client, server.URL, token)
	gotResource := readSnaplinkClientInstanceResourceView(t, client, server.URL, token)
	if gotSession.Owner != owner || gotResource.Owner != owner ||
		sessionSource.calls != 1 || resourceSource.calls != 1 ||
		sessionSource.owner != (model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) ||
		resourceSource.owner != (model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) ||
		len(gotSession.Instances) != 5 || len(gotResource.Instances) != 5 ||
		string(marshalConvergenceJSON(t, gotSession.Instances)) != string(marshalConvergenceJSON(t, gotResource.Instances)) {
		t.Fatalf("owner/pair convergence session=%#v resource=%#v sources=%#v/%#v",
			gotSession, gotResource, sessionSource, resourceSource)
	}
	if len(gotResource.Devices) != 1 || gotResource.Devices[0].DeviceID != "device-a" ||
		gotResource.Devices[0].RunnerInstanceID != "runner-a" || !gotResource.ReadOnly {
		t.Fatalf("paired resource devices=%#v", gotResource.Devices)
	}

	request := runnerExecutionIntentPreviewRequest(owner, "conversation-001", "run-001")
	expected, err := deviceplacement.ObserveRunnerExecutionIntent(request)
	if err != nil {
		t.Fatalf("paired Runner execution-intent fixture is invalid: %v request=%#v", err, request)
	}
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	previewPath := "/api/v1/conversations/conversation-001/runs/run-001/runner-execution-intent/preview"
	previewResponse := snaplinkConversationRequest(t, client, server.URL, token,
		http.MethodPost, previewPath, "", string(body))
	if previewResponse.StatusCode != http.StatusOK {
		t.Fatalf("paired Runner execution-intent status=%d body=%q",
			previewResponse.StatusCode, readConversationClientBody(t, previewResponse))
	}
	var preview deviceplacement.RunnerExecutionIntentObservation
	previewBody := readConversationClientBody(t, previewResponse)
	if err := json.Unmarshal([]byte(previewBody), &preview); err != nil {
		t.Fatalf("decode paired Runner execution-intent: %v body=%q", err, previewBody)
	}
	if preview != expected ||
		preview.Owner != owner || preview.SelectedTargetID != nil ||
		preview.Authority != (deviceplacement.RunnerExecutionIntentAuthority{}) ||
		!preview.PreviewOnly {
		t.Fatalf("paired Runner execution-intent=%#v expected=%#v", preview, expected)
	}
	for _, forbidden := range []string{"fencing_token", "argv", "workspace_ref"} {
		if strings.Contains(previewBody, forbidden) {
			t.Fatalf("Runner execution-intent leaked %q: %q", forbidden, previewBody)
		}
	}

	if configuredExecutable := os.Getenv("FORGE_RUNTIME_BIN"); configuredExecutable != "" {
		executable, err := exec.LookPath(configuredExecutable)
		if err != nil {
			t.Fatalf("FORGE_RUNTIME_BIN must name a built forge-runtime executable: %v", err)
		}
		executable, err = filepath.Abs(executable)
		if err != nil {
			t.Fatal(err)
		}
		inputPath := filepath.Join(t.TempDir(), "runner-execution-intent-client-instance-request.json")
		if err := os.WriteFile(inputPath, body, 0o600); err != nil {
			t.Fatal(err)
		}

		visibleStart := len(recorder.snapshot())
		output, stderr, err := runForgeRuntimeCLI(
			t, executable, server.URL, token, t.TempDir(), "--json", "remote",
			"runner-execution-intent-preview", "--input", inputPath, "--instance", "client-cli",
		)
		if err != nil {
			t.Fatalf("authenticated instance-filtered Runner execution-intent CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
		}
		var cliObservation deviceplacement.RunnerExecutionIntentObservation
		if err := json.Unmarshal([]byte(output), &cliObservation); err != nil {
			t.Fatalf("decode instance-filtered Runner execution-intent CLI: %v stdout=%q", err, output)
		}
		if cliObservation != expected || cliObservation.SelectedTargetID != nil ||
			cliObservation.Authority != (deviceplacement.RunnerExecutionIntentAuthority{}) {
			t.Fatalf("instance-filtered Runner execution-intent=%#v expected=%#v", cliObservation, expected)
		}
		assertRunnerExecutionIntentCLIRequestOrder(t, recorder.snapshot()[visibleStart:], previewPath, true)

		hiddenStart := len(recorder.snapshot())
		_, hiddenStderr, hiddenErr := runForgeRuntimeCLI(
			t, executable, server.URL, token, t.TempDir(), "--json", "remote",
			"runner-execution-intent-preview", "--input", inputPath, "--instance", "client-tui",
		)
		if hiddenErr == nil || !strings.Contains(hiddenStderr, "no Runner execution-intent request was sent") {
			t.Fatalf("hidden instance-filtered Runner execution-intent was not rejected: stderr=%q err=%v", hiddenStderr, hiddenErr)
		}
		assertRunnerExecutionIntentCLIRequestOrder(t, recorder.snapshot()[hiddenStart:], previewPath, false)
		if strings.Contains(output, request.Command.LeaseProof.FencingToken) ||
			strings.Contains(output, request.Command.WorkspaceRef) ||
			strings.Contains(output, request.Command.Argv[0]) {
			t.Fatalf("Runner execution-intent CLI leaked private execution material: %q", output)
		}
	}

	// The ordinary production constructor remains closed even for the full
	// Snaplink JWT. Pair and candidate routes are mounted only on this test mux.
	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	productionRequest := httptest.NewRequest(http.MethodPost, previewPath, strings.NewReader(string(body)))
	productionRequest.Header.Set("Authorization", "Bearer "+token)
	productionRequest.Header.Set("Content-Type", "application/json")
	productionResponse := httptest.NewRecorder()
	production.ServeHTTP(productionResponse, productionRequest)
	if productionResponse.Code != http.StatusNotFound ||
		productionResponse.Body.String() != string(notFoundBody) {
		t.Fatalf("default production Runner execution-intent status=%d body=%q",
			productionResponse.Code, productionResponse.Body.String())
	}
}

func assertRunnerExecutionIntentCLIRequestOrder(
	t *testing.T,
	requests []recordedConversationRequest,
	previewPath string,
	visible bool,
) {
	t.Helper()
	want := []struct {
		method string
		path   string
	}{
		{method: http.MethodGet, path: clientInstanceSessionViewCandidatePath},
		{method: http.MethodGet, path: clientInstanceResourceViewCandidatePath},
	}
	if visible {
		want = append(want,
			struct {
				method string
				path   string
			}{method: http.MethodGet, path: deviceInventoryReadCandidateV2Path},
			struct {
				method string
				path   string
			}{method: http.MethodGet, path: clientInstanceResourceViewCandidatePath},
			struct {
				method string
				path   string
			}{method: http.MethodPost, path: previewPath},
		)
	}
	if len(requests) != len(want) {
		t.Fatalf("Runner execution-intent CLI request count=%d want=%d: %#v", len(requests), len(want), requests)
	}
	for index := range want {
		if requests[index].method != want[index].method || requests[index].path != want[index].path {
			t.Fatalf("Runner execution-intent CLI request[%d]=%s %s want=%s %s: %#v",
				index, requests[index].method, requests[index].path, want[index].method, want[index].path, requests)
		}
	}
}

func readSnaplinkClientInstanceSessionView(
	t *testing.T,
	client *http.Client,
	baseURL, token string,
) deviceplacement.ClientInstanceSessionViewObservation {
	t.Helper()
	response := snaplinkConversationRequest(t, client, baseURL, token,
		http.MethodGet, clientInstanceSessionViewCandidatePath, "", "")
	if response.StatusCode != http.StatusOK {
		t.Fatalf("paired session view status=%d body=%q", response.StatusCode,
			readConversationClientBody(t, response))
	}
	var view deviceplacement.ClientInstanceSessionViewObservation
	body := readConversationClientBody(t, response)
	if err := json.Unmarshal([]byte(body), &view); err != nil {
		t.Fatalf("decode paired session view: %v body=%q", err, body)
	}
	if err := view.Validate(); err != nil {
		t.Fatalf("validate paired session view: %v", err)
	}
	return view
}

func readSnaplinkClientInstanceResourceView(
	t *testing.T,
	client *http.Client,
	baseURL, token string,
) deviceplacement.ClientInstanceResourceViewObservation {
	t.Helper()
	response := snaplinkConversationRequest(t, client, baseURL, token,
		http.MethodGet, clientInstanceResourceViewCandidatePath, "", "")
	if response.StatusCode != http.StatusOK {
		t.Fatalf("paired resource view status=%d body=%q", response.StatusCode,
			readConversationClientBody(t, response))
	}
	var view deviceplacement.ClientInstanceResourceViewObservation
	body := readConversationClientBody(t, response)
	if err := json.Unmarshal([]byte(body), &view); err != nil {
		t.Fatalf("decode paired resource view: %v body=%q", err, body)
	}
	if err := view.Validate(); err != nil {
		t.Fatalf("validate paired resource view: %v", err)
	}
	return view
}
