package appserver

import (
	"encoding/json"
	"errors"
	"net/http"
	"net/url"
	"strings"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/executionlease"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// runnerAttemptBoundaryPath is an owner-bound, metadata-only lifecycle
// projection after execution-boundary preview. It never persists an Attempt
// or dispatches a Runner command.
const runnerAttemptBoundaryPath = "/api/v1/conversations/%s/runs/%s/runner-attempt-boundary/preview"

type runnerAttemptBoundaryConfig struct {
	Enabled      bool
	RegistryPath string
	Now          devicePlacementRegistryCandidateClock
	Activation   devicefabricgate.Request
	Authority    devicefabricgate.RunnerAuthorityConfig
	Backend      conversationBackend
}

func newRunnerAttemptBoundaryRoutes(config *runnerAttemptBoundaryConfig) http.Handler {
	if config == nil || !config.Enabled || config.RegistryPath == "" || config.Now == nil {
		return http.HandlerFunc(serveDisabledRunnerAttemptBoundary)
	}
	gate := devicefabricgate.EvaluateRunnerExecution(devicefabricgate.RunnerExecutionGateRequest{
		Activation: config.Activation,
		Authority:  config.Authority,
	})
	if !gate.Allowed {
		return http.HandlerFunc(serveDisabledRunnerAttemptBoundary)
	}
	return authn.RequireScopes(http.HandlerFunc(runnerAttemptBoundaryHandler{
		registryPath: config.RegistryPath,
		now:          config.Now,
		activation:   config.Activation,
		authority:    config.Authority,
		backend:      config.Backend,
	}.ServeHTTP), runnerTransportAdmissionScope)
}

func serveDisabledRunnerAttemptBoundary(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, r, http.StatusNotFound, notFoundBody)
}

type runnerAttemptBoundaryHandler struct {
	registryPath string
	now          devicePlacementRegistryCandidateClock
	activation   devicefabricgate.Request
	authority    devicefabricgate.RunnerAuthorityConfig
	backend      conversationBackend
}

func (handler runnerAttemptBoundaryHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	conversationID, runID, ok := runnerAttemptBoundaryPathIDs(r.URL.EscapedPath())
	if !ok {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return
	}
	if !validateRunnerAttemptBoundaryMethod(w, r) {
		return
	}
	request, ok := readRunnerAttemptBoundaryRequest(w, r)
	if !ok || !bindRunnerAttemptBoundaryRequest(w, r, request, conversationID, runID) {
		return
	}
	owner := model.Owner{Issuer: request.Owner.Issuer, Subject: request.Owner.Subject, TenantID: request.Owner.TenantID}
	if err := verifyOwnedRunReference(r.Context(), handler.backend, owner, conversationID, runID); err != nil {
		writeOwnedRunReferenceError(w, r, err)
		return
	}
	observation, err := handler.observe(r, request, conversationID, runID)
	if err != nil {
		runnerAttemptBoundaryError(w, r, err)
		return
	}
	if observation.Validate() != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Runner Attempt boundary response is invalid")
		return
	}
	encoded, err := json.Marshal(observation)
	if err != nil {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "Runner Attempt boundary response is invalid")
		return
	}
	writeJSON(w, r, http.StatusOK, append(encoded, '\n'))
}

func validateRunnerAttemptBoundaryMethod(w http.ResponseWriter, r *http.Request) bool {
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "Runner Attempt boundary does not accept query parameters")
		return false
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return false
	}
	return true
}

func readRunnerAttemptBoundaryRequest(w http.ResponseWriter, r *http.Request) (deviceplacement.RunnerAttemptBoundaryPreviewRequest, bool) {
	var request deviceplacement.RunnerAttemptBoundaryPreviewRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return deviceplacement.RunnerAttemptBoundaryPreviewRequest{}, false
	}
	if !hasExactRequiredFields(body,
		"owner", "conversation_id", "run_id", "attempt_id", "attempt_state",
		"command", "transport", "expected_payload_sha256", "controls", "transition",
	) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Runner Attempt boundary request is invalid")
		return deviceplacement.RunnerAttemptBoundaryPreviewRequest{}, false
	}
	return request, true
}

func bindRunnerAttemptBoundaryRequest(w http.ResponseWriter, r *http.Request, request deviceplacement.RunnerAttemptBoundaryPreviewRequest, conversationID, runID string) bool {
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return false
	}
	declaredOwner := model.Owner{Issuer: request.Owner.Issuer, Subject: request.Owner.Subject, TenantID: request.Owner.TenantID}
	if declaredOwner != owner {
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "Runner Attempt boundary owner must match the authenticated principal")
		return false
	}
	if request.ConversationID != conversationID || request.RunID != runID || request.AttemptID != request.Command.LeaseProof.AttemptID {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_binding", "Runner Attempt boundary identities must match the path")
		return false
	}
	return true
}

