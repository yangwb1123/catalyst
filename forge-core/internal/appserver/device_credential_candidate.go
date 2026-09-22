package appserver

import (
	"context"
	"errors"
	"net/http"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/devicecredential"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// lifecycleCredentialCandidatePath is an injected metadata-only credential
// lifecycle boundary. It never mints or returns credential material and is not
// mounted by production route construction.
const lifecycleCredentialCandidatePath = "/api/v1/device-enrollment-heartbeat/credential-candidate"

const lifecycleCredentialCandidateScope = "forge:devices:lifecycle:credential"

type lifecycleCredentialCandidateConfig struct {
	Enabled bool
}

type lifecycleCredentialCandidateRequest struct {
	DeviceID               string                  `json:"device_id"`
	Action                 devicecredential.Action `json:"action"`
	ApprovalState          string                  `json:"approval_state"`
	CredentialID           string                  `json:"credential_id"`
	KeyID                  string                  `json:"key_id"`
	PublicKeySHA256        string                  `json:"public_key_sha256"`
	KeyGeneration          uint64                  `json:"key_generation"`
	IssuedAtMS             uint64                  `json:"issued_at_ms"`
	ExpiresAtMS            uint64                  `json:"expires_at_ms"`
	NextCredentialID       string                  `json:"next_credential_id"`
	NextKeyID              string                  `json:"next_key_id"`
	NextPublicKeySHA256    string                  `json:"next_public_key_sha256"`
	ObservedAtMS           uint64                  `json:"observed_at_ms"`
	ExpectedDeviceRevision uint64                  `json:"expected_device_revision"`
}

type lifecycleCredentialCandidateResponse struct {
	SchemaVersion      string                               `json:"schema_version"`
	EvaluationMode     string                               `json:"evaluation_mode"`
	Owner              deviceidentity.Owner                 `json:"owner"`
	DeviceID           string                               `json:"device_id"`
	Action             devicecredential.Action              `json:"action"`
	Revision           uint64                               `json:"revision"`
	Previous           *devicecredential.State              `json:"previous,omitempty"`
	Next               devicecredential.State               `json:"next"`
	PreviewOnly        bool                                 `json:"preview_only"`
	CandidatePublished bool                                 `json:"candidate_published"`
	Authority          devicecredential.TransitionAuthority `json:"authority"`
}

var (
	errLifecycleCredentialCandidateMissing       = errors.New("lifecycle credential candidate registry is missing")
	errLifecycleCredentialCandidateDeviceMissing = errors.New("lifecycle credential candidate device is missing")
	errLifecycleCredentialCandidateStale         = errors.New("lifecycle credential candidate revision is stale")
	errLifecycleCredentialCandidateBinding       = errors.New("lifecycle credential candidate binding differs from live device")
)

func newLifecycleCredentialCandidateRoutes(config *lifecycleRegistryCandidateConfig) http.Handler {
	if config == nil || !config.Enabled || config.Store == nil || config.Credential == nil || !config.Credential.Enabled {
		return http.HandlerFunc(serveDisabledLifecycleRegistryCandidate)
	}
	return authn.RequireScopes(
		http.HandlerFunc(lifecycleCredentialCandidateHandler{store: config.Store}.ServeHTTP),
		lifecycleCredentialCandidateScope,
	)
}

type lifecycleCredentialCandidateHandler struct {
	store lifecycleRegistryCandidateStore
}

func (handler lifecycleCredentialCandidateHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if !validateLifecycleCredentialCandidateTransport(w, r) {
		return
	}
	request, ok := decodeLifecycleCredentialCandidateRequest(w, r)
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
	snapshot, states, ownerValue, current, transition, err := handler.computeCredentialCandidate(r.Context(), owner, request)
	if err != nil {
		writeLifecycleCredentialCandidateComputeError(w, r, err)
		return
	}
	published, err := handler.store.ReplaceStatesIfUnchanged(r.Context(), owner, snapshot, states)
	if err != nil {
		writeLifecycleCredentialCandidateError(w, r, err)
		return
	}
	if !published.Present() {
		writeConversationError(w, r, http.StatusBadGateway, "lifecycle_registry_invalid", "credential candidate did not publish an image")
		return
	}
	writeLifecycleCredentialCandidateResponse(w, r, ownerValue, request, current, transition)
}

func validateLifecycleCredentialCandidateTransport(w http.ResponseWriter, r *http.Request) bool {
	if r.URL.EscapedPath() != lifecycleCredentialCandidatePath {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return false
	}
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "credential candidate does not accept query parameters")
		return false
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return false
	}
	return true
}

