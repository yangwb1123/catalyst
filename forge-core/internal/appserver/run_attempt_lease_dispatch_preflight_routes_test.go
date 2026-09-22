package appserver

import (
	"encoding/json"
	"net/http"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceplacement"
)

func TestRunAttemptLeaseDispatchPreflightCandidateBindsOwnerAndPath(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	request := runAttemptLeaseDispatchPreflightRequest(owner, "conversation-1", "run-1")
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-1/runs/run-1/attempt-lease-dispatch-preflight/preview"
	response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", string(body))
	if response.Code != http.StatusOK {
		t.Fatalf("preflight status=%d body=%q", response.Code, response.Body.String())
	}
	if !strings.Contains(response.Body.String(), `"rejection_reasons":[]`) {
		t.Fatalf("preflight must encode an empty rejection list as an array: body=%q", response.Body.String())
	}
	var observation deviceplacement.RunAttemptLeaseDispatchPreflightObservation
	if err := json.Unmarshal(response.Body.Bytes(), &observation); err != nil {
		t.Fatalf("decode preflight: %v body=%q", err, response.Body.String())
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate preflight: %v body=%q", err, response.Body.String())
	}
	if observation.Owner != owner || observation.ConversationID != "conversation-1" ||
		observation.RunID != "run-1" || !observation.DeclarativePreflightReady ||
		observation.SelectedTargetID != nil || observation.Authority != (deviceplacement.RunAttemptLeaseDispatchPreflightAuthority{}) {
		t.Fatalf("preflight observation=%#v", observation)
	}
	assertContractHeaders(t, response.Header(), response.Body.Len(), "")

	foreign := requestConversationAPIAs(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "account-foreign", "tenant-slate", "application/json", "", string(body))
	if foreign.Code != http.StatusForbidden {
		t.Fatalf("foreign owner status=%d body=%q", foreign.Code, foreign.Body.String())
	}
	wrongPath := requestConversationAPI(t, routes, identity, http.MethodPost,
		"/api/v1/conversations/other/runs/run-1/attempt-lease-dispatch-preflight/preview",
		"forge:conversations:read", "application/json", "", string(body))
	if wrongPath.Code != http.StatusBadRequest {
		t.Fatalf("wrong path status=%d body=%q", wrongPath.Code, wrongPath.Body.String())
	}
	if method := requestConversationAPI(t, routes, identity, http.MethodGet, path,
		"forge:conversations:read", "", "", ""); method.Code != http.StatusMethodNotAllowed || method.Header().Get("Allow") != http.MethodPost {
		t.Fatalf("method status=%d allow=%q body=%q", method.Code, method.Header().Get("Allow"), method.Body.String())
	}
	if query := requestConversationAPI(t, routes, identity, http.MethodPost, path+"?debug=1",
		"forge:conversations:read", "application/json", "", string(body)); query.Code != http.StatusBadRequest {
		t.Fatalf("query status=%d body=%q", query.Code, query.Body.String())
	}
	if noScope := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:write", "application/json", "", string(body)); noScope.Code != http.StatusForbidden {
		t.Fatalf("scope status=%d body=%q", noScope.Code, noScope.Body.String())
	}
	for _, target := range []string{"/api/v1/devices", "/api/v1/devices/runner-1/heartbeats"} {
		deviceResponse := requestConversationAPI(t, routes, identity, http.MethodGet, target,
			"forge:conversations:read", "", "", "")
		if deviceResponse.Code != http.StatusNotFound || deviceResponse.Body.String() != string(notFoundBody) {
			t.Fatalf("device route %s status=%d body=%q", target, deviceResponse.Code, deviceResponse.Body.String())
		}
	}
}

