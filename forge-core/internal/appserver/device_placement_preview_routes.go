package appserver

import (
	"bytes"
	"net/http"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/executionprofile"
	"forgeos/forge-core/internal/runtimebridge"
)

// devicePlacementPreviewPath accepts one complete, caller-supplied placement
// declaration and returns a deterministic comparison. It deliberately does
// not read a Forge device registry: the declarations are observations supplied
// by the caller and remain unverified.
const devicePlacementPreviewPath = "/api/v1/device-placement/preview"

// devicePlacementPreviewScope is the existing read scope because this route
// returns no server-owned device data and has no mutation effect. A future
// authoritative inventory route must use a separately reviewed device scope.
const devicePlacementPreviewScope = "forge:conversations:read"

func newDevicePlacementPreviewRoutes() http.Handler {
	return authn.RequireScopes(http.HandlerFunc(serveDevicePlacementPreview), devicePlacementPreviewScope)
}

func serveDevicePlacementPreview(w http.ResponseWriter, r *http.Request) {
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "placement preview does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	body, err := readStrictConversationJSON(w, r, new(any))
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if len(body) > deviceplacement.MaxRequestBytes {
		writeConversationError(w, r, http.StatusRequestEntityTooLarge, "request_too_large", "request body exceeds the placement preview limit")
		return
	}
	request, err := deviceplacement.Decode(bytes.NewReader(body))
	if err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "placement preview request is invalid")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if request.Owner.Issuer != owner.Issuer || request.Owner.Subject != owner.Subject || request.Owner.TenantID != owner.TenantID {
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "placement owner must match the authenticated principal")
		return
	}
	result, err := deviceplacement.Evaluate(request)
	if err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "placement preview request is invalid")
		return
	}
	encoded, err := deviceplacement.Marshal(result)
	if err != nil {
		writeConversationError(w, r, http.StatusInternalServerError, "internal_error", "placement preview could not be encoded")
		return
	}
	writeJSON(w, r, http.StatusOK, append(encoded, '\n'))
}

// authenticatedSessionRoutes combines the existing owner-scoped session API
// with the stateless observation candidates. It is intentionally constructed
// only by focused tests; the production constructor below returns the plain
// conversation handler so these candidates remain closed by default.
type authenticatedSessionRoutes struct {
	conversations                    http.Handler
	placement                        http.Handler
	deviceObservation                http.Handler
	sessionRunnerReceipt             http.Handler
	localRunnerPreview               http.Handler
	runAttemptLeaseDispatchPreflight http.Handler
	runnerDispatchPlanPreview        http.Handler
	runObserved                      http.Handler
	executionReconciliation          http.Handler
}

func (routes authenticatedSessionRoutes) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if r.URL.EscapedPath() == devicePlacementPreviewPath {
		routes.placement.ServeHTTP(w, r)
		return
	}
	if _, _, ok := sessionDeviceObservationPathIDs(r.URL.EscapedPath()); ok {
		routes.deviceObservation.ServeHTTP(w, r)
		return
	}
	if _, _, ok := sessionRunnerReceiptObservationPathIDs(r.URL.EscapedPath()); ok {
		routes.sessionRunnerReceipt.ServeHTTP(w, r)
		return
	}
	if _, _, ok := localRunnerPreviewPathIDs(r.URL.EscapedPath()); ok {
		if routes.localRunnerPreview == nil {
			writeJSON(w, r, http.StatusNotFound, notFoundBody)
			return
		}
		routes.localRunnerPreview.ServeHTTP(w, r)
		return
	}
	if _, _, ok := runAttemptLeaseDispatchPreflightPathIDs(r.URL.EscapedPath()); ok {
		if routes.runAttemptLeaseDispatchPreflight == nil {
			writeJSON(w, r, http.StatusNotFound, notFoundBody)
			return
		}
		routes.runAttemptLeaseDispatchPreflight.ServeHTTP(w, r)
		return
	}
	if _, _, ok := runnerDispatchPlanPreviewPathIDs(r.URL.EscapedPath()); ok {
		if routes.runnerDispatchPlanPreview == nil {
			writeJSON(w, r, http.StatusNotFound, notFoundBody)
			return
		}
		routes.runnerDispatchPlanPreview.ServeHTTP(w, r)
		return
	}
	if _, _, ok := conversationRunObservedIDs(r.URL.EscapedPath()); ok {
		if routes.runObserved == nil {
			writeJSON(w, r, http.StatusNotFound, notFoundBody)
			return
		}
		routes.runObserved.ServeHTTP(w, r)
		return
	}
	if _, _, ok := executionReconciliationPreviewPathIDs(r.URL.EscapedPath()); ok {
		if routes.executionReconciliation == nil {
			writeJSON(w, r, http.StatusNotFound, notFoundBody)
			return
		}
		routes.executionReconciliation.ServeHTTP(w, r)
		return
	}
	routes.conversations.ServeHTTP(w, r)
}

func newAuthenticatedSessionRoutes(client *runtimebridge.Client, profiles *executionprofile.Catalog) http.Handler {
	return newConversationRoutesWithExecutionProfiles(client, profiles)
}

// newAuthenticatedSessionRoutesWithObservationCandidates composes the
// read-only placement and observation candidates for focused tests and
// explicitly opt-in integration fixtures. It must not be used by Run.
func newAuthenticatedSessionRoutesWithObservationCandidates(
	client *runtimebridge.Client,
	profiles *executionprofile.Catalog,
) http.Handler {
	return newAuthenticatedSessionRoutesWithObservationCandidatesBackend(client, profiles)
}

func newAuthenticatedSessionRoutesWithObservationCandidatesBackend(
	backend conversationBackend,
	profiles *executionprofile.Catalog,
) http.Handler {
	return authenticatedSessionRoutes{
		conversations:                    newConversationRoutesWithBackendAndExecutionProfiles(backend, profiles),
		placement:                        newDevicePlacementPreviewRoutes(),
		deviceObservation:                newSessionDeviceObservationRoutes(),
		sessionRunnerReceipt:             newSessionRunnerReceiptObservationRoutes(),
		runAttemptLeaseDispatchPreflight: newRunAttemptLeaseDispatchPreflightRoutes(),
		runnerDispatchPlanPreview:        newRunnerDispatchPlanPreviewRoutes(),
		runObserved:                      newRunObservedRoutes(backend),
		executionReconciliation:          newExecutionReconciliationPreviewRoutes(),
	}
}
