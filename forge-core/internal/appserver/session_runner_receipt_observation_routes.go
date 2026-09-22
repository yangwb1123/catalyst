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

// sessionRunnerReceiptObservationPreviewScope is the existing conversation
// read scope. This route only revalidates a caller-supplied value observation;
// it does not read Hub state or grant Runner/device authority.
const sessionRunnerReceiptObservationPreviewScope = "forge:conversations:read"

func newSessionRunnerReceiptObservationRoutes() http.Handler {
	return authn.RequireScopes(
		http.HandlerFunc(serveSessionRunnerReceiptObservationPreview),
		sessionRunnerReceiptObservationPreviewScope,
	)
}

func serveSessionRunnerReceiptObservationPreview(w http.ResponseWriter, r *http.Request) {
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "Runner receipt observation preview does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	var observation deviceplacement.SessionRunnerReceiptObservation
	body, err := readStrictConversationJSON(w, r, &observation)
	if err != nil || !hasExactSessionRunnerReceiptObservationFields(body) {
		if err == nil {
			err = errConversationJSON
		}
		writeConversationRequestError(w, r, err)
		return
	}
	conversationID, runID, ok := sessionRunnerReceiptObservationPathIDs(r.URL.EscapedPath())
	if !ok || observation.ConversationID != conversationID || observation.RunID != runID {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_binding", "observation Conversation and Run must match the path")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if observation.Owner.Issuer != owner.Issuer || observation.Owner.Subject != owner.Subject || observation.Owner.TenantID != owner.TenantID {
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "Runner receipt observation owner must match the authenticated principal")
		return
	}
	if err := observation.Validate(); err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Runner receipt observation is invalid")
		return
	}
	encoded, err := json.Marshal(observation)
	if err != nil {
		writeConversationError(w, r, http.StatusInternalServerError, "internal_error", "Runner receipt observation could not be encoded")
		return
	}
	writeJSON(w, r, http.StatusOK, append(encoded, '\n'))
}

func sessionRunnerReceiptObservationPathIDs(path string) (string, string, bool) {
	conversationID, suffix, ok := conversationPathSuffix(path)
	if !ok {
		return "", "", false
	}
	parts := strings.Split(suffix, "/")
	if len(parts) != 4 || parts[0] != "runs" || parts[1] == "" || parts[2] != "runner-receipt-observation" || parts[3] != "preview" {
		return "", "", false
	}
	runID, err := url.PathUnescape(parts[1])
	if err != nil || strings.ContainsAny(runID, "/\x00") || strings.TrimSpace(runID) == "" || len(runID) > conversationIDMaxBytes {
		return "", "", false
	}
	return conversationID, runID, true
}

func hasExactSessionRunnerReceiptObservationFields(body []byte) bool {
	var object map[string]json.RawMessage
	if err := json.Unmarshal(body, &object); err != nil || object == nil || len(object) != 12 {
		return false
	}
	for _, field := range []string{
		"schema_version", "evaluation_mode", "owner", "conversation_id", "prompt_id", "run_id",
		"receipt_observation", "prompt_run_binding_valid", "receipt_binding_valid", "preview_only", "authority",
	} {
		value, ok := object[field]
		if !ok || len(value) == 0 || bytes.Equal(bytes.TrimSpace(value), []byte("null")) {
			return false
		}
	}
	// selected_target_id is required to be present and canonically null.
	selectedTarget, ok := object["selected_target_id"]
	if !ok || !bytes.Equal(bytes.TrimSpace(selectedTarget), []byte("null")) {
		return false
	}
	if !hasExactSessionRunnerReceiptJSONObject(object["owner"], "issuer", "subject", "tenant_id") ||
		!hasExactSessionRunnerReceiptJSONObject(object["authority"],
			"identity_verified", "receipt_persisted", "execution_authorized", "dispatch_performed", "audit_published") {
		return false
	}
	receipt, ok := exactSessionRunnerReceiptJSONObject(object["receipt_observation"],
		"schema_version", "evaluation_mode", "command_id", "command_sha256", "attempt_id", "target_id",
		"disposition_kind", "observed_at_ms", "receipt_valid", "preview_only", "uncertain",
		"reconciliation_required", "manual_review_required", "automatic_retry", "follow_up", "authority")
	return ok && hasExactSessionRunnerReceiptJSONObject(receipt["authority"],
		"device_identity_verified", "command_persisted", "reservation_created", "execution_authorized",
		"dispatch_performed", "audit_published")
}

func exactSessionRunnerReceiptJSONObject(value []byte, fields ...string) (map[string]json.RawMessage, bool) {
	var object map[string]json.RawMessage
	if err := json.Unmarshal(value, &object); err != nil || object == nil || len(object) != len(fields) {
		return nil, false
	}
	for _, field := range fields {
		entry, ok := object[field]
		if !ok || len(entry) == 0 || bytes.Equal(bytes.TrimSpace(entry), []byte("null")) {
			return nil, false
		}
	}
	return object, true
}

func hasExactSessionRunnerReceiptJSONObject(value []byte, fields ...string) bool {
	_, ok := exactSessionRunnerReceiptJSONObject(value, fields...)
	return ok
}
