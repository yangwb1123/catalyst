package appserver

import (
	"encoding/json"
	"forgeos/forge-core/internal/runtimebridge"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
	"net/http"
	"strings"
)

func (routes conversationRoutes) getConversation(w http.ResponseWriter, r *http.Request) {
	if requestHasBody(r) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "GET requests must not include a body")
		return
	}
	if _, err := parseConversationQuery(r); err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	conversationID, _ := r.Context().Value(conversationIDContextKey{}).(string)
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if routes.backend == nil {
		writeConversationBackendUnavailable(w, r)
		return
	}
	entry, err := routes.backend.GetOwnedConversation(r.Context(), owner, conversationID)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if !conversationEntryJSONSafe(entry) {
		writeConversationBackendError(w, r, &runtimebridge.Error{Code: "invalid_runtime_response"})
		return
	}
	writeConversationJSON(w, r, http.StatusOK, entry)
}

func (routes conversationRoutes) listConversations(w http.ResponseWriter, r *http.Request) {
	if requestHasBody(r) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "GET requests must not include a body")
		return
	}
	query, err := parseConversationQuery(r, "after_id", "limit")
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	afterID := ""
	if values, ok := query["after_id"]; ok {
		afterID = values[0]
		if afterID == "" || len(afterID) > conversationIDMaxBytes || strings.TrimSpace(afterID) == "" {
			writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "pagination query is invalid")
			return
		}
	}
	limit, err := parseConversationLimit(query)
	if err != nil {
		writeConversationRequestError(w, r, err)
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
	page, err := routes.backend.ListOwnedConversations(r.Context(), owner, afterID, limit)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if !conversationPageJSONSafe(page) {
		writeConversationBackendError(w, r, &runtimebridge.Error{Code: "invalid_runtime_response"})
		return
	}
	writeConversationJSON(w, r, http.StatusOK, page)
}

func (routes conversationRoutes) createConversation(w http.ResponseWriter, r *http.Request) {
	key, err := requestIdempotencyKey(r)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	var request createConversationRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if !hasExactRequiredFields(body, "scope", "title") || !validConversationScopeShape(body) ||
		strings.TrimSpace(request.Title) == "" || len(request.Title) > 256 {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "conversation request is invalid")
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
	conversation, err := routes.backend.CreateOwnedConversation(r.Context(), owner, request.Scope, request.Title, key)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if !conversationTimestampsJSONSafe(conversation) {
		writeConversationBackendError(w, r, &runtimebridge.Error{Code: "invalid_runtime_response"})
		return
	}
	writeConversationJSON(w, r, http.StatusCreated, conversation)
}

func (routes conversationRoutes) importConversationHandler(w http.ResponseWriter, r *http.Request) {
	key, err := requestIdempotencyKey(r)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	var request importConversationRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if !hasExactRequiredFields(body, "title", "prompts") || !validConversationImportPromptShape(body) ||
		strings.TrimSpace(request.Title) == "" || len(request.Title) > 256 ||
		len(request.Prompts) > conversationImportPromptMax {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "conversation import request is invalid")
		return
	}
	totalContentBytes := 0
	for _, prompt := range request.Prompts {
		contentBytes := len(prompt.Content)
		if (prompt.Role != "user" && prompt.Role != "assistant") || strings.TrimSpace(prompt.Content) == "" ||
			contentBytes > conversationImportContentMaxBytes-totalContentBytes {
			writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "conversation import request is invalid")
			return
		}
		totalContentBytes += contentBytes
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
	result, err := routes.backend.ImportOwnedConversation(r.Context(), owner, request.Title, request.Prompts, key)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if !conversationImportJSONSafe(result) {
		writeConversationBackendError(w, r, &runtimebridge.Error{Code: "invalid_runtime_response"})
		return
	}
	status := http.StatusCreated
	if result.Replayed {
		status = http.StatusOK
	}
	writeConversationJSON(w, r, status, result)
}

func validConversationImportPromptShape(body []byte) bool {
	var root map[string]json.RawMessage
	if json.Unmarshal(body, &root) != nil {
		return false
	}
	var prompts []json.RawMessage
	if json.Unmarshal(root["prompts"], &prompts) != nil || prompts == nil || len(prompts) > conversationImportPromptMax {
		return false
	}
	for _, prompt := range prompts {
		var object map[string]json.RawMessage
		if json.Unmarshal(prompt, &object) != nil || !exactJSONKeys(object, "role", "content") {
			return false
		}
	}
	return true
}

