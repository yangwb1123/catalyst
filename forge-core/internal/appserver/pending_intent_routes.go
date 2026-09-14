package appserver

import (
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"net/http"
	"strings"

	"forgeos/forge-core/internal/executionprofile"
)

const pendingIntentPageMax = 25

type submitPendingIntentRequest struct {
	Content         string `json:"content"`
	ExpectedVersion uint64 `json:"expected_version"`
}

func (routes executionRoutes) submitPendingIntent(w http.ResponseWriter, r *http.Request) {
	conversationID, _ := r.Context().Value(conversationIDContextKey{}).(string)
	key, err := requestIdempotencyKey(r)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	var request submitPendingIntentRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if !hasExactRequiredFields(body, "content", "expected_version") ||
		request.ExpectedVersion > maxSQLiteCursor || requestContentInvalid(request.Content) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "pending intent request is invalid")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if routes.backend == nil {
		writeConversationBackendUnavailable(w, r)
		return
	}
	profile, err := routes.serverProfile(r, owner, conversationID)
	if err != nil {
		writeExecutionBackendError(w, r, err)
		return
	}
	result, err := routes.backend.SubmitOwnedPromptRunIntent(
		r.Context(), owner, conversationID, request.Content, key, request.ExpectedVersion, profile,
	)
	if err != nil {
		writeExecutionBackendError(w, r, err)
		return
	}
	status := http.StatusCreated
	if result.Replayed {
		status = http.StatusOK
	}
	writeConversationJSON(w, r, status, result)
}

func requestContentInvalid(content string) bool {
	return strings.TrimSpace(content) == "" || len(content) > promptContentMaxBytes
}

func (routes executionRoutes) serverProfile(
	r *http.Request,
	owner model.Owner,
	conversationID string,
) (intentmodel.ServerExecutionProfile, error) {
	if routes.reader == nil || routes.profiles == nil {
		return intentmodel.ServerExecutionProfile{}, executionprofile.ErrProfileUnavailable
	}
	return routes.profiles.ResolveConversationProfile(r.Context(), routes.reader, owner, conversationID)
}

func (routes executionRoutes) listPendingIntents(w http.ResponseWriter, r *http.Request) {
	if requestHasBody(r) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "GET requests must not include a body")
		return
	}
	conversationID, _ := r.Context().Value(conversationIDContextKey{}).(string)
	query, err := parseConversationQuery(r, "limit", "before_submitted_at_ms", "before_intent_id")
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	limit := pendingIntentPageMax
	if _, hasLimit := query["limit"]; hasLimit {
		limit, err = parseConversationLimit(query)
		if err != nil || limit > pendingIntentPageMax {
			writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "pagination query is invalid")
			return
		}
	}
	var before *intentmodel.PendingRunIntentCursor
	times, hasTime := query["before_submitted_at_ms"]
	ids, hasID := query["before_intent_id"]
	if hasTime != hasID {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "pagination query is invalid")
		return
	}
	if hasTime {
		at, parseErr := parseUnsignedDecimal(times[0])
		if parseErr != nil || at > maxSQLiteCursor || !validExecutionRouteID(ids[0]) {
			writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "pagination query is invalid")
			return
		}
		before = &intentmodel.PendingRunIntentCursor{SubmittedAtMS: at, IntentID: ids[0]}
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if routes.backend == nil {
		writeConversationBackendUnavailable(w, r)
		return
	}
	page, err := routes.backend.OwnedConversationPendingRunIntents(r.Context(), owner, conversationID, before, limit)
	if err != nil {
		writeExecutionBackendError(w, r, err)
		return
	}
	writeConversationJSON(w, r, http.StatusOK, page)
}

func (routes executionRoutes) pendingIntentTimeline(w http.ResponseWriter, r *http.Request) {
	if requestHasBody(r) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "GET requests must not include a body")
		return
	}
	conversationID, _ := r.Context().Value(conversationIDContextKey{}).(string)
	intentID, _ := r.Context().Value(intentIDContextKey{}).(string)
	query, err := parseConversationQuery(r, "after_sequence", "limit")
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	after := uint64(0)
	if values, ok := query["after_sequence"]; ok {
		after, err = parseUnsignedDecimal(values[0])
		if err != nil || after > maxSQLiteCursor {
			writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "pagination query is invalid")
			return
		}
	}
	limit := pendingIntentPageMax
	if _, hasLimit := query["limit"]; hasLimit {
		limit, err = parseConversationLimit(query)
		if err != nil || limit > pendingIntentPageMax {
			writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "pagination query is invalid")
			return
		}
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if routes.backend == nil {
		writeConversationBackendUnavailable(w, r)
		return
	}
	page, err := routes.backend.OwnedConversationPendingRunIntentTimeline(
		r.Context(), owner, conversationID, intentID, after, limit,
	)
	if err != nil {
		writeExecutionBackendError(w, r, err)
		return
	}
	writeConversationJSON(w, r, http.StatusOK, page)
}
