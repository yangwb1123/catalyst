package appserver

import (
	"context"
	"errors"
	"net/http"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// lifecycleSignedHeartbeatCandidatePath is the cryptographically checked
// companion to the structural heartbeat candidate. It is mounted only by an
// injected candidate handler and never by ordinary production assembly.
const lifecycleSignedHeartbeatCandidatePath = "/api/v1/device-enrollment-heartbeat/heartbeat-signed"

const lifecycleSignedHeartbeatCandidateScope = "forge:devices:lifecycle:heartbeat:signed"

type lifecycleSignedHeartbeatCandidateRequest struct {
	DeviceID               string                     `json:"device_id"`
	Challenge              deviceidentity.Challenge   `json:"challenge"`
	Proof                  deviceidentity.SignedProof `json:"signed_proof"`
	Heartbeat              deviceheartbeat.Heartbeat  `json:"heartbeat"`
	HeartbeatSHA256        string                     `json:"heartbeat_sha256"`
	LeaseTTLMS             uint64                     `json:"lease_ttl_ms"`
	ExpectedDeviceRevision uint64                     `json:"expected_device_revision"`
}

type lifecycleSignedHeartbeatCandidateResponse struct {
	SchemaVersion      string                                       `json:"schema_version"`
	Owner              deviceidentity.Owner                         `json:"owner"`
	DeviceID           string                                       `json:"device_id"`
	Revision           uint64                                       `json:"revision"`
	Heartbeat          deviceheartbeat.PersistedInstance            `json:"heartbeat"`
	Inventory          deviceinventory.PersistedInventoryState      `json:"inventory"`
	Projection         deviceinventory.PersistedInventoryProjection `json:"projection"`
	ProofVerified      bool                                         `json:"proof_verified"`
	PreviewOnly        bool                                         `json:"preview_only"`
	CandidatePublished bool                                         `json:"candidate_published"`
	Authority          deviceinventory.LifecycleAuthority           `json:"authority"`
}

var errSignedHeartbeatDigestMismatch = errors.New("signed heartbeat payload digest does not match proof challenge")

func newLifecycleSignedHeartbeatCandidateRoutes(config *lifecycleRegistryCandidateConfig) http.Handler {
	if config == nil || !config.Enabled || config.Store == nil || config.Heartbeat == nil ||
		!config.Heartbeat.Enabled {
		return http.HandlerFunc(serveDisabledLifecycleRegistryCandidate)
	}
	return authn.RequireScopes(
		http.HandlerFunc(lifecycleSignedHeartbeatCandidateHandler{config: config.Heartbeat, store: config.Store}.ServeHTTP),
		lifecycleSignedHeartbeatCandidateScope,
	)
}

type lifecycleSignedHeartbeatCandidateHandler struct {
	config *lifecycleHeartbeatCandidateConfig
	store  lifecycleRegistryCandidateStore
}

func (handler lifecycleSignedHeartbeatCandidateHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	request, ok := decodeSignedHeartbeatCandidateRequest(w, r)
	if !ok {
		return
	}
	owner, nowMS, ok := handler.authenticatedNow(w, r)
	if !ok {
		return
	}
	ownerValue := deviceidentity.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
	snapshot, current, ok := readSignedHeartbeatCurrent(w, r, handler.store, owner, request)
	if !ok {
		return
	}
	decision, structural, err := verifySignedHeartbeat(ownerValue, current, request, nowMS)
	if err != nil {
		writeLifecycleHeartbeatCandidateError(w, r, err)
		return
	}
	if decision.ApprovalRequired {
		writeConversationError(w, r, http.StatusConflict, "device_approval_required", "device owner approval is required")
		return
	}
	next, result, ok := handler.commitHeartbeat(w, r, owner, ownerValue, request, snapshot, current, structural, nowMS)
	if !ok {
		return
	}
	writeConversationJSON(w, r, http.StatusOK, lifecycleSignedHeartbeatCandidateResponse{
		SchemaVersion: deviceinventory.EnrollmentHeartbeatLifecycleSchemaVersion,
		Owner:         ownerValue, DeviceID: request.DeviceID, Revision: next.Revision,
		Heartbeat: result.Heartbeat, Inventory: result.Inventory, Projection: result.Projection,
		ProofVerified: true, PreviewOnly: true, CandidatePublished: true,
		Authority: deviceinventory.LifecycleAuthority{},
	})
}

func (handler lifecycleSignedHeartbeatCandidateHandler) authenticatedNow(
	w http.ResponseWriter,
	r *http.Request,
) (model.Owner, uint64, bool) {
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return model.Owner{}, 0, false
	}
	if handler.config == nil || handler.store == nil || handler.config.Now == nil || handler.config.StaleAfterMS == 0 {
		writeConversationBackendUnavailable(w, r)
		return model.Owner{}, 0, false
	}
	nowMS, err := handler.config.Now(r.Context())
	if err == nil {
		return owner, nowMS, true
	}
	if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
		writeConversationError(w, r, http.StatusServiceUnavailable, "heartbeat_clock_unavailable", "server observation clock is unavailable")
	} else {
		writeConversationBackendUnavailable(w, r)
	}
	return model.Owner{}, 0, false
}

