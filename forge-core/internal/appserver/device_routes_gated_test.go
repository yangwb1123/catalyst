package appserver

import (
	"net/http"
	"testing"
)

func TestDeviceRoutesRemainUnregisteredOnSessionCoordinator(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newConversationRoutesWithBackend(nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name, method, target, contentType, idempotencyKey, body string
	}{
		{"inventory candidate", http.MethodGet, deviceInventoryReadCandidatePath, "", "", ""},
		{"lossless inventory candidate", http.MethodGet, deviceInventoryReadCandidateV2Path, "", "", ""},
		{"inventory", http.MethodGet, "/api/v1/devices?limit=25", "", "", ""},
		{"enrollment", http.MethodPost, "/api/v1/devices/enrollments", "application/json", "device-enrollment-gate-probe", `{}`},
		{"heartbeat", http.MethodPost, "/api/v1/devices/device-1/heartbeats", "application/json", "device-heartbeat-gate-probe", `{}`},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(
				t, routes, identity, test.method, test.target,
				"forge:conversations:read forge:conversations:write",
				test.contentType, test.idempotencyKey, test.body,
			)
			if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
				t.Fatalf("default device route response = %d %q", response.Code, response.Body.String())
			}
			assertContractHeaders(t, response.Header(), len(notFoundBody), "")
		})
	}
}
