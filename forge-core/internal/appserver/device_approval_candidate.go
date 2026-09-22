package appserver

import (
	"context"
	"errors"
	"net/http"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceapproval"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// lifecycleApprovalCandidateConfig is intentionally small. Enabling it only
// exposes an injected candidate route; it does not enable enrollment,
// credential issuance, heartbeat acceptance, inventory authority, or Runner
// dispatch.
type lifecycleApprovalCandidateConfig struct {
	Enabled bool
}

type lifecycleApprovalCandidateRequest struct {
	DeviceID               string                `json:"device_id"`
	Action                 deviceapproval.Action `json:"action"`
	NextKeyID              string                `json:"next_key_id"`
	NextPublicKeySHA256    string                `json:"next_public_key_sha256"`
	ExpectedDeviceRevision uint64                `json:"expected_device_revision"`
}

type lifecycleApprovalCandidateResponse struct {
	SchemaVersion      string                             `json:"schema_version"`
	EvaluationMode     string                             `json:"evaluation_mode"`
	Owner              deviceidentity.Owner               `json:"owner"`
	DeviceID           string                             `json:"device_id"`
	Action             deviceapproval.Action              `json:"action"`
	Revision           uint64                             `json:"revision"`
	Previous           deviceapproval.State               `json:"previous"`
	Next               deviceapproval.State               `json:"next"`
	PreviewOnly        bool                               `json:"preview_only"`
	CandidatePublished bool                               `json:"candidate_published"`
	Authority          deviceapproval.TransitionAuthority `json:"authority"`
}

var (
	errLifecycleApprovalCandidateMissing       = errors.New("lifecycle approval candidate registry is missing")
	errLifecycleApprovalCandidateDeviceMissing = errors.New("lifecycle approval candidate device is missing")
	errLifecycleApprovalCandidateStale         = errors.New("lifecycle approval candidate revision is stale")
)

func newLifecycleApprovalCandidateRoutes(config *lifecycleRegistryCandidateConfig) http.Handler {
	if config == nil || !config.Enabled || config.Store == nil || config.Approval == nil || !config.Approval.Enabled {
		return http.HandlerFunc(serveDisabledLifecycleRegistryCandidate)
	}
	return authn.RequireScopes(
		http.HandlerFunc(lifecycleApprovalCandidateHandler{store: config.Store}.ServeHTTP),
		lifecycleApprovalCandidateScope,
	)
}

type lifecycleApprovalCandidateHandler struct {
	store lifecycleRegistryCandidateStore
}

func (handler lifecycleApprovalCandidateHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if !validateLifecycleApprovalCandidateTransport(w, r) {
		return
	}
	request, ok := decodeLifecycleApprovalCandidateRequest(w, r)
	if !ok {
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if handler.store == nil {
		writeConversationBackendUnavailable(w, r)
		return
	}
	snapshot, states, ownerValue, currentImage, transition, err := handler.computeApprovalCandidate(r.Context(), owner, request)
	if err != nil {
		writeLifecycleApprovalCandidateComputeError(w, r, err)
		return
	}
	published, err := handler.store.ReplaceStatesIfUnchanged(r.Context(), owner, snapshot, states)
	if err != nil {
		writeLifecycleApprovalCandidateError(w, r, err)
		return
	}
	if !published.Present() {
		writeConversationError(w, r, http.StatusBadGateway, "lifecycle_registry_invalid", "approval candidate did not publish an image")
		return
	}
	writeLifecycleApprovalCandidateResponse(w, r, ownerValue, request, currentImage, transition)
}

func validateLifecycleApprovalCandidateTransport(w http.ResponseWriter, r *http.Request) bool {
	if r.URL.EscapedPath() != lifecycleApprovalCandidatePath {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return false
	}
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "approval candidate does not accept query parameters")
		return false
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return false
	}
	return true
}

func decodeLifecycleApprovalCandidateRequest(w http.ResponseWriter, r *http.Request) (lifecycleApprovalCandidateRequest, bool) {
	var request lifecycleApprovalCandidateRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return lifecycleApprovalCandidateRequest{}, false
	}
	if !hasExactRequiredFields(body,
		"device_id", "action", "next_key_id", "next_public_key_sha256", "expected_device_revision",
	) || request.DeviceID == "" || request.ExpectedDeviceRevision == 0 {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "approval candidate request is invalid")
		return lifecycleApprovalCandidateRequest{}, false
	}
	return request, true
}

func (handler lifecycleApprovalCandidateHandler) computeApprovalCandidate(
	ctx context.Context,
	owner model.Owner,
	request lifecycleApprovalCandidateRequest,
) (
	deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot,
	[]deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
	deviceidentity.Owner,
	deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
	deviceapproval.Transition,
	error,
) {
	snapshot, err := handler.store.ReadSnapshot(ctx, owner)
	if err != nil {
		return emptyApprovalCandidateComputation(err)
	}
	if !snapshot.Present() {
		return emptyApprovalCandidateComputation(errLifecycleApprovalCandidateMissing)
	}
	ownerValue := deviceidentity.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
	states := snapshot.States()
	if err := deviceinventory.ValidatePersistedEnrollmentHeartbeatLifecycleRegistry(
		deviceinventory.PersistedEnrollmentHeartbeatLifecycleRegistryState{Owner: ownerValue, States: states},
	); err != nil {
		return emptyApprovalCandidateComputation(err)
	}
	index := approvalCandidateStateIndex(states, request.DeviceID)
	if index < 0 {
		return emptyApprovalCandidateComputation(errLifecycleApprovalCandidateDeviceMissing)
	}
	currentImage := states[index]
	if currentImage.Revision != request.ExpectedDeviceRevision {
		return emptyApprovalCandidateComputation(errLifecycleApprovalCandidateStale)
	}
	transition, err := applyApprovalCandidate(currentImage, ownerValue, request)
	if err != nil {
		return emptyApprovalCandidateComputation(err)
	}
	next := transition.Next
	states[index].ApprovalCandidate = &next
	return snapshot, states, ownerValue, currentImage, transition, nil
}

