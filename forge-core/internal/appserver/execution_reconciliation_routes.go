package appserver

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/url"
	"strings"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/executionreconcile"
)

// executionReconciliationPreviewScope is intentionally the existing
// conversation read scope. The endpoint only classifies a caller-supplied
// restart image; it never reads or mutates Run, Attempt, lease, registry, or
// Runner state.
const executionReconciliationPreviewScope = "forge:conversations:read"

func newExecutionReconciliationPreviewRoutes() http.Handler {
	return authn.RequireScopes(
		http.HandlerFunc(serveExecutionReconciliationPreview),
		executionReconciliationPreviewScope,
	)
}

func serveExecutionReconciliationPreview(w http.ResponseWriter, r *http.Request) {
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "execution reconciliation preview does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	var input executionreconcile.Input
	body, err := readStrictConversationJSON(w, r, &input)
	if err != nil || !hasExactExecutionReconciliationInputFields(body) {
		if err == nil {
			err = errConversationJSON
		}
		writeConversationRequestError(w, r, err)
		return
	}
	conversationID, runID, ok := executionReconciliationPreviewPathIDs(r.URL.EscapedPath())
	if !ok || input.ConversationID != conversationID || input.RunID != runID {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_binding", "execution reconciliation Conversation and Run must match the path")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if input.Owner.Issuer != owner.Issuer || input.Owner.Subject != owner.Subject || input.Owner.TenantID != owner.TenantID {
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "execution reconciliation owner must match the authenticated principal")
		return
	}
	observation, err := executionreconcile.Observe(input)
	if err != nil || observation.Validate() != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "execution reconciliation input is invalid")
		return
	}
	encoded, err := json.Marshal(observation)
	if err != nil {
		writeConversationError(w, r, http.StatusInternalServerError, "internal_error", "execution reconciliation could not be encoded")
		return
	}
	writeJSON(w, r, http.StatusOK, append(encoded, '\n'))
}

func executionReconciliationPreviewPathIDs(path string) (string, string, bool) {
	conversationID, suffix, ok := conversationPathSuffix(path)
	if !ok {
		return "", "", false
	}
	parts := strings.Split(suffix, "/")
	if len(parts) != 4 || parts[0] != "runs" || parts[1] == "" ||
		parts[2] != "execution-reconciliation" || parts[3] != "preview" {
		return "", "", false
	}
	runID, err := url.PathUnescape(parts[1])
	if err != nil || !validExecutionRouteID(runID) {
		return "", "", false
	}
	return conversationID, runID, true
}

func hasExactExecutionReconciliationInputFields(body []byte) bool {
	var object map[string]json.RawMessage
	if err := json.Unmarshal(body, &object); err != nil || object == nil || len(object) != 11 {
		return false
	}
	for _, field := range []string{
		"owner", "conversation_id", "run_id", "attempt_id", "command_id", "target_id",
		"run_status", "attempt_state", "lease", "observed_at_ms",
	} {
		value, ok := object[field]
		if !ok || len(value) == 0 || bytes.Equal(bytes.TrimSpace(value), []byte("null")) {
			return false
		}
	}
	// terminal is optional in the input, but its key is required so the wire
	// shape cannot silently change between absent and explicit null.
	_, ok := object["terminal"]
	return ok
}
