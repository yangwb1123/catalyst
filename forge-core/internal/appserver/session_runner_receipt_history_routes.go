package appserver

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/url"
	"strings"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
)

// sessionRunnerReceiptHistoryPreviewScope is the existing conversation read
// scope. The route reduces a caller-supplied history and never reads receipt,
// Run, lease, or Runner state.
const sessionRunnerReceiptHistoryPreviewScope = "forge:conversations:read"

func newSessionRunnerReceiptHistoryRoutes() http.Handler {
	return authn.RequireScopes(
		http.HandlerFunc(serveSessionRunnerReceiptHistoryPreview),
		sessionRunnerReceiptHistoryPreviewScope,
	)
}

func serveSessionRunnerReceiptHistoryPreview(w http.ResponseWriter, r *http.Request) {
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "Runner receipt history preview does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	var input deviceplacement.SessionRunnerReceiptHistoryObservation
	body, err := readStrictConversationJSON(w, r, &input)
	if err != nil || !hasExactSessionRunnerReceiptHistoryFields(body) {
		if err == nil {
			err = errConversationJSON
		}
		writeConversationRequestError(w, r, err)
		return
	}
	conversationID, runID, ok := sessionRunnerReceiptHistoryPathIDs(r.URL.EscapedPath())
	if !ok || input.ConversationID != conversationID || input.RunID != runID {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_binding", "history Conversation and Run must match the path")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if input.Owner.Issuer != owner.Issuer || input.Owner.Subject != owner.Subject || input.Owner.TenantID != owner.TenantID {
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "Runner receipt history owner must match the authenticated principal")
		return
	}
	if err := input.Validate(); err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Runner receipt history is invalid")
		return
	}
	derived, err := deviceplacement.ObserveSessionRunnerReceiptHistory(deviceplacement.SessionRunnerReceiptHistoryRequest{
		Owner: input.Owner, ConversationID: input.ConversationID, PromptID: input.PromptID,
		RunID: input.RunID, Receipts: input.Receipts,
	})
	if err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Runner receipt history is invalid")
		return
	}
	encoded, err := json.Marshal(derived)
	if err != nil {
		writeConversationError(w, r, http.StatusInternalServerError, "internal_error", "Runner receipt history could not be encoded")
		return
	}
	writeJSON(w, r, http.StatusOK, append(encoded, '\n'))
}

func sessionRunnerReceiptHistoryPathIDs(path string) (string, string, bool) {
	conversationID, suffix, ok := conversationPathSuffix(path)
	if !ok {
		return "", "", false
	}
	parts := strings.Split(suffix, "/")
	if len(parts) != 4 || parts[0] != "runs" || parts[1] == "" || parts[2] != "runner-receipt-history" || parts[3] != "preview" {
		return "", "", false
	}
	runID, err := url.PathUnescape(parts[1])
	if err != nil || strings.ContainsAny(runID, "/\x00") || strings.TrimSpace(runID) == "" || len(runID) > conversationIDMaxBytes {
		return "", "", false
	}
	return conversationID, runID, true
}

func hasExactSessionRunnerReceiptHistoryFields(body []byte) bool {
	var object map[string]json.RawMessage
	if err := json.Unmarshal(body, &object); err != nil || object == nil || len(object) != 20 {
		return false
	}
	for _, field := range []string{
		"schema_version", "evaluation_mode", "owner", "conversation_id", "prompt_id", "run_id", "receipts",
		"attempt_count", "latest_attempt_id", "latest_command_id", "latest_target_id", "latest_disposition_kind",
		"latest_observed_at_ms", "reconciliation_required", "manual_review_required", "automatic_retry", "follow_up",
		"selected_target_id", "preview_only", "authority",
	} {
		value, ok := object[field]
		if !ok || len(value) == 0 || bytes.Equal(bytes.TrimSpace(value), []byte("null")) {
			if field == "selected_target_id" && ok && bytes.Equal(bytes.TrimSpace(value), []byte("null")) {
				continue
			}
			return false
		}
	}
	if !hasExactSessionRunnerReceiptJSONObject(object["owner"], "issuer", "subject", "tenant_id") ||
		!hasExactSessionRunnerReceiptJSONObject(object["authority"], "identity_verified", "receipt_persisted", "execution_authorized", "dispatch_performed", "audit_published") {
		return false
	}
	var receipts []json.RawMessage
	if err := json.Unmarshal(object["receipts"], &receipts); err != nil || len(receipts) == 0 || len(receipts) > deviceplacement.MaxSessionRunnerReceiptHistory {
		return false
	}
	for _, receipt := range receipts {
		if !hasExactSessionRunnerReceiptObservationFields(receipt) {
			return false
		}
	}
	return true
}
