package appserver

import (
	"net/http"
	"net/url"
	"strings"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// runAttemptLeaseDispatchPreflightPath is a private, stateless candidate. It
// joins caller-supplied Run/Attempt/lease/placement declarations but never
// reads a Run store, device registry, clock, or Runner.
const runAttemptLeaseDispatchPreflightScope = "forge:conversations:read"

func newRunAttemptLeaseDispatchPreflightRoutes() http.Handler {
	return authn.RequireScopes(
		http.HandlerFunc(serveRunAttemptLeaseDispatchPreflight),
		runAttemptLeaseDispatchPreflightScope,
	)
}

func serveRunAttemptLeaseDispatchPreflight(w http.ResponseWriter, r *http.Request) {
	conversationID, runID, ok := runAttemptLeaseDispatchPreflightPathIDs(r.URL.EscapedPath())
	if !ok {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return
	}
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "Run/Attempt/lease preflight does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	var request deviceplacement.RunAttemptLeaseDispatchPreflightRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if !hasExactRequiredFields(body, "owner", "conversation_id", "run_id", "run_status", "dispatch_plan") {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Run/Attempt/lease preflight request is invalid")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if !samePreflightOwner(request.Owner, owner) || !samePreflightOwner(request.DispatchPlan.Intent.Owner, owner) {
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "Run/Attempt/lease preflight owner must match the authenticated principal")
		return
	}
	if request.ConversationID != conversationID || request.RunID != runID ||
		request.DispatchPlan.Intent.ConversationID != conversationID ||
		request.DispatchPlan.Intent.RunID != runID {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_binding", "Run/Attempt/lease preflight identities must match the path")
		return
	}
	observation, err := deviceplacement.ObserveRunAttemptLeaseDispatchPreflight(request)
	if err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Run/Attempt/lease preflight request is invalid")
		return
	}
	if !validRunAttemptLeaseDispatchPreflightObservation(observation, owner, conversationID, runID) {
		writeConversationError(w, r, http.StatusBadGateway, "run_attempt_lease_dispatch_preflight_invalid", "Run/Attempt/lease preflight result is invalid")
		return
	}
	writeConversationJSON(w, r, http.StatusOK, observation)
}

func validRunAttemptLeaseDispatchPreflightObservation(
	value deviceplacement.RunAttemptLeaseDispatchPreflightObservation,
	owner model.Owner,
	conversationID string,
	runID string,
) bool {
	return value.Validate() == nil &&
		samePreflightOwner(value.Owner, owner) &&
		value.ConversationID == conversationID &&
		value.RunID == runID &&
		value.SelectedTargetID == nil &&
		value.PreviewOnly &&
		value.Authority == (deviceplacement.RunAttemptLeaseDispatchPreflightAuthority{})
}

func samePreflightOwner(value deviceplacement.Owner, owner model.Owner) bool {
	return value.Issuer == owner.Issuer && value.Subject == owner.Subject && value.TenantID == owner.TenantID
}

func runAttemptLeaseDispatchPreflightPathIDs(path string) (string, string, bool) {
	conversationID, suffix, ok := conversationPathSuffix(path)
	if !ok {
		return "", "", false
	}
	parts := strings.Split(suffix, "/")
	if len(parts) != 4 || parts[0] != "runs" || parts[1] == "" ||
		parts[2] != "attempt-lease-dispatch-preflight" || parts[3] != "preview" {
		return "", "", false
	}
	runID, err := url.PathUnescape(parts[1])
	if err != nil || !validExecutionRouteID(runID) {
		return "", "", false
	}
	return conversationID, runID, true
}
