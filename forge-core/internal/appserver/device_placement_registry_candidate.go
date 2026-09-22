package appserver

import (
	"context"
	"encoding/json"
	"net/http"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// devicePlacementRegistryCandidatePath is an explicitly injected candidate
// surface over the persisted lifecycle-registry observation. It is separate
// from devicePlacementPreviewPath: that route compares caller-supplied device
// declarations, while this route reads an owner-bound v2 observation source.
// The ordinary Coordinator constructor never mounts this path; the accepted
// device-fabric activation assembly may mount it as a display-only preflight.
const devicePlacementRegistryCandidatePath = "/api/v1/device-placement/registry-preview"

// devicePlacementRegistryCandidateScope is kept separate from conversation
// read access because the source returns device-shaped observations. It grants
// no inventory authority, selection, reservation, scheduling, or execution.
const devicePlacementRegistryCandidateScope = "forge:devices:placement:preview"

// devicePlacementRegistryCandidateClock is injected so the evaluator never
// reads wall time. The authenticated owner and this clock are the only values
// supplied by the HTTP boundary; the request body contains requirements only.
type devicePlacementRegistryCandidateClock func(context.Context) (int64, error)

type devicePlacementRegistryCandidateConfig struct {
	Enabled bool
	Source  deviceInventoryReadV2Source
	Now     devicePlacementRegistryCandidateClock
}

type devicePlacementRegistryCandidateRequest struct {
	Requirements deviceplacement.Requirements `json:"requirements"`
}

// newDevicePlacementRegistryCandidateRoutes constructs the candidate only for
// focused tests and explicitly injected migration fixtures. A missing,
// disabled, source-less, or clock-less config has the same 404 surface as an
// unregistered production route.
func newDevicePlacementRegistryCandidateRoutes(config *devicePlacementRegistryCandidateConfig) http.Handler {
	if config == nil || !config.Enabled || config.Source == nil || config.Now == nil {
		return http.HandlerFunc(serveDisabledDevicePlacementRegistryCandidate)
	}
	return authn.RequireScopes(
		http.HandlerFunc(devicePlacementRegistryCandidateHandler{
			source: config.Source,
			now:    config.Now,
		}.ServeHTTP),
		devicePlacementRegistryCandidateScope,
	)
}

func serveDisabledDevicePlacementRegistryCandidate(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, r, http.StatusNotFound, notFoundBody)
}

type devicePlacementRegistryCandidateHandler struct {
	source deviceInventoryReadV2Source
	now    devicePlacementRegistryCandidateClock
}

func (handler devicePlacementRegistryCandidateHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if r.URL.EscapedPath() != devicePlacementRegistryCandidatePath {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return
	}
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "registry placement preview does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	var request devicePlacementRegistryCandidateRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if !hasExactRequiredFields(body, "requirements") {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "registry placement preview request is invalid")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if handler.source == nil || handler.now == nil {
		writeConversationBackendUnavailable(w, r)
		return
	}
	evaluatedAtMS, err := handler.now(r.Context())
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if evaluatedAtMS <= 0 || evaluatedAtMS > deviceplacement.MaxSafeIntegerMS {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "registry placement preview clock response is invalid")
		return
	}
	observation, err := handler.source.ReadOwnedDeviceInventoryV2(r.Context(), owner)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if !validDeviceInventoryReadCandidateV2(observation, owner) {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "registry placement preview source response is invalid")
		return
	}
	result, err := deviceplacement.EvaluatePersistedInventoryObservationV2(
		observation,
		deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		request.Requirements,
		evaluatedAtMS,
	)
	if err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "registry placement preview request is invalid")
		return
	}
	if !validDevicePlacementRegistryCandidateResult(result, owner, evaluatedAtMS) {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "registry placement preview result is invalid")
		return
	}
	body, err = json.Marshal(result)
	if err != nil || len(body) > deviceplacement.MaxRequestBytes {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "registry placement preview result is invalid")
		return
	}
	writeJSON(w, r, http.StatusOK, append(body, '\n'))
}

func validDevicePlacementRegistryCandidateResult(
	value deviceplacement.PersistedInventoryPlacementV2Evaluation,
	owner model.Owner,
	evaluatedAtMS int64,
) bool {
	return value.SchemaVersion == deviceplacement.PersistedInventoryPlacementV2SchemaVersion &&
		value.EvaluationMode == deviceplacement.PersistedInventoryPlacementV2EvaluationMode &&
		value.SourceSchemaVersion == deviceplacement.SessionDeviceObservationInventoryV2SchemaVersion &&
		value.Owner == (deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) &&
		value.EvaluatedAtMS == evaluatedAtMS &&
		value.Notice == deviceplacement.PersistedInventoryPlacementV2Notice &&
		value.SelectedDeviceID == nil && value.SelectedInstanceID == nil &&
		value.Authority == (deviceplacement.PersistedInventoryPlacementBatchAuthority{}) &&
		value.EligibleCandidateCount >= 0 && value.EligibleCandidateCount <= len(value.Decisions)
}