func TestRunAttemptLeaseDispatchPreflightCandidateRejectsAmbiguousBodiesAndProductionStaysClosed(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	candidate := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	request := runAttemptLeaseDispatchPreflightRequest(owner, "conversation-1", "run-1")
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-1/runs/run-1/attempt-lease-dispatch-preflight/preview"
	for _, test := range []struct {
		name string
		body string
		want int
	}{
		{name: "unknown root field", body: strings.Replace(string(body), `{"owner":`, `{"unexpected":true,"owner":`, 1), want: http.StatusBadRequest},
		{name: "duplicate root field", body: strings.Replace(string(body), `"run_status":"nonterminal"`, `"run_status":"nonterminal","run_status":"nonterminal"`, 1), want: http.StatusBadRequest},
		{name: "duplicate nested field", body: strings.Replace(string(body), `"attempt_state":"accepted"`, `"attempt_state":"accepted","attempt_state":"accepted"`, 1), want: http.StatusBadRequest},
		{name: "malformed body", body: `{}`, want: http.StatusBadRequest},
	} {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(t, candidate, identity, http.MethodPost, path,
				"forge:conversations:read", "application/json", "", test.body)
			if response.Code != test.want {
				t.Fatalf("status=%d body=%q want=%d", response.Code, response.Body.String(), test.want)
			}
		})
	}

	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	response := requestConversationAPI(t, production, identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", string(body))
	if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
		t.Fatalf("production preflight status=%d body=%q", response.Code, response.Body.String())
	}
}

func runAttemptLeaseDispatchPreflightRequest(
	owner deviceplacement.Owner,
	conversationID string,
	runID string,
) deviceplacement.RunAttemptLeaseDispatchPreflightRequest {
	commandSHA := strings.Repeat("a", 64)
	placement := deviceplacement.Request{
		SchemaVersion: deviceplacement.RequestSchemaVersion, EvaluatedAtMS: 1_800_000_000_000,
		Owner: owner, MaxSnapshotAgeMS: 60_000,
		Requirements: deviceplacement.Requirements{
			OS: "linux", Architecture: "amd64", MinCPUCores: 4,
			MinMemoryBytes: 8 << 30, MinStorageBytes: 20 << 30, Runtime: "oci",
			DataResidencyZones: []string{"us-west"}, MinimumTrustZone: "standard",
			SandboxFloor: "container", ConcurrencySlots: 1,
		},
		Devices: []deviceplacement.Device{
			preflightDevice(owner, "runner-1"),
			preflightDevice(owner, "runner-2"),
		},
	}
	intent := deviceplacement.RunnerExecutionIntentObservation{
		SchemaVersion:  deviceplacement.RunnerExecutionIntentSchemaVersion,
		EvaluationMode: deviceplacement.RunnerExecutionIntentEvaluationMode,
		Owner:          owner, ConversationID: conversationID, PromptID: "prompt-1", RunID: runID,
		AttemptID: "attempt-1", CommandID: "command-1", TargetID: "runner-1",
		CommandSHA256: commandSHA, IdempotencyKey: runID + ":attempt-1:command-1",
		PromptRunBindingValid: true, RunnerCommandBindingValid: true, PreviewOnly: true,
	}
	return deviceplacement.RunAttemptLeaseDispatchPreflightRequest{
		Owner: owner, ConversationID: conversationID, RunID: runID, RunStatus: "nonterminal",
		DispatchPlan: deviceplacement.RunnerDispatchPlanPreviewRequest{
			AttemptState: "accepted", Placement: placement, Intent: intent,
			Lease: deviceplacement.RunnerTerminalLeaseGrant{
				V: 1, AttemptID: "attempt-1", TargetID: "runner-1", Epoch: 1,
				FencingToken: "fence-1", IssuedAtMS: 1_799_999_999_000, ExpiresAtMS: 1_800_000_005_000,
			},
		},
	}
}

func preflightDevice(owner deviceplacement.Owner, id string) deviceplacement.Device {
	return deviceplacement.Device{
		DeviceID: id, Owner: owner, ApprovalState: "approved", CordonState: "clear", Liveness: "online",
		SnapshotObservedAtMS: 1_799_999_999_000, LeaseExpiresAtMS: 1_800_000_060_000,
		OS: "linux", Architecture: "amd64", AvailableCPUCores: 8, AvailableMemoryBytes: 16 << 30,
		AvailableStorage: 100 << 30, Runtimes: []string{"oci"}, DataResidencyZones: []string{"us-west"},
		TrustZone: "standard", SandboxLevels: []string{"container"}, ConcurrencyLimit: 4,
		ActiveConcurrency: 1,
	}
}
