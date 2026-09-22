package appserver

import (
	"encoding/json"
	"forgeos/forge-core/internal/runtimebridge"
	"net/http"
	"net/url"
	"strings"

	"forgeos/forge-core/internal/authn"
)

// runObservedPath is an explicitly enabled, read-only evidence candidate. It
// is intentionally not registered by the production session constructor.
const runObservedPathSuffix = "observation"

const runObservedScope = "forge:conversations:read"

func newRunObservedRoutes(backend conversationBackend) http.Handler {
	return authn.RequireScopes(
		http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			serveRunObserved(w, r, backend)
		}),
		runObservedScope,
	)
}

func serveRunObserved(w http.ResponseWriter, r *http.Request, backend conversationBackend) {
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "Run observation does not accept query parameters")
		return
	}
	if r.Method != http.MethodGet {
		w.Header().Set("Allow", http.MethodGet)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	if requestHasBody(r) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "GET requests must not include a body")
		return
	}
	conversationID, runID, ok := conversationRunObservedIDs(r.URL.EscapedPath())
	if !ok {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if backend == nil {
		writeConversationBackendUnavailable(w, r)
		return
	}

	observed, err := backend.OwnedConversationRunObservation(r.Context(), owner, conversationID, runID)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if !runObservedJSONSafe(observed, owner, conversationID, runID) {
		writeConversationBackendError(w, r, &runtimebridge.Error{Code: "invalid_runtime_response"})
		return
	}
	body, err := json.Marshal(observed)
	if err != nil {
		writeConversationError(w, r, http.StatusInternalServerError, "internal_error", "Run observation could not be encoded")
		return
	}
	writeJSON(w, r, http.StatusOK, append(body, '\n'))
}

func conversationRunObservedIDs(path string) (string, string, bool) {
	conversationID, suffix, ok := conversationPathSuffix(path)
	if !ok {
		return "", "", false
	}
	parts := strings.Split(suffix, "/")
	if len(parts) != 3 || parts[0] != "runs" || parts[1] == "" || parts[2] != runObservedPathSuffix {
		return "", "", false
	}
	runID, err := url.PathUnescape(parts[1])
	if err != nil || strings.ContainsAny(runID, "/\x00") || strings.TrimSpace(runID) == "" || len(runID) > conversationIDMaxBytes {
		return "", "", false
	}
	return conversationID, runID, true
}
