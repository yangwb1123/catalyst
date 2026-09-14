package appserver

import (
	"context"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"net/http"
	"net/url"
)

const conversationChangesPath = "/api/v1/conversation-changes"

type ownedConversationChangesBackend interface {
	OwnedConversationChangesAfter(
		ctx context.Context,
		owner model.Owner,
		after uint64,
		limit int,
	) (model.OwnedConversationChangePage, error)
}

func (routes conversationRoutes) ownedConversationChanges(w http.ResponseWriter, r *http.Request) {
	if requestHasBody(r) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "GET requests must not include a body")
		return
	}
	query, err := parseConversationQuery(r, "after_cursor", "limit")
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	after, err := parseConversationAfterCursor(query)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
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
	backend, ok := routes.backend.(ownedConversationChangesBackend)
	if !ok {
		writeConversationBackendUnavailable(w, r)
		return
	}
	page, err := backend.OwnedConversationChangesAfter(r.Context(), owner, after, limit)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	writeConversationJSON(w, r, http.StatusOK, page)
}

func parseConversationAfterCursor(query url.Values) (uint64, error) {
	values, ok := query["after_cursor"]
	if !ok {
		return 0, errConversationJSON
	}
	cursor, err := parseUnsignedDecimal(values[0])
	if err != nil || cursor > maxSQLiteCursor {
		return 0, errConversationJSON
	}
	return cursor, nil
}
