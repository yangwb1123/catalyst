package appserver

import (
	"crypto/rand"
	"encoding/hex"
	"errors"
	"math"
	"net/http"
	"strings"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// lifecycleChallengeCandidatePath is an injected, owner-scoped challenge
// issuance boundary. It is never mounted by ordinary or accepted production
// route assembly while the device-fabric decisions remain Proposed.
const lifecycleChallengeCandidatePath = "/api/v1/device-enrollment-heartbeat/challenge-candidate"

const lifecycleChallengeCandidateScope = "forge:devices:lifecycle:challenge"

const lifecycleChallengeCandidateSchemaVersion = "forge.device-challenge-candidate/v1"

const (
	minLifecycleChallengeTTLMS uint64 = 1_000
	maxLifecycleChallengeTTLMS uint64 = 300_000
)

type lifecycleChallengeCandidateConfig struct {
	Enabled bool
	Random  func([]byte) error
}

type lifecycleChallengeCandidateRequest struct {
	DeviceID               string `json:"device_id"`
	HeartbeatSHA256        string `json:"heartbeat_sha256"`
	TTLMS                  uint64 `json:"ttl_ms"`
	ExpectedDeviceRevision uint64 `json:"expected_device_revision"`
}

type lifecycleChallengeCandidateResponse struct {
	SchemaVersion      string                             `json:"schema_version"`
	Owner              deviceidentity.Owner               `json:"owner"`
	DeviceID           string                             `json:"device_id"`
	Revision           uint64                             `json:"revision"`
	Challenge          deviceidentity.Challenge           `json:"challenge"`
	PreviewOnly        bool                               `json:"preview_only"`
	CandidatePublished bool                               `json:"candidate_published"`
	Authority          deviceinventory.LifecycleAuthority `json:"authority"`
}

var (
	errLifecycleChallengeDeviceMissing = errors.New("lifecycle challenge candidate device is missing")
	errLifecycleChallengeStale         = errors.New("lifecycle challenge candidate revision is stale")
	errLifecycleChallengeActive        = errors.New("lifecycle challenge candidate is still active")
	errLifecycleChallengeNotReady      = errors.New("lifecycle challenge candidate device is not ready")
)

func newLifecycleChallengeCandidateRoutes(config *lifecycleRegistryCandidateConfig) http.Handler {
	if config == nil || !config.Enabled || config.Store == nil || config.Heartbeat == nil ||
		!config.Heartbeat.Enabled || config.Challenge == nil || !config.Challenge.Enabled {
		return http.HandlerFunc(serveDisabledLifecycleRegistryCandidate)
	}
	handler := lifecycleChallengeCandidateHandler{
		store: config.Store, clock: config.Heartbeat.Now, random: config.Challenge.Random,
	}
	return authn.RequireScopes(http.HandlerFunc(handler.ServeHTTP), lifecycleChallengeCandidateScope)
}

type lifecycleChallengeCandidateHandler struct {
	store  lifecycleRegistryCandidateStore
	clock  lifecycleHeartbeatCandidateClock
	random func([]byte) error
}

func (handler lifecycleChallengeCandidateHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	request, ok := decodeLifecycleChallengeCandidateRequest(w, r)
	if !ok {
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if handler.store == nil || handler.clock == nil {
		writeConversationBackendUnavailable(w, r)
		return
	}
	nowMS, err := handler.clock(r.Context())
	if err != nil {
		writeConversationBackendUnavailable(w, r)
		return
	}
	snapshot, current, index, ok := handler.readCurrent(w, r, owner, request)
	if !ok {
		return
	}
	challenge, err := handler.issueChallenge(request, nowMS, current)
	if err != nil {
		writeLifecycleChallengeCandidateError(w, r, err)
		return
	}
	states := snapshot.States()
	states[index].ChallengeCandidate = &challenge
	published, err := handler.store.ReplaceStatesIfUnchanged(r.Context(), owner, snapshot, states)
	if err != nil {
		writeLifecycleChallengeCandidateError(w, r, err)
		return
	}
	if !published.Present() {
		writeConversationError(w, r, http.StatusBadGateway, "lifecycle_registry_invalid", "challenge candidate did not publish an image")
		return
	}
	writeLifecycleChallengeCandidateResponse(w, r, owner, request, challenge)
}

func writeLifecycleChallengeCandidateResponse(
	w http.ResponseWriter,
	r *http.Request,
	owner model.Owner,
	request lifecycleChallengeCandidateRequest,
	challenge deviceidentity.Challenge,
) {
	writeConversationJSON(w, r, http.StatusOK, lifecycleChallengeCandidateResponse{
		SchemaVersion: lifecycleChallengeCandidateSchemaVersion,
		Owner:         deviceidentity.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		DeviceID:      request.DeviceID, Revision: request.ExpectedDeviceRevision, Challenge: challenge,
		PreviewOnly: true, CandidatePublished: true, Authority: deviceinventory.LifecycleAuthority{},
	})
}

func decodeLifecycleChallengeCandidateRequest(w http.ResponseWriter, r *http.Request) (lifecycleChallengeCandidateRequest, bool) {
	if r.URL.EscapedPath() != lifecycleChallengeCandidatePath {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return lifecycleChallengeCandidateRequest{}, false
	}
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "challenge candidate does not accept query parameters")
		return lifecycleChallengeCandidateRequest{}, false
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return lifecycleChallengeCandidateRequest{}, false
	}
	var request lifecycleChallengeCandidateRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return lifecycleChallengeCandidateRequest{}, false
	}
	if !hasExactRequiredFields(body, "device_id", "heartbeat_sha256", "ttl_ms", "expected_device_revision") ||
		request.DeviceID == "" || !validLifecycleChallengeDigest(request.HeartbeatSHA256) ||
		request.TTLMS < minLifecycleChallengeTTLMS || request.TTLMS > maxLifecycleChallengeTTLMS ||
		request.ExpectedDeviceRevision == 0 {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "challenge candidate request is invalid")
		return lifecycleChallengeCandidateRequest{}, false
	}
	return request, true
}

