package appserver

import (
	"encoding/json"
	"errors"
	"net/http"
	"net/url"
	"strings"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/executionlease"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// runnerDispatchAdmissionPath is an explicitly mounted, read-only recheck
// between a durable fenced lease and a future Runner transport. It never
// authorizes or dispatches a command.
const runnerDispatchAdmissionPath = "/api/v1/conversations/%s/runs/%s/runner-dispatch-admission/preview"

const runnerDispatchAdmissionScope = "forge:devices:placement:lease"

type runnerDispatchAdmissionConfig struct {
	Enabled      bool
	RegistryPath string
	Now          devicePlacementRegistryCandidateClock
	Backend      conversationBackend
}

func newRunnerDispatchAdmissionRoutes(config *runnerDispatchAdmissionConfig) http.Handler {
	if config == nil || !config.Enabled || config.RegistryPath == "" || config.Now == nil {
		return http.HandlerFunc(serveDisabledRunnerDispatchAdmission)
	}
	return authn.RequireScopes(http.HandlerFunc(runnerDispatchAdmissionHandler{
		registryPath: config.RegistryPath,
		now:          config.Now,
		backend:      config.Backend,
	}.ServeHTTP), runnerDispatchAdmissionScope)
}

func serveDisabledRunnerDispatchAdmission(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, r, http.StatusNotFound, notFoundBody)
}

type runnerDispatchAdmissionHandler struct {
	registryPath string
	now          devicePlacementRegistryCandidateClock
	backend      conversationBackend
}

func (handler runnerDispatchAdmissionHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	conversationID, runID, ok := runnerDispatchAdmissionPathIDs(r.URL.EscapedPath())
	if !ok {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return
	}
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "Runner dispatch admission does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	var request deviceplacement.RunnerDispatchAdmissionRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if !hasExactRequiredFields(body, "owner", "conversation_id", "run_id", "attempt_id", "attempt_state", "command", "evaluated_at_ms") {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Runner dispatch admission request is invalid")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	declaredOwner := model.Owner{Issuer: request.Owner.Issuer, Subject: request.Owner.Subject, TenantID: request.Owner.TenantID}
	if !validRunnerDispatchAdmissionOwner(declaredOwner, owner) {
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "Runner dispatch admission owner must match the authenticated principal")
		return
	}
	if request.ConversationID != conversationID || request.RunID != runID ||
		request.Command.LeaseProof.AttemptID != request.AttemptID {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_binding", "Runner dispatch admission identities must match the path")
		return
	}
	if err := verifyOwnedRunReference(r.Context(), handler.backend, owner, conversationID, runID); err != nil {
		writeOwnedRunReferenceError(w, r, err)
		return
	}
	evaluatedAtMS, err := handler.now(r.Context())
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if evaluatedAtMS <= 0 || evaluatedAtMS > deviceplacement.MaxSafeIntegerMS {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "Runner dispatch admission clock response is invalid")
		return
	}
	// The caller cannot choose the evaluation time. This keeps the response
	// tied to the same Coordinator clock that checks the durable lease.
	request.EvaluatedAtMS = uint64(evaluatedAtMS)
	registry, err := deviceinventory.NewPersistedExecutionLeaseRegistryFileSetReadAdapter(
		handler.registryPath,
		deviceidentity.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
	)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	entry, err := registry.Lookup(r.Context(), conversationID, runID, request.AttemptID, executionlease.LeaseProof{
		AttemptID:    request.Command.LeaseProof.AttemptID,
		TargetID:     request.Command.LeaseProof.TargetID,
		Epoch:        request.Command.LeaseProof.Epoch,
		FencingToken: request.Command.LeaseProof.FencingToken,
	})
	if err != nil {
		runnerDispatchAdmissionError(w, r, err)
		return
	}
	observation, err := deviceplacement.ObserveRunnerDispatchAdmission(request, deviceplacement.RunnerDispatchAdmissionLease{
		TargetID: entry.InstanceID, Epoch: entry.Grant.Epoch,
		IssuedAtMS: entry.Grant.IssuedAtMS, ExpiresAtMS: entry.Grant.ExpiresAtMS,
		Current: true, Active: entry.IsActive(uint64(evaluatedAtMS)),
	})
	if err != nil || observation.Validate() != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Runner dispatch admission request is invalid")
		return
	}
	encoded, err := json.Marshal(observation)
	if err != nil {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "Runner dispatch admission response is invalid")
		return
	}
	writeJSON(w, r, http.StatusOK, append(encoded, '\n'))
}

func runnerDispatchAdmissionPathIDs(path string) (string, string, bool) {
	conversationID, suffix, ok := conversationPathSuffix(path)
	if !ok {
		return "", "", false
	}
	parts := strings.Split(suffix, "/")
	if len(parts) != 4 || parts[0] != "runs" || parts[1] == "" ||
		parts[2] != "runner-dispatch-admission" || parts[3] != "preview" {
		return "", "", false
	}
	runID, err := url.PathUnescape(parts[1])
	if err != nil || !validExecutionRouteID(runID) {
		return "", "", false
	}
	return conversationID, runID, true
}

func validRunnerDispatchAdmissionOwner(value model.Owner, owner model.Owner) bool {
	return value == owner
}

func runnerDispatchAdmissionError(w http.ResponseWriter, r *http.Request, err error) {
	switch {
	case errors.Is(err, executionlease.ErrLeaseStale):
		writeConversationError(w, r, http.StatusConflict, "lease_stale", "Runner dispatch admission proof is stale")
	case errors.Is(err, executionlease.ErrLeaseNotFound):
		writeConversationError(w, r, http.StatusConflict, "lease_not_found", "Runner dispatch admission proof is not registered")
	default:
		writeConversationBackendError(w, r, err)
	}
}
