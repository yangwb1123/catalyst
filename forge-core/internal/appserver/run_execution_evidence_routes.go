package appserver

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/url"
	"strings"

	"forgeos/forge-core/internal/auditprojection"
	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/runtimebridge/model"
)

// runExecutionEvidencePreview is a read-only binding of two already projected
// observations. It is mounted only by the explicit observation/EXECUTE
// candidate constructors; the ordinary Run constructor keeps it closed.
const runExecutionEvidencePreviewPath = "/api/v1/conversations/%s/runs/%s/execution-evidence/preview"

const runExecutionEvidencePreviewScope = "forge:conversations:read"

func newRunExecutionEvidencePreviewRoutes() http.Handler {
	return authn.RequireScopes(
		http.HandlerFunc(serveRunExecutionEvidencePreview),
		runExecutionEvidencePreviewScope,
	)
}

type runExecutionEvidencePreviewRequest struct {
	RunObserved            auditprojection.RunObserved                     `json:"run_observed"`
	SessionReceiptObserved deviceplacement.SessionRunnerReceiptObservation `json:"session_receipt_observed"`
}

func serveRunExecutionEvidencePreview(w http.ResponseWriter, r *http.Request) {
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "Run execution evidence preview does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	var request runExecutionEvidencePreviewRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil || !hasExactRunExecutionEvidenceInputFields(body) {
		if err == nil {
			err = errConversationJSON
		}
		writeConversationRequestError(w, r, err)
		return
	}
	conversationID, runID, ok := runExecutionEvidencePreviewPathIDs(r.URL.EscapedPath())
	if !ok || request.RunObserved.ConversationID != conversationID ||
		request.RunObserved.RunID != runID || request.SessionReceiptObserved.ConversationID != conversationID ||
		request.SessionReceiptObserved.RunID != runID {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_binding", "Run execution evidence observations must match the path")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	declaredOwner := model.Owner{
		Issuer:   request.SessionReceiptObserved.Owner.Issuer,
		Subject:  request.SessionReceiptObserved.Owner.Subject,
		TenantID: request.SessionReceiptObserved.Owner.TenantID,
	}
	if declaredOwner != owner {
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "Run execution evidence owner must match the authenticated principal")
		return
	}
	writeRunExecutionEvidencePreview(w, r, request)
}

func writeRunExecutionEvidencePreview(
	w http.ResponseWriter,
	r *http.Request,
	request runExecutionEvidencePreviewRequest,
) {
	evidence, err := deviceplacement.ObserveRunExecutionEvidence(deviceplacement.RunExecutionEvidenceInput{
		Run:     request.RunObserved,
		Receipt: request.SessionReceiptObserved,
	})
	if err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Run execution evidence observations are invalid")
		return
	}
	encoded, err := json.Marshal(evidence)
	if err != nil {
		writeConversationError(w, r, http.StatusInternalServerError, "internal_error", "Run execution evidence could not be encoded")
		return
	}
	writeJSON(w, r, http.StatusOK, append(encoded, '\n'))
}

func runExecutionEvidencePreviewPathIDs(path string) (string, string, bool) {
	conversationID, suffix, ok := conversationPathSuffix(path)
	if !ok {
		return "", "", false
	}
	parts := strings.Split(suffix, "/")
	if len(parts) != 4 || parts[0] != "runs" || parts[1] == "" ||
		parts[2] != "execution-evidence" || parts[3] != "preview" {
		return "", "", false
	}
	runID, err := url.PathUnescape(parts[1])
	if err != nil || !validExecutionRouteID(runID) {
		return "", "", false
	}
	return conversationID, runID, true
}

func hasExactRunExecutionEvidenceInputFields(body []byte) bool {
	var object map[string]json.RawMessage
	if err := json.Unmarshal(body, &object); err != nil || object == nil || len(object) != 2 {
		return false
	}
	for _, field := range []string{"run_observed", "session_receipt_observed"} {
		value, ok := object[field]
		if !ok || len(value) == 0 || bytes.Equal(bytes.TrimSpace(value), []byte("null")) {
			return false
		}
	}
	return true
}