func (handler lifecycleSignedHeartbeatCandidateHandler) commitHeartbeat(
	w http.ResponseWriter,
	r *http.Request,
	owner model.Owner,
	ownerValue deviceidentity.Owner,
	request lifecycleSignedHeartbeatCandidateRequest,
	snapshot deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot,
	current deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
	structural deviceidentity.Proof,
	nowMS uint64,
) (deviceinventory.PersistedEnrollmentHeartbeatLifecycleState, deviceinventory.EnrollmentHeartbeatLifecycleResult, bool) {
	input := signedHeartbeatLifecycleInput(ownerValue, current, request, structural, nowMS, handler.config.StaleAfterMS)
	next, result, err := deviceinventory.CommitPersistedEnrollmentHeartbeatLifecycle(
		&current, request.ExpectedDeviceRevision, input,
	)
	if err != nil {
		writeLifecycleHeartbeatCandidateError(w, r, err)
		return deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{}, deviceinventory.EnrollmentHeartbeatLifecycleResult{}, false
	}
	states := snapshot.States()
	for index := range states {
		if states[index].Device.DeviceID == request.DeviceID {
			consumed := request.Challenge
			consumed.Consumed = true
			next.ChallengeCandidate = &consumed
			states[index] = next
			break
		}
	}
	published, err := handler.store.ReplaceStatesIfUnchanged(r.Context(), owner, snapshot, states)
	if err != nil {
		writeLifecycleHeartbeatCandidateError(w, r, err)
		return deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{}, deviceinventory.EnrollmentHeartbeatLifecycleResult{}, false
	}
	if !published.Present() {
		writeConversationError(w, r, http.StatusBadGateway, "lifecycle_registry_invalid", "signed heartbeat replacement did not publish an image")
		return deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{}, deviceinventory.EnrollmentHeartbeatLifecycleResult{}, false
	}
	return next, result, true
}

func signedHeartbeatLifecycleInput(
	owner deviceidentity.Owner,
	current deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
	request lifecycleSignedHeartbeatCandidateRequest,
	proof deviceidentity.Proof,
	nowMS, staleAfterMS uint64,
) deviceinventory.EnrollmentHeartbeatLifecycleInput {
	return deviceinventory.EnrollmentHeartbeatLifecycleInput{
		Owner:                     owner,
		Device:                    current.Device,
		Challenge:                 request.Challenge,
		Proof:                     proof,
		Heartbeat:                 request.Heartbeat,
		ExpectedHeartbeatRevision: current.Heartbeat.Revision,
		CordonState:               current.Inventory.Device.CordonState,
		ReservationState:          current.Inventory.Device.ReservationState,
		ServerObservedAtMS:        nowMS,
		LeaseTTLMS:                request.LeaseTTLMS,
		IdentityNowMS:             nowMS,
		EvaluatedAtMS:             nowMS,
		StaleAfterMS:              staleAfterMS,
	}
}