func (routes conversationRoutes) listConversationPrompts(w http.ResponseWriter, r *http.Request) {
	if requestHasBody(r) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "GET requests must not include a body")
		return
	}
	conversationID, _ := r.Context().Value(conversationIDContextKey{}).(string)
	query, err := parseConversationQuery(r, "limit", "before_created_at_ms", "before_prompt_id")
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	limit, err := parseConversationLimit(query)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	var before *model.PromptPageCursor
	timeValues, hasTime := query["before_created_at_ms"]
	idValues, hasID := query["before_prompt_id"]
	if hasTime != hasID {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "pagination query is invalid")
		return
	}
	if hasTime {
		createdAtMS, parseErr := parseUnsignedDecimal(timeValues[0])
		promptID := idValues[0]
		if parseErr != nil || createdAtMS > maxSafeJSONInteger || strings.TrimSpace(promptID) == "" || len(promptID) > conversationIDMaxBytes {
			writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "pagination query is invalid")
			return
		}
		before = &model.PromptPageCursor{CreatedAtMS: createdAtMS, PromptID: promptID}
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
	page, err := routes.backend.OwnedConversationPrompts(r.Context(), owner, conversationID, before, limit)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if !conversationPromptPageJSONSafe(page, conversationID, before, limit) {
		writeConversationBackendError(w, r, &runtimebridge.Error{Code: "invalid_runtime_response"})
		return
	}
	writeConversationJSON(w, r, http.StatusOK, page)
}

func (routes conversationRoutes) listConversationRuns(w http.ResponseWriter, r *http.Request) {
	if requestHasBody(r) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "GET requests must not include a body")
		return
	}
	conversationID, _ := r.Context().Value(conversationIDContextKey{}).(string)
	query, err := parseConversationQuery(r, "limit", "before_created_at_ms", "before_run_id")
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	limit := 25
	if _, hasLimit := query["limit"]; hasLimit {
		limit, err = parseConversationLimit(query)
		if err != nil || limit > 25 {
			writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "pagination query is invalid")
			return
		}
	}
	var before *runmodel.OwnedRunPageCursor
	timeValues, hasTime := query["before_created_at_ms"]
	idValues, hasID := query["before_run_id"]
	before, ok := parseRunPageCursor(timeValues, idValues, hasTime, hasID)
	if !ok {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "pagination query is invalid")
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
	page, err := routes.backend.OwnedConversationRuns(r.Context(), owner, conversationID, before, limit)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if !conversationRunPageJSONSafe(page, conversationID, before, limit) {
		writeConversationBackendError(w, r, &runtimebridge.Error{Code: "invalid_runtime_response"})
		return
	}
	writeConversationJSON(w, r, http.StatusOK, page)
}

func parseRunPageCursor(
	timeValues, idValues []string,
	hasTime, hasID bool,
) (*runmodel.OwnedRunPageCursor, bool) {
	if hasTime != hasID {
		return nil, false
	}
	if !hasTime {
		return nil, true
	}
	createdAtMS, err := parseUnsignedDecimal(timeValues[0])
	runID := idValues[0]
	if err != nil || createdAtMS > maxSafeJSONInteger || strings.TrimSpace(runID) == "" ||
		len(runID) > conversationIDMaxBytes {
		return nil, false
	}
	return &runmodel.OwnedRunPageCursor{CreatedAtMS: createdAtMS, RunID: runID}, true
}

func (routes conversationRoutes) listConversationRunTimeline(w http.ResponseWriter, r *http.Request) {
	if requestHasBody(r) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "GET requests must not include a body")
		return
	}
	conversationID, _ := r.Context().Value(conversationIDContextKey{}).(string)
	runID, _ := r.Context().Value(runIDContextKey{}).(string)
	query, err := parseConversationQuery(r, "after_sequence", "limit")
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	after := uint64(0)
	if values, ok := query["after_sequence"]; ok {
		after, err = parseUnsignedDecimal(values[0])
		if err != nil || after > maxSafeJSONInteger {
			writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "pagination query is invalid")
			return
		}
	}
	limit, err := parseConversationLimit(query)
	if err != nil {
		writeConversationRequestError(w, r, err)
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
	page, err := routes.backend.OwnedConversationRunTimeline(r.Context(), owner, conversationID, runID, after, limit)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if !conversationRunTimelineJSONSafe(page, conversationID, runID, after, limit) {
		writeConversationBackendError(w, r, &runtimebridge.Error{Code: "invalid_runtime_response"})
		return
	}
	writeConversationJSON(w, r, http.StatusOK, page)
}

func (routes conversationRoutes) appendConversationPrompt(w http.ResponseWriter, r *http.Request) {
	conversationID, _ := r.Context().Value(conversationIDContextKey{}).(string)
	key, err := requestIdempotencyKey(r)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	var request appendPromptRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if !hasExactRequiredFields(body, "content", "expected_version") || strings.TrimSpace(request.Content) == "" || len(request.Content) > promptContentMaxBytes || request.ExpectedVersion > maxSafeJSONInteger {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "prompt request is invalid")
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
	prompt, aggregateVersion, replayed, err := routes.backend.AppendOwnedPrompt(r.Context(), owner, conversationID, request.Content, key, request.ExpectedVersion)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	status := http.StatusCreated
	if replayed {
		status = http.StatusOK
	}
	if !conversationPromptAppendJSONSafe(prompt, aggregateVersion) {
		writeConversationBackendError(w, r, &runtimebridge.Error{Code: "invalid_runtime_response"})
		return
	}
	writeConversationJSON(w, r, status, struct {
		Prompt           model.ConversationPrompt `json:"prompt"`
		AggregateVersion uint64                   `json:"aggregate_version"`
		Replayed         bool                     `json:"replayed"`
	}{Prompt: prompt, AggregateVersion: aggregateVersion, Replayed: replayed})
}
