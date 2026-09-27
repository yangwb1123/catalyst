package appserver

import (
	"context"
	"errors"
	"net/http"

	"forgeos/forge-core/internal/runtimebridge"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
)

// verifyOwnedRunReference keeps the accepted Runner preview boundaries tied to
// a durable, owner-scoped Runtime Run. A nil backend is retained for pure
// route/value tests; the production assembly supplies the Runtime bridge.
func verifyOwnedRunReference(
	ctx context.Context,
	backend conversationBackend,
	owner model.Owner,
	conversationID string,
	runID string,
) error {
	if backend == nil {
		return nil
	}
	var before *runmodel.OwnedRunPageCursor
	for pageIndex := 0; pageIndex < runnerExecutionIntentReferenceMaxPages; pageIndex++ {
		page, err := backend.OwnedConversationRuns(ctx, owner, conversationID, before, runnerExecutionIntentRunPageLimit)
		if err != nil {
			return err
		}
		if !conversationRunPageJSONSafe(page, conversationID, before, runnerExecutionIntentRunPageLimit) {
			return &runtimebridge.Error{Code: "invalid_runtime_response"}
		}
		for _, run := range page.Runs {
			if run.RunID == runID {
				return nil
			}
		}
		if !page.HasMore {
			return &runtimebridge.Error{Code: "not_found"}
		}
		if page.NextCursor == nil || (before != nil && *before == *page.NextCursor) {
			return &runtimebridge.Error{Code: "invalid_runtime_response"}
		}
		next := *page.NextCursor
		before = &next
	}
	return &runtimebridge.Error{Code: "invalid_runtime_response"}
}

func writeOwnedRunReferenceError(w http.ResponseWriter, r *http.Request, err error) {
	var bridgeErr *runtimebridge.Error
	if errors.As(err, &bridgeErr) && bridgeErr != nil && bridgeErr.Code == "not_found" {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "conversation or Run reference was not found")
		return
	}
	writeConversationBackendError(w, r, err)
}
