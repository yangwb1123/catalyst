package appserver

import (
	"encoding/json"
	"net/http"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// schedulerSelectionPreviewPath is an EXECUTE admission candidate. It
// exposes one deterministic target declaration from the owner-private v2
// observation but never adopts that target as a reservation or lease.
const schedulerSelectionPreviewPath = "/api/v1/device-placement/scheduler-preview"

const schedulerSelectionPreviewScope = "forge:devices:placement:preview"

type schedulerSelectionPreviewConfig struct {
	Enabled bool
	Source  deviceInventoryReadV2Source
	Now     devicePlacementRegistryCandidateClock
}

type schedulerSelectionPreviewRequest struct {
	ConversationID string                       `json:"conversation_id"`
	RunID          string                       `json:"run_id"`
	AttemptID      string                       `json:"attempt_id"`
	Requirements   deviceplacement.Requirements `json:"requirements"`
}

// newSchedulerSelectionPreviewRoutes is only composed by accepted EXECUTE
// activation. A missing or disabled dependency has the same 404 surface as
// an unregistered production route.
func newSchedulerSelectionPreviewRoutes(config *schedulerSelectionPreviewConfig) http.Handler {
	if config == nil || !config.Enabled || config.Source == nil || config.Now == nil {
		return http.HandlerFunc(serveDisabledSchedulerSelectionPreview)
	}
	return authn.RequireScopes(http.HandlerFunc(schedulerSelectionPreviewHandler{
		source: config.Source, now: config.Now,
	}.ServeHTTP), schedulerSelectionPreviewScope)
}

func serveDisabledSchedulerSelectionPreview(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, r, http.StatusNotFound, notFoundBody)
}

type schedulerSelectionPreviewHandler struct {
	source deviceInventoryReadV2Source
	now    devicePlacementRegistryCandidateClock
}

func (handler schedulerSelectionPreviewHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if r.URL.EscapedPath() != schedulerSelectionPreviewPath {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return
	}
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "scheduler selection preview does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	var request schedulerSelectionPreviewRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if !hasExactRequiredFields(body, "conversation_id", "run_id", "attempt_id", "requirements") {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "scheduler selection preview request is invalid")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	evaluatedAtMS, err := handler.now(r.Context())
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if evaluatedAtMS <= 0 || evaluatedAtMS > deviceplacement.MaxSafeIntegerMS {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "scheduler selection preview clock response is invalid")
		return
	}
	observation, err := handler.source.ReadOwnedDeviceInventoryV2(r.Context(), owner)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if !validDeviceInventoryReadCandidateV2(observation, owner) {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "scheduler selection preview source response is invalid")
		return
	}
	placement, err := deviceplacement.EvaluatePersistedInventoryObservationV2(
		observation,
		deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		request.Requirements,
		evaluatedAtMS,
	)
	if err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "scheduler selection preview request is invalid")
		return
	}
	selection, err := deviceplacement.SelectSchedulerCandidate(deviceplacement.SchedulerSelectionPreviewRequest{
		Owner:          deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		ConversationID: request.ConversationID, RunID: request.RunID, AttemptID: request.AttemptID,
		Placement: placement,
	})
	if err != nil || selection.Validate() != nil || !validSchedulerSelectionPreview(selection, owner) {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "scheduler selection preview result is invalid")
		return
	}
	body, err = json.Marshal(selection)
	if err != nil || len(body) > deviceplacement.MaxRequestBytes {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "scheduler selection preview result is invalid")
		return
	}
	writeJSON(w, r, http.StatusOK, append(body, '\n'))
}

func validSchedulerSelectionPreview(value deviceplacement.SchedulerSelectionPreviewObservation, owner model.Owner) bool {
	return value.Owner == (deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) &&
		value.Authority == (deviceplacement.SchedulerSelectionPreviewAuthority{}) && value.PreviewOnly
}
