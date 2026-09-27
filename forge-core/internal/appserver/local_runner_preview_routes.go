package appserver

import (
	"net/http"
	"net/url"
	"strings"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/executionprofile"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// localRunnerPreviewScope is intentionally a read scope. The candidate only
// returns metadata from an explicitly injected local Runner adapter; it does
// not create a Run, reserve a device, or grant execution authority.
const localRunnerPreviewScope = "forge:conversations:read"

// localRunnerPreviewCandidateConfig is private and test-only. Production
// session routes leave this candidate unmounted while execution governance is
// still proposed.
type localRunnerPreviewCandidateConfig struct {
	Enabled bool
	Adapter deviceplacement.LocalRunnerPreviewAdapter
}

func newLocalRunnerPreviewCandidateRoutes(config *localRunnerPreviewCandidateConfig) http.Handler {
	if config == nil || !config.Enabled || config.Adapter.Executor == nil {
		return http.HandlerFunc(serveDisabledLocalRunnerPreviewCandidate)
	}
	return authn.RequireScopes(
		http.HandlerFunc(localRunnerPreviewCandidateHandler{adapter: config.Adapter}.ServeHTTP),
		localRunnerPreviewScope,
	)
}

func serveDisabledLocalRunnerPreviewCandidate(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, r, http.StatusNotFound, notFoundBody)
}

type localRunnerPreviewCandidateHandler struct {
	adapter deviceplacement.LocalRunnerPreviewAdapter
}

func (handler localRunnerPreviewCandidateHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	conversationID, intentID, ok := localRunnerPreviewPathIDs(r.URL.EscapedPath())
	if !ok {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return
	}
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "local Runner preview does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	var request deviceplacement.LocalRunnerPreviewRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if !hasExactRequiredFields(body, "intent", "grant", "observed_at_ms") {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "local Runner preview request is invalid")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	declaredOwner := request.Intent.Owner
	if declaredOwner.Issuer != owner.Issuer || declaredOwner.Subject != owner.Subject || declaredOwner.TenantID != owner.TenantID {
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "local Runner preview owner must match the authenticated principal")
		return
	}
	if request.Intent.ConversationID != conversationID || request.Intent.Prompt.IntentID != intentID {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_binding", "local Runner preview identities must match the path")
		return
	}
	observation, err := handler.adapter.Execute(r.Context(), request)
	if err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "local Runner preview request is invalid")
		return
	}
	if !validLocalRunnerPreviewObservation(observation, owner, conversationID, request) {
		writeConversationError(w, r, http.StatusBadGateway, "local_runner_preview_invalid", "local Runner preview result is invalid")
		return
	}
	writeConversationJSON(w, r, http.StatusOK, observation)
}

func validLocalRunnerPreviewObservation(
	value deviceplacement.LocalRunnerPreviewObservation,
	owner model.Owner,
	conversationID string,
	request deviceplacement.LocalRunnerPreviewRequest,
) bool {
	intent := request.Intent
	receipt := value.SessionReceipt.ReceiptObservation
	return value.Validate() == nil &&
		value.Intent.Owner.Issuer == owner.Issuer &&
		value.Intent.Owner.Subject == owner.Subject &&
		value.Intent.Owner.TenantID == owner.TenantID &&
		value.Intent.ConversationID == conversationID &&
		value.Intent.Owner == intent.Owner &&
		value.Intent.ConversationID == intent.ConversationID &&
		value.Intent.PromptID == intent.Prompt.PromptID &&
		value.Intent.RunID == intent.Run.RunID &&
		value.Intent.AttemptID == intent.Binding.AttemptID &&
		value.Intent.CommandID == intent.Binding.CommandID &&
		value.Intent.TargetID == intent.Binding.TargetID &&
		value.Intent.CommandSHA256 == intent.Binding.CommandSHA256 &&
		value.Intent.IdempotencyKey == intent.Binding.IdempotencyKey &&
		value.CommandID == intent.Binding.CommandID &&
		value.AttemptID == intent.Binding.AttemptID &&
		value.TargetID == intent.Binding.TargetID &&
		value.CommandSHA256 == intent.Binding.CommandSHA256 &&
		value.ObservedAtMS == request.ObservedAtMS &&
		value.SessionReceipt.Owner == intent.Owner &&
		value.SessionReceipt.ConversationID == conversationID &&
		value.SessionReceipt.PromptID == intent.Prompt.PromptID &&
		value.SessionReceipt.RunID == intent.Run.RunID &&
		receipt.CommandID == intent.Binding.CommandID &&
		receipt.AttemptID == intent.Binding.AttemptID &&
		receipt.TargetID == intent.Binding.TargetID &&
		receipt.CommandSHA256 == intent.Binding.CommandSHA256 &&
		receipt.ObservedAtMS == request.ObservedAtMS &&
		receipt.DispositionKind == value.DispositionKind
}

// localRunnerPreviewPathIDs accepts only the private test candidate path:
// /api/v1/conversations/{conversation_id}/run-intents/{intent_id}/execution-readiness-preview
func localRunnerPreviewPathIDs(path string) (string, string, bool) {
	conversationID, suffix, ok := conversationPathSuffix(path)
	if !ok {
		return "", "", false
	}
	parts := strings.Split(suffix, "/")
	if len(parts) != 3 || parts[0] != "run-intents" || parts[1] == "" || parts[2] != "execution-readiness-preview" {
		return "", "", false
	}
	intentID, err := url.PathUnescape(parts[1])
	if err != nil || !validExecutionRouteID(intentID) {
		return "", "", false
	}
	return conversationID, intentID, true
}

// newConversationRoutesWithInertExecutionAPIAndLocalRunnerPreview is a
// focused-test constructor. It composes the existing inert intent surface
// with the injected local preview and is deliberately not called by Run.
func newConversationRoutesWithInertExecutionAPIAndLocalRunnerPreview(
	backend conversationBackend,
	profiles *executionprofile.Catalog,
	config *localRunnerPreviewCandidateConfig,
) http.Handler {
	sessions := newConversationRoutesWithBackendAndExecutionProfiles(backend, profiles)
	return executionSurface{
		sessions: authenticatedSessionRoutes{
			conversations:                    sessions,
			placement:                        newDevicePlacementPreviewRoutes(),
			deviceObservation:                newSessionDeviceObservationRoutes(),
			sessionRunnerReceipt:             newSessionRunnerReceiptObservationRoutes(),
			sessionRunnerReceiptHistory:      newSessionRunnerReceiptHistoryRoutes(),
			sessionRunnerReconciliation:      newSessionRunnerReconciliationProjectionRoutes(),
			localRunnerPreview:               newLocalRunnerPreviewCandidateRoutes(config),
			runAttemptLeaseDispatchPreflight: newRunAttemptLeaseDispatchPreflightRoutes(),
			runnerDispatchPlanPreview:        newRunnerDispatchPlanPreviewRoutes(),
			runExecutionEvidence:             newRunExecutionEvidencePreviewRoutes(),
			executionReconciliation:          newExecutionReconciliationPreviewRoutes(),
		},
		execution: newExecutionRoutes(backend, profiles),
	}
}
