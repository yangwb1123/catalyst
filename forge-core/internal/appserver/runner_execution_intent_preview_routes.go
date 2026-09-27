package appserver

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/url"
	"strings"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/runtimebridge"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
)

// runnerExecutionIntentPreviewScope is deliberately the existing conversation
// read scope. This route joins caller-supplied Prompt, Run, and Runner command
// declarations only; it does not persist a command, inspect a lease, select a
// target, or dispatch work.
const runnerExecutionIntentPreviewScope = "forge:conversations:read"

func newRunnerExecutionIntentPreviewRoutes() http.Handler {
	return newRunnerExecutionIntentPreviewRoutesWithBackend(nil)
}

// newRunnerExecutionIntentPreviewRoutesWithBackend keeps the value-only
// candidate available to focused contract tests while allowing the accepted
// Coordinator assembly to bind the request to owner-scoped durable session
// references. The binding is read-only and does not turn this preview into a
// Run, lease, or Runner effect.
func newRunnerExecutionIntentPreviewRoutesWithBackend(backend conversationBackend) http.Handler {
	return authn.RequireScopes(
		http.HandlerFunc(runnerExecutionIntentPreviewHandler{backend: backend}.ServeHTTP),
		runnerExecutionIntentPreviewScope,
	)
}

type runnerExecutionIntentPreviewHandler struct {
	backend conversationBackend
}

func (handler runnerExecutionIntentPreviewHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	serveRunnerExecutionIntentPreviewWithBackend(w, r, handler.backend)
}

func serveRunnerExecutionIntentPreview(w http.ResponseWriter, r *http.Request) {
	serveRunnerExecutionIntentPreviewWithBackend(w, r, nil)
}

func serveRunnerExecutionIntentPreviewWithBackend(w http.ResponseWriter, r *http.Request, backend conversationBackend) {
	conversationID, runID, ok := runnerExecutionIntentPreviewPathIDs(r.URL.EscapedPath())
	if !ok {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return
	}
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "Runner execution-intent preview does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	var request deviceplacement.RunnerExecutionIntentRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if !hasExactRequiredFields(body, "owner", "conversation_id", "prompt_receipt", "run_reference", "execution_intent", "command") {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Runner execution-intent preview request is invalid")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if !sameRunnerExecutionIntentOwner(request.Owner, owner) {
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "Runner execution-intent preview owner must match the authenticated principal")
		return
	}
	if request.ConversationID != conversationID || request.Run.ConversationID != conversationID ||
		request.Run.RunID != runID || request.Binding.ConversationID != conversationID ||
		request.Binding.RunID != runID {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_binding", "Runner execution-intent preview identities must match the path")
		return
	}
	if err := verifyRunnerExecutionIntentReferences(r.Context(), backend, owner, request); err != nil {
		writeRunnerExecutionIntentReferenceError(w, r, err)
		return
	}
	observation, err := deviceplacement.ObserveRunnerExecutionIntent(request)
	if err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Runner execution-intent preview request is invalid")
		return
	}
	if !validRunnerExecutionIntentObservation(observation, owner, conversationID, runID) {
		writeConversationError(w, r, http.StatusBadGateway, "runner_execution_intent_preview_invalid", "Runner execution-intent preview result is invalid")
		return
	}
	encoded, err := json.Marshal(observation)
	if err != nil {
		writeConversationError(w, r, http.StatusInternalServerError, "internal_error", "Runner execution-intent preview could not be encoded")
		return
	}
	writeJSON(w, r, http.StatusOK, append(encoded, '\n'))
}

const (
	runnerExecutionIntentPromptPageLimit   = 128
	runnerExecutionIntentRunPageLimit      = 25
	runnerExecutionIntentReferenceMaxPages = 64
)

// verifyRunnerExecutionIntentReferences binds the request to the existing
// owner-scoped Prompt and Run projections. It deliberately compares identity
// only: timestamps, status, and command/lease fields remain part of the
// value-level preview contract and are not silently reconstructed here.
func verifyRunnerExecutionIntentReferences(
	ctx context.Context,
	backend conversationBackend,
	owner model.Owner,
	request deviceplacement.RunnerExecutionIntentRequest,
) error {
	if backend == nil {
		return nil
	}
	promptExists, err := runnerExecutionIntentPromptExists(ctx, backend, owner, request)
	if err != nil {
		return err
	}
	if !promptExists {
		return &runtimebridge.Error{Code: "not_found"}
	}
	runExists, err := runnerExecutionIntentRunExists(ctx, backend, owner, request)
	if err != nil {
		return err
	}
	if !runExists {
		return &runtimebridge.Error{Code: "not_found"}
	}
	return nil
}