func decodeLifecycleCredentialCandidateRequest(w http.ResponseWriter, r *http.Request) (lifecycleCredentialCandidateRequest, bool) {
	var request lifecycleCredentialCandidateRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return lifecycleCredentialCandidateRequest{}, false
	}
	if !hasExactRequiredFields(body,
		"device_id", "action", "approval_state", "credential_id", "key_id",
		"public_key_sha256", "key_generation", "issued_at_ms", "expires_at_ms",
		"next_credential_id", "next_key_id", "next_public_key_sha256", "observed_at_ms",
		"expected_device_revision",
	) || request.DeviceID == "" || request.ExpectedDeviceRevision == 0 {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "credential candidate request is invalid")
		return lifecycleCredentialCandidateRequest{}, false
	}
	return request, true
}

func (handler lifecycleCredentialCandidateHandler) computeCredentialCandidate(
	ctx context.Context,
	owner model.Owner,
	request lifecycleCredentialCandidateRequest,
) (
	deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot,
	[]deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
	deviceidentity.Owner,
	deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
	devicecredential.Transition,
	error,
) {
	snapshot, err := handler.store.ReadSnapshot(ctx, owner)
	if err != nil {
		return emptyCredentialCandidateComputation(err)
	}
	if !snapshot.Present() {
		return emptyCredentialCandidateComputation(errLifecycleCredentialCandidateMissing)
	}
	ownerValue := deviceidentity.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
	states := snapshot.States()
	if err := deviceinventory.ValidatePersistedEnrollmentHeartbeatLifecycleRegistry(
		deviceinventory.PersistedEnrollmentHeartbeatLifecycleRegistryState{Owner: ownerValue, States: states},
	); err != nil {
		return emptyCredentialCandidateComputation(err)
	}
	index := credentialCandidateStateIndex(states, request.DeviceID)
	if index < 0 {
		return emptyCredentialCandidateComputation(errLifecycleCredentialCandidateDeviceMissing)
	}
	currentImage := states[index]
	if currentImage.Revision != request.ExpectedDeviceRevision {
		return emptyCredentialCandidateComputation(errLifecycleCredentialCandidateStale)
	}
	transition, err := applyCredentialCandidate(currentImage, ownerValue, request)
	if err != nil {
		return emptyCredentialCandidateComputation(err)
	}
	next := transition.Next
	states[index].CredentialCandidate = &next
	return snapshot, states, ownerValue, currentImage, transition, nil
}

func applyCredentialCandidate(
	image deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
	owner deviceidentity.Owner,
	request lifecycleCredentialCandidateRequest,
) (devicecredential.Transition, error) {
	var current *devicecredential.State
	if image.CredentialCandidate != nil {
		copy := *image.CredentialCandidate
		current = &copy
	}
	if request.Action == devicecredential.ActionIssue && image.Device.CredentialState == devicecredential.CredentialRevoked {
		return devicecredential.Transition{}, devicecredential.ErrCredentialTerminal
	}
	if request.Action == devicecredential.ActionIssue &&
		(request.ApprovalState != image.Device.ApprovalState || request.KeyID != image.Device.KeyID || request.PublicKeySHA256 != image.Device.PublicKeySHA256) {
		return devicecredential.Transition{}, errLifecycleCredentialCandidateBinding
	}
	if current != nil && request.ApprovalState != "" && request.ApprovalState != current.ApprovalState {
		return devicecredential.Transition{}, errLifecycleCredentialCandidateBinding
	}
	if request.Action != devicecredential.ActionIssue && current == nil {
		return devicecredential.Transition{}, devicecredential.ErrCurrentRequired
	}
	return devicecredential.Apply(devicecredential.Request{
		Current: current, Action: request.Action, Owner: owner, DeviceID: request.DeviceID,
		ApprovalState: request.ApprovalState, CredentialID: request.CredentialID,
		KeyID: request.KeyID, PublicKeySHA256: request.PublicKeySHA256,
		KeyGeneration: request.KeyGeneration, IssuedAtMS: request.IssuedAtMS,
		ExpiresAtMS: request.ExpiresAtMS, NextCredentialID: request.NextCredentialID,
		NextKeyID: request.NextKeyID, NextPublicKeySHA256: request.NextPublicKeySHA256,
		ObservedAtMS: request.ObservedAtMS,
	})
}

