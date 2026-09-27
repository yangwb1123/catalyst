package appserver

import (
	"context"
	"errors"
	"net/http"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
)

// lifecycleHeartbeatCandidatePath is an injected, candidate-only heartbeat
// boundary. Production does not mount it while ADR-0114 is Proposed and the
// device fabric remains default-off.
const lifecycleHeartbeatCandidatePath = "/api/v1/device-enrollment-heartbeat/heartbeat"

// A heartbeat uses its own scope so a human Conversation token cannot
// accidentally become a device lifecycle credential when this candidate is
// evaluated in a fixture or migration harness.
const lifecycleHeartbeatCandidateScope = "forge:devices:lifecycle:heartbeat"

// lifecycleHeartbeatCandidateClock is deliberately injected. The candidate
// must use a server-owned observation time; accepting a timestamp from the
// device would make proof expiry and lease expiry caller-controlled.
type lifecycleHeartbeatCandidateClock func(context.Context) (uint64, error)

type lifecycleHeartbeatCandidateConfig struct {
	Enabled bool
	// AllowUnsignedProof is an explicit fixture-only opt-in for the legacy
	// structural proof path. Production and accepted assemblies never set it;
	// cryptographic challenge consumption belongs to the signed candidate.
	AllowUnsignedProof bool
	Now                lifecycleHeartbeatCandidateClock
	StaleAfterMS       uint64
}

type lifecycleHeartbeatCandidateRequest struct {
	Device                 deviceidentity.DeviceBinding `json:"device"`
	Challenge              deviceidentity.Challenge     `json:"challenge"`
	Proof                  deviceidentity.Proof         `json:"proof"`
	Heartbeat              deviceheartbeat.Heartbeat    `json:"heartbeat"`
	LeaseTTLMS             uint64                       `json:"lease_ttl_ms"`
	ExpectedDeviceRevision uint64                       `json:"expected_device_revision"`
}

type lifecycleHeartbeatCandidateResponse struct {
	SchemaVersion      string                                       `json:"schema_version"`
	Owner              deviceidentity.Owner                         `json:"owner"`
	DeviceID           string                                       `json:"device_id"`
	Revision           uint64                                       `json:"revision"`
	Heartbeat          deviceheartbeat.PersistedInstance            `json:"heartbeat"`
	Inventory          deviceinventory.PersistedInventoryState      `json:"inventory"`
	Projection         deviceinventory.PersistedInventoryProjection `json:"projection"`
	PreviewOnly        bool                                         `json:"preview_only"`
	CandidatePublished bool                                         `json:"candidate_published"`
	Authority          deviceinventory.LifecycleAuthority           `json:"authority"`
}

func newLifecycleHeartbeatCandidateRoutes(config *lifecycleRegistryCandidateConfig) http.Handler {
	if config == nil || !config.Enabled || config.Store == nil || config.Heartbeat == nil ||
		!config.Heartbeat.Enabled || !config.Heartbeat.AllowUnsignedProof {
		return http.HandlerFunc(serveDisabledLifecycleRegistryCandidate)
	}
	return authn.RequireScopes(
		http.HandlerFunc(lifecycleHeartbeatCandidateHandler{config: config.Heartbeat, store: config.Store}.ServeHTTP),
		lifecycleHeartbeatCandidateScope,
	)
}

type lifecycleHeartbeatCandidateHandler struct {
	config *lifecycleHeartbeatCandidateConfig
	store  lifecycleRegistryCandidateStore
}

func (handler lifecycleHeartbeatCandidateHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if r.URL.EscapedPath() != lifecycleHeartbeatCandidatePath {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return
	}
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "heartbeat candidate does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	var request lifecycleHeartbeatCandidateRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if !hasExactRequiredFields(body,
		"device", "challenge", "proof", "heartbeat", "lease_ttl_ms",
		"expected_device_revision",
	) || request.LeaseTTLMS == 0 {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "heartbeat request is invalid")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if handler.config == nil || handler.store == nil || handler.config.Now == nil || handler.config.StaleAfterMS == 0 {
		writeConversationBackendUnavailable(w, r)
		return
	}
	nowMS, err := handler.config.Now(r.Context())
	if err != nil {
		if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
			writeConversationError(w, r, http.StatusServiceUnavailable, "heartbeat_clock_unavailable", "server observation clock is unavailable")
			return
		}
		writeConversationBackendUnavailable(w, r)
		return
	}
	snapshot, err := handler.store.ReadSnapshot(r.Context(), owner)
	if err != nil {
		writeLifecycleRegistryCandidateReadError(w, r, err)
		return
	}
	ownerValue := deviceidentity.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
	var current *deviceinventory.PersistedEnrollmentHeartbeatLifecycleRegistryState
	cordonState, reservationState := "clear", "none"
	if snapshot.Present() {
		value := deviceinventory.PersistedEnrollmentHeartbeatLifecycleRegistryState{
			Owner:  ownerValue,
			States: snapshot.States(),
		}
		current = &value
		for _, state := range value.States {
			if state.Device.DeviceID == request.Device.DeviceID {
				cordonState = state.Inventory.Device.CordonState
				reservationState = state.Inventory.Device.ReservationState
				break
			}
		}
	}
	input := deviceinventory.EnrollmentHeartbeatLifecycleInput{
		Owner:                     ownerValue,
		Device:                    request.Device,
		Challenge:                 request.Challenge,
		Proof:                     request.Proof,
		Heartbeat:                 request.Heartbeat,
		ExpectedHeartbeatRevision: request.ExpectedDeviceRevision,
		CordonState:               cordonState,
		ReservationState:          reservationState,
		ServerObservedAtMS:        nowMS,
		LeaseTTLMS:                request.LeaseTTLMS,
		IdentityNowMS:             nowMS,
		EvaluatedAtMS:             nowMS,
		StaleAfterMS:              handler.config.StaleAfterMS,
	}
	next, result, err := deviceinventory.CommitPersistedEnrollmentHeartbeatLifecycleRegistry(
		current, ownerValue, request.ExpectedDeviceRevision, input,
	)
	if err != nil {
		writeLifecycleHeartbeatCandidateError(w, r, err)
		return
	}
	published, err := handler.store.ReplaceStatesIfUnchanged(r.Context(), owner, snapshot, next.States)
	if err != nil {
		writeLifecycleHeartbeatCandidateError(w, r, err)
		return
	}
	if !published.Present() {
		writeConversationError(w, r, http.StatusBadGateway, "lifecycle_registry_invalid", "heartbeat replacement did not publish an image")
		return
	}
	response := lifecycleHeartbeatCandidateResponse{
		SchemaVersion:      deviceinventory.EnrollmentHeartbeatLifecycleSchemaVersion,
		Owner:              ownerValue,
		DeviceID:           request.Device.DeviceID,
		Revision:           result.Inventory.Revision,
		Heartbeat:          result.Heartbeat,
		Inventory:          result.Inventory,
		Projection:         result.Projection,
		PreviewOnly:        result.PreviewOnly,
		CandidatePublished: true,
		Authority:          result.Authority,
	}
	writeConversationJSON(w, r, http.StatusOK, response)
}

func writeLifecycleHeartbeatCandidateError(w http.ResponseWriter, r *http.Request, err error) {
	switch {
	case errors.Is(err, deviceidentity.ErrOwnerMismatch),
		errors.Is(err, deviceidentity.ErrDeviceMismatch),
		errors.Is(err, deviceidentity.ErrKeyMismatch),
		errors.Is(err, errSignedHeartbeatDigestMismatch),
		errors.Is(err, deviceidentity.ErrInvalidSignedProof),
		errors.Is(err, deviceidentity.ErrPublicKeyEncoding),
		errors.Is(err, deviceidentity.ErrPublicKeyDigestMismatch),
		errors.Is(err, deviceidentity.ErrSignatureEncoding),
		errors.Is(err, deviceidentity.ErrSignatureInvalid),
		errors.Is(err, deviceidentity.ErrCredentialRevoked),
		errors.Is(err, deviceidentity.ErrCredentialExpired),
		errors.Is(err, deviceidentity.ErrChallengeMismatch),
		errors.Is(err, deviceidentity.ErrChallengeReplayed),
		errors.Is(err, deviceidentity.ErrChallengeExpired),
		errors.Is(err, deviceidentity.ErrChallengeNotYetLive),
		errors.Is(err, deviceidentity.ErrProofExpired),
		errors.Is(err, deviceidentity.ErrProofNotYetLive),
		errors.Is(err, deviceidentity.ErrInvalidProofWindow),
		errors.Is(err, deviceidentity.ErrInvalidBinding),
		errors.Is(err, deviceidentity.ErrUnknownApproval),
		errors.Is(err, deviceidentity.ErrUnknownCredential):
		writeConversationError(w, r, http.StatusForbidden, "device_identity_rejected", "device identity proof was rejected")
	case errors.Is(err, deviceinventory.ErrApprovalRequired):
		writeConversationError(w, r, http.StatusConflict, "device_approval_required", "device owner approval is required")
	case errors.Is(err, deviceheartbeat.ErrRevisionConflict),
		errors.Is(err, deviceheartbeat.ErrOldGeneration),
		errors.Is(err, deviceheartbeat.ErrGenerationSkipped),
		errors.Is(err, deviceheartbeat.ErrInstanceChangedWithinGeneration),
		errors.Is(err, deviceheartbeat.ErrSequenceNotIncreasing),
		errors.Is(err, deviceinventory.ErrLifecycleRevisionConflict),
		errors.Is(err, deviceinventory.ErrLifecycleRegistryRevisionConflict),
		errors.Is(err, deviceinventory.ErrLifecycleBindingChanged),
		errors.Is(err, deviceinventory.ErrLifecycleServerStateChanged),
		errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryFileCASConflict),
		errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryFileCASRollback):
		writeConversationError(w, r, http.StatusConflict, "heartbeat_conflict", "heartbeat is stale; refresh lifecycle state and retry")
	case errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryOwnerMismatch),
		errors.Is(err, deviceinventory.ErrLifecycleRegistryOwnerMismatch):
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "lifecycle registry owner does not match the authenticated principal")
	case errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryFileCASWrite):
		writeConversationBackendUnavailable(w, r)
	default:
		writeConversationError(w, r, http.StatusBadRequest, "heartbeat_rejected", "heartbeat request was rejected")
	}
}