func (handler runnerAttemptBoundaryHandler) observe(r *http.Request, request deviceplacement.RunnerAttemptBoundaryPreviewRequest, conversationID, runID string) (deviceplacement.RunnerAttemptBoundaryObservation, error) {
	evaluatedAtMS, err := handler.now(r.Context())
	if err != nil {
		return deviceplacement.RunnerAttemptBoundaryObservation{}, err
	}
	if evaluatedAtMS <= 0 || evaluatedAtMS > deviceplacement.MaxSafeIntegerMS {
		return deviceplacement.RunnerAttemptBoundaryObservation{}, errInvalidRunnerAttemptBoundaryClock
	}
	owner := request.Owner
	registry, err := deviceinventory.NewPersistedExecutionLeaseRegistryFileSetReadAdapter(
		handler.registryPath, deviceidentity.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
	)
	if err != nil {
		return deviceplacement.RunnerAttemptBoundaryObservation{}, err
	}
	entry, err := registry.Lookup(r.Context(), conversationID, runID, request.AttemptID, executionlease.LeaseProof{
		AttemptID: request.Command.LeaseProof.AttemptID, TargetID: request.Command.LeaseProof.TargetID,
		Epoch: request.Command.LeaseProof.Epoch, FencingToken: request.Command.LeaseProof.FencingToken,
	})
	if err != nil {
		return deviceplacement.RunnerAttemptBoundaryObservation{}, err
	}
	boundary, err := deviceplacement.ObserveRunnerExecutionBoundaryPreview(request.RunnerExecutionBoundaryPreviewRequest,
		deviceplacement.RunnerDispatchAdmissionLease{TargetID: entry.InstanceID, Epoch: entry.Grant.Epoch,
			IssuedAtMS: entry.Grant.IssuedAtMS, ExpiresAtMS: entry.Grant.ExpiresAtMS,
			Current: true, Active: entry.IsActive(uint64(evaluatedAtMS))},
		handler.activation, handler.authority, uint64(evaluatedAtMS))
	if err != nil {
		return deviceplacement.RunnerAttemptBoundaryObservation{}, errInvalidRunnerAttemptBoundaryRequest
	}
	observation, err := deviceplacement.ObserveRunnerAttemptBoundary(deviceplacement.RunnerAttemptBoundaryRequest{
		Boundary: boundary, Transition: request.Transition,
	})
	if err != nil {
		return deviceplacement.RunnerAttemptBoundaryObservation{}, errInvalidRunnerAttemptBoundaryRequest
	}
	return observation, nil
}

var (
	errInvalidRunnerAttemptBoundaryClock   = errors.New("Runner Attempt boundary clock response is invalid")
	errInvalidRunnerAttemptBoundaryRequest = errors.New("Runner Attempt boundary request is invalid")
)

func runnerAttemptBoundaryPathIDs(path string) (string, string, bool) {
	conversationID, suffix, ok := conversationPathSuffix(path)
	if !ok {
		return "", "", false
	}
	parts := strings.Split(suffix, "/")
	if len(parts) != 4 || parts[0] != "runs" || parts[1] == "" ||
		parts[2] != "runner-attempt-boundary" || parts[3] != "preview" {
		return "", "", false
	}
	runID, err := url.PathUnescape(parts[1])
	if err != nil || !validExecutionRouteID(runID) {
		return "", "", false
	}
	return conversationID, runID, true
}

func runnerAttemptBoundaryError(w http.ResponseWriter, r *http.Request, err error) {
	switch {
	case errors.Is(err, errInvalidRunnerAttemptBoundaryClock):
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "Runner Attempt boundary clock response is invalid")
	case errors.Is(err, errInvalidRunnerAttemptBoundaryRequest):
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Runner Attempt boundary request is invalid")
	case errors.Is(err, executionlease.ErrLeaseStale):
		writeConversationError(w, r, http.StatusConflict, "lease_stale", "Runner Attempt boundary proof is stale")
	case errors.Is(err, executionlease.ErrLeaseNotFound):
		writeConversationError(w, r, http.StatusConflict, "lease_not_found", "Runner Attempt boundary proof is not registered")
	default:
		writeConversationBackendError(w, r, err)
	}
}