func decodeSignedHeartbeatCandidateRequest(w http.ResponseWriter, r *http.Request) (lifecycleSignedHeartbeatCandidateRequest, bool) {
	if r.URL.EscapedPath() != lifecycleSignedHeartbeatCandidatePath {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return lifecycleSignedHeartbeatCandidateRequest{}, false
	}
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "signed heartbeat candidate does not accept query parameters")
		return lifecycleSignedHeartbeatCandidateRequest{}, false
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return lifecycleSignedHeartbeatCandidateRequest{}, false
	}
	var request lifecycleSignedHeartbeatCandidateRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return lifecycleSignedHeartbeatCandidateRequest{}, false
	}
	if !hasExactRequiredFields(body, "device_id", "challenge", "signed_proof", "heartbeat", "heartbeat_sha256", "lease_ttl_ms", "expected_device_revision") ||
		request.DeviceID == "" || request.LeaseTTLMS == 0 || request.ExpectedDeviceRevision == 0 {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "signed heartbeat request is invalid")
		return lifecycleSignedHeartbeatCandidateRequest{}, false
	}
	return request, true
}

func readSignedHeartbeatCurrent(
	w http.ResponseWriter,
	r *http.Request,
	store lifecycleRegistryCandidateStore,
	owner model.Owner,
	request lifecycleSignedHeartbeatCandidateRequest,
) (deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot, deviceinventory.PersistedEnrollmentHeartbeatLifecycleState, bool) {
	snapshot, err := store.ReadSnapshot(r.Context(), owner)
	if err != nil {
		writeLifecycleRegistryCandidateReadError(w, r, err)
		return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{}, false
	}
	for _, state := range snapshot.States() {
		if state.Device.DeviceID != request.DeviceID {
			continue
		}
		if state.Revision != request.ExpectedDeviceRevision {
			writeConversationError(w, r, http.StatusConflict, "heartbeat_conflict", "heartbeat is stale; refresh lifecycle state and retry")
			return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{}, false
		}
		if state.ChallengeCandidate == nil {
			writeLifecycleHeartbeatCandidateError(w, r, deviceidentity.ErrChallengeMismatch)
			return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{}, false
		}
		if state.ChallengeCandidate.Consumed {
			writeLifecycleHeartbeatCandidateError(w, r, deviceidentity.ErrChallengeReplayed)
			return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{}, false
		}
		if *state.ChallengeCandidate != request.Challenge {
			writeLifecycleHeartbeatCandidateError(w, r, deviceidentity.ErrChallengeMismatch)
			return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{}, false
		}
		return snapshot, state, true
	}
	writeConversationError(w, r, http.StatusNotFound, "device_not_found", "device is not present in the owner lifecycle registry")
	return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{}, false
}

func verifySignedHeartbeat(
	owner deviceidentity.Owner,
	current deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
	request lifecycleSignedHeartbeatCandidateRequest,
	nowMS uint64,
) (deviceidentity.Decision, deviceidentity.Proof, error) {
	decision, err := deviceidentity.VerifySignedProof(owner, current.Device, request.Challenge, request.Proof, nowMS)
	if err != nil {
		return deviceidentity.Decision{}, deviceidentity.Proof{}, err
	}
	digest, err := request.Heartbeat.Digest()
	if err != nil || digest != request.HeartbeatSHA256 || digest != request.Challenge.ChallengeSHA256 {
		return deviceidentity.Decision{}, deviceidentity.Proof{}, errSignedHeartbeatDigestMismatch
	}
	structural, err := request.Proof.StructuralProof()
	if err != nil {
		return deviceidentity.Decision{}, deviceidentity.Proof{}, err
	}
	if structural.DeviceID != request.DeviceID || request.Heartbeat.DeviceID != request.DeviceID {
		return deviceidentity.Decision{}, deviceidentity.Proof{}, deviceidentity.ErrDeviceMismatch
	}
	return decision, structural, nil
}
