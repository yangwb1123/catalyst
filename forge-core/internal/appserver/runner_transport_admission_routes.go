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

// runnerTransportAdmissionPath is a metadata-only join between a caller's
// already verified D3 transport observation and the current durable lease.
// It never opens a Runner connection or sends the transport payload.
const runnerTransportAdmissionPath = "/api/v1/conversations/%s/runs/%s/runner-transport-admission/preview"

const runnerTransportAdmissionScope = "forge:devices:placement:lease"

type runnerTransportAdmissionConfig struct {
	Enabled      bool
	RegistryPath string
	Now          devicePlacementRegistryCandidateClock
	Backend      conversationBackend
}

func newRunnerTransportAdmissionRoutes(config *runnerTransportAdmissionConfig) http.Handler {
	if config == nil || !config.Enabled || config.RegistryPath == "" || config.Now == nil {
		return http.HandlerFunc(serveDisabledRunnerTransportAdmission)
	}
	return authn.RequireScopes(http.HandlerFunc(runnerTransportAdmissionHandler{
		registryPath: config.RegistryPath,
		now:          config.Now,
		backend:      config.Backend,
	}.ServeHTTP), runnerTransportAdmissionScope)
}

func serveDisabledRunnerTransportAdmission(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, r, http.StatusNotFound, notFoundBody)
}

type runnerTransportAdmissionHandler struct {
	registryPath string
	now          devicePlacementRegistryCandidateClock
	backend      conversationBackend
}

func (handler runnerTransportAdmissionHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	conversationID, runID, ok := runnerTransportAdmissionPathIDs(r.URL.EscapedPath())
	if !ok {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return
	}
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "Runner transport admission does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	var request deviceplacement.RunnerTransportAdmissionRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if !hasExactRequiredFields(body,
		"owner", "conversation_id", "run_id", "attempt_id", "attempt_state",
		"command", "lease", "transport", "expected_payload_sha256", "evaluated_at_ms",
	) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Runner transport admission request is invalid")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	declaredOwner := model.Owner{Issuer: request.Owner.Issuer, Subject: request.Owner.Subject, TenantID: request.Owner.TenantID}
	if declaredOwner != owner {
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "Runner transport admission owner must match the authenticated principal")
		return
	}
	if request.ConversationID != conversationID || request.RunID != runID ||
		request.AttemptID != request.Command.LeaseProof.AttemptID {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_binding", "Runner transport admission identities must match the path")
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
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "Runner transport admission clock response is invalid")
		return
	}
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
		runnerTransportAdmissionError(w, r, err)
		return
	}
	if request.Lease.TargetID != entry.InstanceID || request.Lease.Epoch != entry.Grant.Epoch ||
		request.Lease.IssuedAtMS != entry.Grant.IssuedAtMS || request.Lease.ExpiresAtMS != entry.Grant.ExpiresAtMS {
		writeConversationError(w, r, http.StatusConflict, "lease_stale", "Runner transport admission lease observation is stale")
		return
	}
	request.EvaluatedAtMS = uint64(evaluatedAtMS)
	request.Lease = deviceplacement.RunnerDispatchAdmissionLease{
		TargetID: entry.InstanceID, Epoch: entry.Grant.Epoch,
		IssuedAtMS: entry.Grant.IssuedAtMS, ExpiresAtMS: entry.Grant.ExpiresAtMS,
		Current: true, Active: entry.IsActive(uint64(evaluatedAtMS)),
	}
	observation, err := deviceplacement.ObserveRunnerTransportAdmission(request)
	if err != nil || observation.Validate() != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "Runner transport admission request is invalid")
		return
	}
	encoded, err := json.Marshal(observation)
	if err != nil {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "Runner transport admission response is invalid")
		return
	}
	writeJSON(w, r, http.StatusOK, append(encoded, '\n'))
}

func runnerTransportAdmissionPathIDs(path string) (string, string, bool) {
	conversationID, suffix, ok := conversationPathSuffix(path)
	if !ok {
		return "", "", false
	}
	parts := strings.Split(suffix, "/")
	if len(parts) != 4 || parts[0] != "runs" || parts[1] == "" ||
		parts[2] != "runner-transport-admission" || parts[3] != "preview" {
		return "", "", false
	}
	runID, err := url.PathUnescape(parts[1])
	if err != nil || !validExecutionRouteID(runID) {
		return "", "", false
	}
	return conversationID, runID, true
}

func runnerTransportAdmissionError(w http.ResponseWriter, r *http.Request, err error) {
	switch {
	case errors.Is(err, executionlease.ErrLeaseStale):
		writeConversationError(w, r, http.StatusConflict, "lease_stale", "Runner transport admission proof is stale")
	case errors.Is(err, executionlease.ErrLeaseNotFound):
		writeConversationError(w, r, http.StatusConflict, "lease_not_found", "Runner transport admission proof is not registered")
	default:
		writeConversationBackendError(w, r, err)
	}
}
