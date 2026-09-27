package appserver

import (
	"context"
	"encoding/json"
	"fmt"
	"forgeos/forge-core/internal/runtimebridge"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"net/http"
	"net/url"
	"strconv"
	"time"
)

// conversationChangesStreamPath is a read-only SSE boundary over the same
// owner-filtered change metadata as conversationChangesPath. It deliberately
// has a separate path so existing page consumers keep their exact response
// and cursor semantics.
const conversationChangesStreamPath = conversationChangesPath + "/stream"

const (
	conversationChangesStreamDefaultWait = 15 * time.Second
	conversationChangesStreamMaxWait     = 30 * time.Second
	conversationChangesStreamPollPeriod  = 100 * time.Millisecond
)

func (routes conversationRoutes) ownedConversationChangesStream(w http.ResponseWriter, r *http.Request) {
	if requestHasBody(r) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "GET requests must not include a body")
		return
	}
	query, err := parseConversationQuery(r, "after_cursor", "limit", "wait_ms")
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
	wait, err := parseConversationChangesStreamWait(query)
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

	page, err := routes.readConversationChangesStreamPage(r.Context(), backend, owner, after, limit)
	if err != nil {
		if r.Context().Err() != nil {
			return
		}
		writeConversationBackendError(w, r, err)
		return
	}
	if conversationChangesPageHasRows(page, after) {
		writeConversationChangesSSE(w, r, page)
		return
	}
	if wait == 0 {
		writeConversationChangesStreamEmpty(w, r)
		return
	}

	// The Runtime bridge currently exposes a bounded page read rather than a
	// blocking watcher. Re-read at a fixed, bounded cadence so a Prompt written
	// by another client can wake this request without changing the existing
	// bridge ABI. The request context remains the cancellation authority.
	deadline := time.NewTimer(wait)
	defer deadline.Stop()
	poll := time.NewTicker(conversationChangesStreamPollPeriod)
	defer poll.Stop()
	for {
		select {
		case <-r.Context().Done():
			return
		case <-deadline.C:
			writeConversationChangesStreamEmpty(w, r)
			return
		case <-poll.C:
			page, err = routes.readConversationChangesStreamPage(r.Context(), backend, owner, after, limit)
			if err != nil {
				if r.Context().Err() != nil {
					return
				}
				writeConversationBackendError(w, r, err)
				return
			}
			if conversationChangesPageHasRows(page, after) {
				writeConversationChangesSSE(w, r, page)
				return
			}
		}
	}
}

func (routes conversationRoutes) readConversationChangesStreamPage(
	ctx context.Context,
	backend ownedConversationChangesBackend,
	owner model.Owner,
	after uint64,
	limit int,
) (model.OwnedConversationChangePage, error) {
	page, err := backend.OwnedConversationChangesAfter(ctx, owner, after, limit)
	if err != nil {
		// The handler suppresses the response when the client canceled while the
		// bridge was in flight; preserving the typed error here keeps ordinary
		// backend failures on the same mapping as the replay endpoint.
		return model.OwnedConversationChangePage{}, err
	}
	if !conversationChangePageJSONSafe(page) {
		return model.OwnedConversationChangePage{}, &runtimebridge.Error{Code: "invalid_runtime_response"}
	}
	return page, nil
}

func parseConversationChangesStreamWait(query url.Values) (time.Duration, error) {
	values, ok := query["wait_ms"]
	if !ok {
		return conversationChangesStreamDefaultWait, nil
	}
	value, err := parseUnsignedDecimal(values[0])
	if err != nil || value > uint64(conversationChangesStreamMaxWait/time.Millisecond) {
		return 0, errConversationJSON
	}
	return time.Duration(value) * time.Millisecond, nil
}

func conversationChangesPageHasRows(page model.OwnedConversationChangePage, after uint64) bool {
	return len(page.Changes) > 0 || page.ScannedThroughCursor > after
}

func writeConversationChangesStreamEmpty(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("Content-Length", "0")
	w.Header().Set("Content-Security-Policy", "default-src 'none'; frame-ancestors 'none'")
	w.Header().Set("Content-Type", "text/event-stream; charset=utf-8")
	w.Header().Set("Cross-Origin-Resource-Policy", "same-origin")
	w.Header().Set("Referrer-Policy", "no-referrer")
	w.Header().Set("X-Content-Type-Options", "nosniff")
	w.WriteHeader(http.StatusNoContent)
}

func writeConversationChangesSSE(w http.ResponseWriter, r *http.Request, page model.OwnedConversationChangePage) {
	body, err := json.Marshal(page)
	if err != nil {
		writeConversationError(w, r, http.StatusInternalServerError, "internal_error", "request could not be completed")
		return
	}
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("Content-Security-Policy", "default-src 'none'; frame-ancestors 'none'")
	w.Header().Set("Content-Type", "text/event-stream; charset=utf-8")
	w.Header().Set("Cross-Origin-Resource-Policy", "same-origin")
	w.Header().Set("Referrer-Policy", "no-referrer")
	w.Header().Set("X-Content-Type-Options", "nosniff")
	w.WriteHeader(http.StatusOK)
	_, _ = fmt.Fprintf(w, "event: conversation_changes\nid: %s\ndata: %s\n\n", strconv.FormatUint(page.ScannedThroughCursor, 10), body)
	if flusher, ok := w.(http.Flusher); ok {
		flusher.Flush()
	}
}