func runnerExecutionIntentPromptExists(
	ctx context.Context,
	backend conversationBackend,
	owner model.Owner,
	request deviceplacement.RunnerExecutionIntentRequest,
) (bool, error) {
	var before *model.PromptPageCursor
	for pageIndex := 0; pageIndex < runnerExecutionIntentReferenceMaxPages; pageIndex++ {
		page, err := backend.OwnedConversationPrompts(ctx, owner, request.ConversationID, before, runnerExecutionIntentPromptPageLimit)
		if err != nil {
			return false, err
		}
		if !conversationPromptPageJSONSafe(page, request.ConversationID, before, runnerExecutionIntentPromptPageLimit) {
			return false, &runtimebridge.Error{Code: "invalid_runtime_response"}
		}
		for _, prompt := range page.Prompts {
			if prompt.ID == request.Prompt.PromptID && prompt.ConversationID == request.ConversationID {
				return true, nil
			}
		}
		if !page.HasMore {
			return false, nil
		}
		if page.NextCursor == nil || (before != nil && *before == *page.NextCursor) {
			return false, &runtimebridge.Error{Code: "invalid_runtime_response"}
		}
		next := *page.NextCursor
		before = &next
	}
	return false, &runtimebridge.Error{Code: "invalid_runtime_response"}
}

func runnerExecutionIntentRunExists(
	ctx context.Context,
	backend conversationBackend,
	owner model.Owner,
	request deviceplacement.RunnerExecutionIntentRequest,
) (bool, error) {
	var before *runmodel.OwnedRunPageCursor
	for pageIndex := 0; pageIndex < runnerExecutionIntentReferenceMaxPages; pageIndex++ {
		page, err := backend.OwnedConversationRuns(ctx, owner, request.ConversationID, before, runnerExecutionIntentRunPageLimit)
		if err != nil {
			return false, err
		}
		if !conversationRunPageJSONSafe(page, request.ConversationID, before, runnerExecutionIntentRunPageLimit) {
			return false, &runtimebridge.Error{Code: "invalid_runtime_response"}
		}
		for _, run := range page.Runs {
			if run.RunID == request.Run.RunID && run.PromptID == request.Prompt.PromptID {
				return true, nil
			}
		}
		if !page.HasMore {
			return false, nil
		}
		if page.NextCursor == nil || (before != nil && *before == *page.NextCursor) {
			return false, &runtimebridge.Error{Code: "invalid_runtime_response"}
		}
		next := *page.NextCursor
		before = &next
	}
	return false, &runtimebridge.Error{Code: "invalid_runtime_response"}
}

func writeRunnerExecutionIntentReferenceError(w http.ResponseWriter, r *http.Request, err error) {
	var bridgeErr *runtimebridge.Error
	if errors.As(err, &bridgeErr) && bridgeErr != nil && bridgeErr.Code == "not_found" {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "conversation or execution-intent reference was not found")
		return
	}
	writeConversationBackendError(w, r, err)
}

func validRunnerExecutionIntentObservation(
	value deviceplacement.RunnerExecutionIntentObservation,
	owner model.Owner,
	conversationID string,
	runID string,
) bool {
	return sameRunnerExecutionIntentOwner(value.Owner, owner) &&
		value.SchemaVersion == deviceplacement.RunnerExecutionIntentSchemaVersion &&
		value.EvaluationMode == deviceplacement.RunnerExecutionIntentEvaluationMode &&
		value.ConversationID == conversationID && value.RunID == runID &&
		value.PromptRunBindingValid && value.RunnerCommandBindingValid && value.PreviewOnly &&
		value.SelectedTargetID == nil &&
		value.Authority == (deviceplacement.RunnerExecutionIntentAuthority{})
}

func sameRunnerExecutionIntentOwner(value deviceplacement.Owner, owner model.Owner) bool {
	return value.Issuer == owner.Issuer && value.Subject == owner.Subject && value.TenantID == owner.TenantID
}

func runnerExecutionIntentPreviewPathIDs(path string) (string, string, bool) {
	conversationID, suffix, ok := conversationPathSuffix(path)
	if !ok {
		return "", "", false
	}
	parts := strings.Split(suffix, "/")
	if len(parts) != 4 || parts[0] != "runs" || parts[1] == "" ||
		parts[2] != "runner-execution-intent" || parts[3] != "preview" {
		return "", "", false
	}
	runID, err := url.PathUnescape(parts[1])
	if err != nil || !validExecutionRouteID(runID) {
		return "", "", false
	}
	return conversationID, runID, true
}
