package appserver

import (
	"net/http"
	"testing"
)

func TestDeviceRoutesRemainDefaultOffWithDeviceScope(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name, method, target, contentType, body string
	}{
		{name: "inventory read", method: http.MethodGet, target: "/api/v1/devices"},
		{name: "inventory trailing slash", method: http.MethodGet, target: "/api/v1/devices/"},
		{name: "enrollment", method: http.MethodPost, target: "/api/v1/devices/enrollments", contentType: "application/json", body: `{}`},
		{name: "heartbeat", method: http.MethodPost, target: "/api/v1/devices/device-a/heartbeats", contentType: "application/json", body: `{}`},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(t, routes, identity, test.method, test.target,
				deviceInventoryReadCandidateScope, test.contentType, "default-off-"+test.name, test.body)
			if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
				t.Fatalf("device route status=%d body=%q", response.Code, response.Body.String())
			}
			assertContractHeaders(t, response.Header(), len(notFoundBody), "")
		})
	}
}

func TestObservationCandidatesRemainDefaultOffFromProductionSessionConstructor(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name, method, target, body string
	}{
		{
			name: "lifecycle registry read", method: http.MethodGet,
			target: lifecycleRegistryCandidatePath,
		},
		{
			name: "placement preview", method: http.MethodPost,
			target: devicePlacementPreviewPath, body: `{}`,
		},
		{
			name: "registry placement preview", method: http.MethodPost,
			target: devicePlacementRegistryCandidatePath, body: `{}`,
		},
		{
			name: "session device observation", method: http.MethodPost,
			target: "/api/v1/conversations/conversation-1/runs/run-1/device-observation/preview", body: `{}`,
		},
		{
			name: "runner receipt observation", method: http.MethodPost,
			target: "/api/v1/conversations/conversation-1/runs/run-1/runner-receipt-observation/preview", body: `{}`,
		},
		{
			name: "runner receipt history", method: http.MethodPost,
			target: "/api/v1/conversations/conversation-1/runs/run-1/runner-receipt-history/preview", body: `{}`,
		},
		{
			name: "runner reconciliation projection", method: http.MethodPost,
			target: "/api/v1/conversations/conversation-1/runs/run-1/runner-reconciliation/preview", body: `{}`,
		},
		{
			name: "local runner preview", method: http.MethodPost,
			target: "/api/v1/conversations/conversation-1/run-intents/intent-1/execution-readiness-preview", body: `{}`,
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(t, routes, identity, test.method, test.target,
				"forge:conversations:read", "default-off-"+test.name, "application/json", test.body)
			if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
				t.Fatalf("production observation route status=%d body=%q", response.Code, response.Body.String())
			}
			assertContractHeaders(t, response.Header(), len(notFoundBody), "")
		})
	}
}