func emptyCredentialCandidateComputation(err error) (
	deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot,
	[]deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
	deviceidentity.Owner,
	deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
	devicecredential.Transition,
	error,
) {
	return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, nil,
		deviceidentity.Owner{}, deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{}, devicecredential.Transition{}, err
}

func credentialCandidateStateIndex(states []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState, deviceID string) int {
	for index := range states {
		if states[index].Device.DeviceID == deviceID {
			return index
		}
	}
	return -1
}

func writeLifecycleCredentialCandidateResponse(
	w http.ResponseWriter,
	r *http.Request,
	owner deviceidentity.Owner,
	request lifecycleCredentialCandidateRequest,
	current deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
	transition devicecredential.Transition,
) {
	writeConversationJSON(w, r, http.StatusOK, lifecycleCredentialCandidateResponse{
		SchemaVersion: transition.SchemaVersion, EvaluationMode: transition.EvaluationMode,
		Owner: owner, DeviceID: request.DeviceID, Action: transition.Action,
		Revision: current.Revision, Previous: transition.Previous, Next: transition.Next,
		PreviewOnly: true, CandidatePublished: true,
		Authority: devicecredential.TransitionAuthority{},
	})
}

func writeLifecycleCredentialCandidateComputeError(w http.ResponseWriter, r *http.Request, err error) {
	switch {
	case errors.Is(err, errLifecycleCredentialCandidateMissing):
		writeConversationError(w, r, http.StatusNotFound, "lifecycle_registry_missing", "lifecycle registry image is not present")
	case errors.Is(err, errLifecycleCredentialCandidateDeviceMissing):
		writeConversationError(w, r, http.StatusNotFound, "device_not_found", "device is not present in the owner lifecycle registry")
	case errors.Is(err, errLifecycleCredentialCandidateStale):
		writeConversationError(w, r, http.StatusConflict, "credential_conflict", "device lifecycle image is stale; refresh and retry")
	case errors.Is(err, errLifecycleCredentialCandidateBinding):
		writeConversationError(w, r, http.StatusBadRequest, "credential_rejected", "credential candidate does not match the live device binding")
	case errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryFileCASInvalid),
		errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryOwnerMismatch),
		errors.Is(err, deviceinventory.ErrLifecycleRegistryOwnerMismatch),
		errors.Is(err, deviceinventory.ErrLifecycleRegistryInvalidState):
		writeConversationError(w, r, http.StatusBadGateway, "lifecycle_registry_invalid", "lifecycle registry source response is invalid")
	default:
		var credentialErr devicecredential.ErrorCode
		if errors.As(err, &credentialErr) {
			writeLifecycleCredentialCandidateError(w, r, err)
			return
		}
		writeLifecycleRegistryCandidateReadError(w, r, err)
	}
}

func writeLifecycleCredentialCandidateError(w http.ResponseWriter, r *http.Request, err error) {
	switch {
	case errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryFileCASConflict):
		writeConversationError(w, r, http.StatusConflict, "credential_conflict", "lifecycle registry changed; refresh and retry")
	case errors.Is(err, deviceinventory.ErrLifecycleRegistryOwnerMismatch):
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "lifecycle registry owner does not match the authenticated principal")
	case errors.Is(err, devicecredential.ErrCredentialTerminal),
		errors.Is(err, devicecredential.ErrRotationUnchanged),
		errors.Is(err, devicecredential.ErrGenerationOverflow),
		errors.Is(err, devicecredential.ErrCredentialMismatch),
		errors.Is(err, devicecredential.ErrKeyGenerationMismatch):
		writeConversationError(w, r, http.StatusConflict, "credential_conflict", "device credential lifecycle transition is not allowed")
	case errors.Is(err, devicecredential.ErrApprovalRevoked):
		writeConversationError(w, r, http.StatusConflict, "approval_required", "device owner approval does not permit a credential candidate")
	case errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryFileCASWrite):
		writeConversationBackendUnavailable(w, r)
	default:
		writeConversationError(w, r, http.StatusBadRequest, "credential_rejected", "device credential candidate was rejected")
	}
}