func (handler lifecycleChallengeCandidateHandler) readCurrent(
	w http.ResponseWriter,
	r *http.Request,
	owner model.Owner,
	request lifecycleChallengeCandidateRequest,
) (deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot, deviceinventory.PersistedEnrollmentHeartbeatLifecycleState, int, bool) {
	snapshot, err := handler.store.ReadSnapshot(r.Context(), owner)
	if err != nil {
		writeLifecycleRegistryCandidateReadError(w, r, err)
		return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{}, -1, false
	}
	for index, state := range snapshot.States() {
		if state.Device.DeviceID != request.DeviceID {
			continue
		}
		if state.Revision != request.ExpectedDeviceRevision {
			writeLifecycleChallengeCandidateError(w, r, errLifecycleChallengeStale)
			return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{}, -1, false
		}
		return snapshot, state, index, true
	}
	writeLifecycleChallengeCandidateError(w, r, errLifecycleChallengeDeviceMissing)
	return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{}, -1, false
}

func (handler lifecycleChallengeCandidateHandler) issueChallenge(
	request lifecycleChallengeCandidateRequest,
	nowMS uint64,
	current deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
) (deviceidentity.Challenge, error) {
	if current.Device.ApprovalState != "approved" || current.Device.CredentialState != "active" {
		return deviceidentity.Challenge{}, errLifecycleChallengeNotReady
	}
	if current.ChallengeCandidate != nil && !current.ChallengeCandidate.Consumed && nowMS < current.ChallengeCandidate.ExpiresAtMS {
		return deviceidentity.Challenge{}, errLifecycleChallengeActive
	}
	if nowMS > math.MaxUint64-request.TTLMS {
		return deviceidentity.Challenge{}, errLifecycleChallengeStale
	}
	challengeID, err := newLifecycleChallengeID(handler.random)
	if err != nil {
		return deviceidentity.Challenge{}, err
	}
	return deviceidentity.Challenge{
		ChallengeID: challengeID, ChallengeSHA256: request.HeartbeatSHA256,
		IssuedAtMS: nowMS, ExpiresAtMS: nowMS + request.TTLMS,
	}, nil
}

func newLifecycleChallengeID(randomFn func([]byte) error) (string, error) {
	value := make([]byte, 16)
	if randomFn == nil {
		_, err := rand.Read(value)
		if err != nil {
			return "", err
		}
	} else if err := randomFn(value); err != nil {
		return "", err
	}
	return hex.EncodeToString(value), nil
}

func validLifecycleChallengeDigest(value string) bool {
	return len(value) == deviceidentity.DigestHexBytes && strings.Trim(value, "0123456789abcdef") == ""
}

func writeLifecycleChallengeCandidateError(w http.ResponseWriter, r *http.Request, err error) {
	switch {
	case errors.Is(err, errLifecycleChallengeDeviceMissing):
		writeConversationError(w, r, http.StatusNotFound, "device_not_found", "device is not present in the owner lifecycle registry")
	case errors.Is(err, errLifecycleChallengeStale), errors.Is(err, errLifecycleChallengeActive):
		writeConversationError(w, r, http.StatusConflict, "challenge_conflict", "challenge state is stale or still active")
	case errors.Is(err, errLifecycleChallengeNotReady):
		writeConversationError(w, r, http.StatusForbidden, "device_identity_rejected", "device is not approved for a challenge")
	case errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryFileCASConflict):
		writeConversationError(w, r, http.StatusConflict, "challenge_conflict", "lifecycle registry changed; retry with a fresh revision")
	case errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryFileCASWrite):
		writeConversationBackendUnavailable(w, r)
	default:
		writeConversationError(w, r, http.StatusBadGateway, "challenge_candidate_error", "challenge candidate request failed")
	}
}