func applyApprovalCandidate(
	image deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
	owner deviceidentity.Owner,
	request lifecycleApprovalCandidateRequest,
) (deviceapproval.Transition, error) {
	current := deviceapproval.State{
		DeviceID:        image.Device.DeviceID,
		Owner:           owner,
		ApprovalState:   image.Device.ApprovalState,
		KeyID:           image.Device.KeyID,
		PublicKeySHA256: image.Device.PublicKeySHA256,
		KeyGeneration:   1,
	}
	if image.ApprovalCandidate != nil {
		current = *image.ApprovalCandidate
	}
	return deviceapproval.Apply(deviceapproval.Request{
		Current:             current,
		Action:              request.Action,
		Owner:               owner,
		DeviceID:            request.DeviceID,
		NextKeyID:           request.NextKeyID,
		NextPublicKeySHA256: request.NextPublicKeySHA256,
	})
}

func emptyApprovalCandidateComputation(err error) (
	deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot,
	[]deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
	deviceidentity.Owner,
	deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
	deviceapproval.Transition,
	error,
) {
	return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, nil,
		deviceidentity.Owner{}, deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{}, deviceapproval.Transition{}, err
}

func approvalCandidateStateIndex(states []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState, deviceID string) int {
	for index := range states {
		if states[index].Device.DeviceID == deviceID {
			return index
		}
	}
	return -1
}

func writeLifecycleApprovalCandidateResponse(
	w http.ResponseWriter,
	r *http.Request,
	owner deviceidentity.Owner,
	request lifecycleApprovalCandidateRequest,
	current deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
	transition deviceapproval.Transition,
) {
	writeConversationJSON(w, r, http.StatusOK, lifecycleApprovalCandidateResponse{
		SchemaVersion:      transition.SchemaVersion,
		EvaluationMode:     transition.EvaluationMode,
		Owner:              owner,
		DeviceID:           request.DeviceID,
		Action:             transition.Action,
		Revision:           current.Revision,
		Previous:           transition.Previous,
		Next:               transition.Next,
		PreviewOnly:        true,
		CandidatePublished: true,
		Authority:          deviceapproval.TransitionAuthority{},
	})
}

func writeLifecycleApprovalCandidateComputeError(w http.ResponseWriter, r *http.Request, err error) {
	switch {
	case errors.Is(err, errLifecycleApprovalCandidateMissing):
		writeConversationError(w, r, http.StatusNotFound, "lifecycle_registry_missing", "lifecycle registry image is not present")
	case errors.Is(err, errLifecycleApprovalCandidateDeviceMissing):
		writeConversationError(w, r, http.StatusNotFound, "device_not_found", "device is not present in the owner lifecycle registry")
	case errors.Is(err, errLifecycleApprovalCandidateStale):
		writeConversationError(w, r, http.StatusConflict, "approval_conflict", "device lifecycle image is stale; refresh and retry")
	case errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryFileCASInvalid),
		errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryOwnerMismatch),
		errors.Is(err, deviceinventory.ErrLifecycleRegistryOwnerMismatch),
		errors.Is(err, deviceinventory.ErrLifecycleRegistryInvalidState):
		writeConversationError(w, r, http.StatusBadGateway, "lifecycle_registry_invalid", "lifecycle registry source response is invalid")
	default:
		var approvalErr deviceapproval.ErrorCode
		if errors.As(err, &approvalErr) {
			writeLifecycleApprovalCandidateError(w, r, err)
			return
		}
		writeLifecycleRegistryCandidateReadError(w, r, err)
	}
}

func writeLifecycleApprovalCandidateError(w http.ResponseWriter, r *http.Request, err error) {
	switch {
	case errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryFileCASConflict):
		writeConversationError(w, r, http.StatusConflict, "approval_conflict", "lifecycle registry changed; refresh and retry")
	case errors.Is(err, deviceinventory.ErrLifecycleRegistryOwnerMismatch):
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "lifecycle registry owner does not match the authenticated principal")
	case errors.Is(err, deviceapproval.ErrRevocationTerminal),
		errors.Is(err, deviceapproval.ErrInvalidTransition),
		errors.Is(err, deviceapproval.ErrRotationKeyUnchanged),
		errors.Is(err, deviceapproval.ErrKeyGenerationOverflow):
		writeConversationError(w, r, http.StatusConflict, "approval_conflict", "device approval lifecycle transition is not allowed")
	case errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryFileCASWrite):
		writeConversationBackendUnavailable(w, r)
	default:
		writeConversationError(w, r, http.StatusBadRequest, "approval_rejected", "device approval candidate was rejected")
	}
}
