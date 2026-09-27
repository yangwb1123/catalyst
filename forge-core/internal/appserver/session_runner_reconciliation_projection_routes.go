package appserver

import (
	"encoding/json"
	"net/http"
	"net/url"
	"strings"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
)

const sessionRunnerReconciliationProjectionScope = "forge:conversations:read"

func newSessionRunnerReconciliationProjectionRoutes() http.Handler {
	return authn.RequireScopes(
		http.HandlerFunc(serveSessionRunnerReconciliationProjection),
		sessionRunnerReconciliationProjectionScope,
	)
}

func serveSessionRunnerReconciliationProjection(w http.ResponseWriter, r *http.Request) {
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "Runner reconciliation preview does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	var history deviceplacement.SessionRunnerReceiptHistoryObservation
	body, err := readStrictConversationJSON(w, r, &history)
	if err != nil || !hasExactSessionRunnerReceiptHistoryFields(body) {
		if err == nil {
			err = errConversationJSON
		}
		writeConversationRequestError(w, r, err)
		return
	}
	conversationID, runID, ok := sessionRunnerReconciliationProjectionPathIDs(r.URL.EscapedPath())
	if !ok || history.ConversationID != conversationID || history.RunID != runID {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_binding", "reconciliation Conversation and Run must match the path")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if history.Owner.Issuer != owner.Issuer || history.Owner.Subject != owner.Subject || history.Owner.TenantID != owner.TenantID {
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "Runner reconciliation owner must match the authenticated principal")
		return
	}
	projection, err := deviceplacement.ProjectSessionRunnerReconciliation(history)
	if err != nil || projection.Validate() != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Runner reconciliation history is invalid")
		return
	}
	encoded, err := json.Marshal(projection)
	if err != nil {
		writeConversationError(w, r, http.StatusInternalServerError, "internal_error", "Runner reconciliation projection could not be encoded")
		return
	}
	writeJSON(w, r, http.StatusOK, append(encoded, '\n'))
}

func sessionRunnerReconciliationProjectionPathIDs(path string) (string, string, bool) {
	conversationID, suffix, ok := conversationPathSuffix(path)
	if !ok {
		return "", "", false
	}
	parts := strings.Split(suffix, "/")
	if len(parts) != 4 || parts[0] != "runs" || parts[1] == "" ||
		parts[2] != "runner-reconciliation" || parts[3] != "preview" {
		return "", "", false
	}
	runID, err := url.PathUnescape(parts[1])
	if err != nil || strings.ContainsAny(runID, "/\x00") || strings.TrimSpace(runID) == "" ||
		len(runID) > conversationIDMaxBytes {
		return "", "", false
	}
	return conversationID, runID, true
}
