package appserver

import (
	"net/http"
	"testing"

	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
)

// TestAcceptedNonExecuteDeviceFabricKeepsExecutionEvidenceClosed protects the
// second half of the activation boundary. An accepted INVENTORY or OBSERVE
// image may expose owner-scoped resource projections, but it must not make
// receipt, reconciliation, Attempt, scheduler, or dispatch candidates
// reachable. Those surfaces require the separately reviewed EXECUTE/P4 path.
func TestAcceptedNonExecuteDeviceFabricKeepsExecutionEvidenceClosed(t *testing.T) {
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
			const scopes = "forge:conversations:read forge:devices:placement:preview forge:devices:placement:lease"
			for _, route := range []struct {
				name, method, target string
			}{
				{name: "runner execution intent", method: http.MethodPost, target: "/api/v1/conversations/conversation-1/runs/run-1/runner-execution-intent/preview"},
				{name: "runner dispatch plan", method: http.MethodPost, target: "/api/v1/conversations/conversation-1/runs/run-1/runner-dispatch-plan-preview"},
				{name: "runner dispatch admission", method: http.MethodPost, target: "/api/v1/conversations/conversation-1/runs/run-1/runner-dispatch-admission/preview"},
				{name: "runner transport admission", method: http.MethodPost, target: "/api/v1/conversations/conversation-1/runs/run-1/runner-transport-admission/preview"},
				{name: "runner execution boundary", method: http.MethodPost, target: "/api/v1/conversations/conversation-1/runs/run-1/runner-execution-boundary/preview"},
				{name: "runner Attempt boundary", method: http.MethodPost, target: "/api/v1/conversations/conversation-1/runs/run-1/runner-attempt-boundary/preview"},
				{name: "runner receipt", method: http.MethodPost, target: "/api/v1/conversations/conversation-1/runs/run-1/runner-receipt-observation/preview"},
				{name: "runner receipt history", method: http.MethodPost, target: "/api/v1/conversations/conversation-1/runs/run-1/runner-receipt-history/preview"},
				{name: "runner reconciliation", method: http.MethodPost, target: "/api/v1/conversations/conversation-1/runs/run-1/runner-reconciliation/preview"},
				{name: "Attempt preflight", method: http.MethodPost, target: "/api/v1/conversations/conversation-1/runs/run-1/attempt-lease-dispatch-preflight/preview"},
				{name: "execution reconciliation", method: http.MethodPost, target: "/api/v1/conversations/conversation-1/runs/run-1/execution-reconciliation/preview"},
				{name: "execution evidence", method: http.MethodPost, target: "/api/v1/conversations/conversation-1/runs/run-1/execution-evidence/preview"},
				{name: "scheduler selection", method: http.MethodPost, target: schedulerSelectionPreviewPath},
				{name: "scheduler lease claim", method: http.MethodPost, target: schedulerSelectionLeasePath},
				{name: "scheduler lease renewal", method: http.MethodPost, target: schedulerSelectionLeaseRenewalPath},
				{name: "scheduler lease release", method: http.MethodPost, target: schedulerSelectionLeaseReleasePath},
			} {
				t.Run(route.name, func(t *testing.T) {
					response := requestConversationAPI(t, handler, identity, route.method, route.target,
						scopes, "application/json", "non-execute-"+route.name, `{}`)
					if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
						t.Fatalf("accepted %s execution-adjacent route status=%d body=%q", mode, response.Code, response.Body.String())
					}
					assertContractHeaders(t, response.Header(), len(notFoundBody), "")
				})
			}
		})
	}
}
