package appserver

import (
	"net/http"
	"testing"

	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
)

// TestAcceptedDeviceFabricAssemblyKeepsLifecycleMutationCandidatesClosed
// protects the boundary between the accepted owner-scoped read image and the
// future device-credential enrollment surface. An accepted INVENTORY or
// OBSERVE activation may expose read-only projections, but it must not mount
// the candidate challenge, heartbeat, approval, credential, or registry replacement
// handlers. Those handlers currently use an injected test store and are not a
// production device-identity transport.
func TestAcceptedDeviceFabricAssemblyKeepsLifecycleMutationCandidatesClosed(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{
		lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1),
	})

	for _, mode := range []devicefabricgate.Mode{
		devicefabricgate.ModeInventory,
		devicefabricgate.ModeObserve,
	} {
		t.Run(string(mode), func(t *testing.T) {
			activation := acceptedInventoryActivation()
			activation.Mode = mode
			sessions, err := newAuthenticatedSessionRoutesWithDeviceFabricActivation(
				nil, nil, ptrDeviceFabricRequest(activation), registryPath, "",
			)
			if err != nil {
				t.Fatalf("accepted %s activation assembly: %v", mode, err)
			}
			handler := authenticator.Handler(sessions)
			for _, route := range []struct {
				name, method, path, scope, contentType, body string
			}{
				{
					name: "heartbeat", method: http.MethodPost,
					path: lifecycleHeartbeatCandidatePath, scope: lifecycleHeartbeatCandidateScope,
					contentType: "application/json", body: `{}`,
				},
				{
					name: "signed heartbeat", method: http.MethodPost,
					path: lifecycleSignedHeartbeatCandidatePath, scope: lifecycleSignedHeartbeatCandidateScope,
					contentType: "application/json", body: `{}`,
				},
				{
					name: "challenge", method: http.MethodPost,
					path: lifecycleChallengeCandidatePath, scope: lifecycleChallengeCandidateScope,
					contentType: "application/json", body: `{}`,
				},
				{
					name: "approval", method: http.MethodPost,
					path: lifecycleApprovalCandidatePath, scope: lifecycleApprovalCandidateScope,
					contentType: "application/json", body: `{}`,
				},
				{
					name: "credential", method: http.MethodPost,
					path: lifecycleCredentialCandidatePath, scope: lifecycleCredentialCandidateScope,
					contentType: "application/json", body: `{}`,
				},
				{
					name: "registry replacement", method: http.MethodPut,
					// The accepted read mux authenticates this path with its read
					// scope before rejecting every non-GET method. Using that
					// scope here proves the request reached the read-only guard;
					// a write scope must be rejected earlier with 403.
					path: lifecycleRegistryCandidatePath, scope: lifecycleRegistryCandidateReadScope,
					contentType: "application/json", body: `{"states":[]}`,
				},
			} {
				t.Run(route.name, func(t *testing.T) {
					response := requestConversationAPI(t, handler, identity, route.method, route.path,
						route.scope, route.contentType, "", route.body)
					if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
						t.Fatalf("accepted %s lifecycle route status=%d body=%q", mode, response.Code, response.Body.String())
					}
					assertContractHeaders(t, response.Header(), len(notFoundBody), "")
				})
			}
		})
	}
}
