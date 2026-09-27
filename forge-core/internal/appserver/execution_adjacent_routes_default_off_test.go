package appserver

import (
	"net/http"
	"testing"
)

// TestExecutionAdjacentRoutesRemainDefaultOffFromProductionSessionConstructor
// keeps every execution-adjacent candidate behind the explicit device-fabric
// activation assembly. A normal Coordinator construction must not expose a
// preview, admission, boundary, evidence, scheduler, or lease route merely
// because the request is authenticated.
func TestExecutionAdjacentRoutesRemainDefaultOffFromProductionSessionConstructor(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}

	const scopes = "forge:conversations:read forge:devices:placement:preview forge:devices:placement:lease"
	tests := []struct {
		name, method, target, contentType, body string
	}{
		{
			name: "runner execution-intent preview", method: http.MethodPost,
			target:      "/api/v1/conversations/conversation-1/runs/run-1/runner-execution-intent/preview",
			contentType: "application/json", body: `{}`,
		},
		{
			name: "runner dispatch-plan preview", method: http.MethodPost,
			target:      "/api/v1/conversations/conversation-1/runs/run-1/runner-dispatch-plan-preview",
			contentType: "application/json", body: `{}`,
		},
		{
			name: "runner dispatch-admission preview", method: http.MethodPost,
			target:      "/api/v1/conversations/conversation-1/runs/run-1/runner-dispatch-admission/preview",
			contentType: "application/json", body: `{}`,
		},
		{
			name: "runner transport-admission preview", method: http.MethodPost,
			target:      "/api/v1/conversations/conversation-1/runs/run-1/runner-transport-admission/preview",
			contentType: "application/json", body: `{}`,
		},
		{
			name: "runner execution-boundary preview", method: http.MethodPost,
			target:      "/api/v1/conversations/conversation-1/runs/run-1/runner-execution-boundary/preview",
			contentType: "application/json", body: `{}`,
		},
		{
			name: "runner attempt-boundary preview", method: http.MethodPost,
			target:      "/api/v1/conversations/conversation-1/runs/run-1/runner-attempt-boundary/preview",
			contentType: "application/json", body: `{}`,
		},
		{
			name: "execution reconciliation preview", method: http.MethodPost,
			target:      "/api/v1/conversations/conversation-1/runs/run-1/execution-reconciliation/preview",
			contentType: "application/json", body: `{}`,
		},
		{
			name: "execution evidence preview", method: http.MethodPost,
			target:      "/api/v1/conversations/conversation-1/runs/run-1/execution-evidence/preview",
			contentType: "application/json", body: `{}`,
		},
		{
			name: "scheduler selection preview", method: http.MethodPost,
			target: schedulerSelectionPreviewPath, contentType: "application/json", body: `{}`,
		},
		{
			name: "scheduler lease claim", method: http.MethodPost,
			target: schedulerSelectionLeasePath, contentType: "application/json", body: `{}`,
		},
		{
			name: "scheduler lease renewal", method: http.MethodPost,
			target: schedulerSelectionLeaseRenewalPath, contentType: "application/json", body: `{}`,
		},
		{
			name: "scheduler lease release", method: http.MethodPost,
			target: schedulerSelectionLeaseReleasePath, contentType: "application/json", body: `{}`,
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(t, routes, identity, test.method, test.target,
				scopes, test.contentType, "default-off-"+test.name, test.body)
			if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
				t.Fatalf("execution-adjacent route status=%d body=%q", response.Code, response.Body.String())
			}
			assertContractHeaders(t, response.Header(), len(notFoundBody), "")
		})
	}
}
