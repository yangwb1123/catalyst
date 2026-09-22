package appserver

import (
	"encoding/json"
	"net/http"
	"net/url"
	"strings"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// runnerDispatchPlanPreviewScope is deliberately the existing conversation
// read scope. This candidate evaluates caller-supplied declarations only; it
// does not read a registry, create a lease, select a target, or dispatch work.
const runnerDispatchPlanPreviewScope = "forge:conversations:read"

// newRunnerDispatchPlanPreviewRoutes constructs the explicitly injected
// owner/path-bound dispatch-plan observation. The production Coordinator
// constructor never mounts this handler.
func newRunnerDispatchPlanPreviewRoutes() http.Handler {
	return authn.RequireScopes(
		http.HandlerFunc(serveRunnerDispatchPlanPreview),
		runnerDispatchPlanPreviewScope,
	)
}

func serveRunnerDispatchPlanPreview(w http.ResponseWriter, r *http.Request) {
	conversationID, runID, ok := runnerDispatchPlanPreviewPathIDs(r.URL.EscapedPath())
	if !ok {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return
	}
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "Runner dispatch-plan preview does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	var request deviceplacement.RunnerDispatchPlanPreviewRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if !hasExactRequiredFields(body, "attempt_state", "placement_request", "runner_execution_intent", "lease") {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Runner dispatch-plan preview request is invalid")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if !sameRunnerDispatchOwner(request.Placement.Owner, owner) ||
		!sameRunnerDispatchOwner(request.Intent.Owner, owner) {
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "Runner dispatch-plan preview owner must match the authenticated principal")
		return
	}
	if request.Intent.ConversationID != conversationID || request.Intent.RunID != runID {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_binding", "Runner dispatch-plan preview identities must match the path")
		return
	}
	observation, err := deviceplacement.ObserveRunnerDispatchPlanPreview(request)
	if err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Runner dispatch-plan preview request is invalid")
		return
	}
	if !validRunnerDispatchPlanPreviewObservation(observation, owner, conversationID, runID) {
		writeConversationError(w, r, http.StatusBadGateway, "runner_dispatch_plan_preview_invalid", "Runner dispatch-plan preview result is invalid")
		return
	}
	encoded, err := json.Marshal(observation)
	if err != nil {
		writeConversationError(w, r, http.StatusInternalServerError, "internal_error", "Runner dispatch-plan preview could not be encoded")
		return
	}
	writeJSON(w, r, http.StatusOK, append(encoded, '\n'))
}

func validRunnerDispatchPlanPreviewObservation(
	value deviceplacement.RunnerDispatchPlanPreviewObservation,
	owner model.Owner,
	conversationID string,
	runID string,
) bool {
	return value.Validate() == nil &&
		sameRunnerDispatchOwner(value.Owner, owner) &&
		value.ConversationID == conversationID && value.RunID == runID &&
		value.SelectedTargetID == nil && value.PreviewOnly &&
		value.Authority == (deviceplacement.RunnerDispatchPlanPreviewAuthority{})
}

func sameRunnerDispatchOwner(value deviceplacement.Owner, owner model.Owner) bool {
	return value.Issuer == owner.Issuer && value.Subject == owner.Subject && value.TenantID == owner.TenantID
}

func runnerDispatchPlanPreviewPathIDs(path string) (string, string, bool) {
	conversationID, suffix, ok := conversationPathSuffix(path)
	if !ok {
		return "", "", false
	}
	parts := strings.Split(suffix, "/")
	if len(parts) != 3 || parts[0] != "runs" || parts[1] == "" || parts[2] != "runner-dispatch-plan-preview" {
		return "", "", false
	}
	runID, err := url.PathUnescape(parts[1])
	if err != nil || !validExecutionRouteID(runID) {
		return "", "", false
	}
	return conversationID, runID, true
}
