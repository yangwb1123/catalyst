package appserver

import (
	"net/http"
	"testing"
)

// TestLifecycleCandidatesRemainUnregisteredOnSessionCoordinator keeps the
// injected enrollment/heartbeat lifecycle mux separate from the ordinary
// authenticated Conversation server.  These paths contain identity,
// heartbeat, approval, and credential candidate shapes; exposing any of them
// from the normal constructor would bypass the ADR-0114 activation boundary.
func TestLifecycleCandidatesRemainUnregisteredOnSessionCoordinator(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newConversationRoutesWithBackend(nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name, method, target, contentType, body string
	}{
		{
			name: "lifecycle registry read", method: http.MethodGet,
			target: lifecycleRegistryCandidatePath,
		},
		{
			name: "lifecycle registry replacement", method: http.MethodPut,
			target: lifecycleRegistryCandidatePath, contentType: "application/json",
			body: `{"states":[]}`,
		},
		{
			name: "heartbeat candidate", method: http.MethodPost,
			target: lifecycleHeartbeatCandidatePath, contentType: "application/json",
			body: `{}`,
		},
		{
			name: "signed heartbeat candidate", method: http.MethodPost,
			target: lifecycleSignedHeartbeatCandidatePath, contentType: "application/json",
			body: `{}`,
		},
		{
			name: "challenge candidate", method: http.MethodPost,
			target: lifecycleChallengeCandidatePath, contentType: "application/json",
			body: `{}`,
		},
		{
			name: "approval candidate", method: http.MethodPost,
			target: lifecycleApprovalCandidatePath, contentType: "application/json",
			body: `{}`,
		},
		{
			name: "credential candidate", method: http.MethodPost,
			target: lifecycleCredentialCandidatePath, contentType: "application/json",
			body: `{}`,
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(
				t, routes, identity, test.method, test.target,
				"forge:conversations:read forge:conversations:write forge:devices:lifecycle:read forge:devices:lifecycle:write forge:devices:lifecycle:heartbeat forge:devices:lifecycle:heartbeat:signed forge:devices:lifecycle:challenge forge:devices:lifecycle:approval forge:devices:lifecycle:credential",
				test.contentType, "default-off-"+test.name, test.body,
			)
			if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
				t.Fatalf("lifecycle candidate route status=%d body=%q", response.Code, response.Body.String())
			}
			assertContractHeaders(t, response.Header(), len(notFoundBody), "")
		})
	}
}
