package appserver

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"net/http"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/executionlease"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// schedulerSelectionLeasePath is mounted only by the explicitly configured
// EXECUTE+P4 assembly. It creates a durable fenced reservation, but does not
// authorize a command, contact a Runner, or publish an audit event.
const schedulerSelectionLeasePath = "/api/v1/device-placement/scheduler-lease"

const schedulerSelectionLeaseRenewalPath = "/api/v1/device-placement/scheduler-lease/renew"

const schedulerSelectionLeaseReleasePath = "/api/v1/device-placement/scheduler-lease/release"

const schedulerSelectionLeaseScope = "forge:devices:placement:lease"

type schedulerSelectionLeaseConfig struct {
	Enabled      bool
	Source       deviceInventoryReadV2Source
	PolicySource devicePlacementPolicyReadSource
	Now          devicePlacementRegistryCandidateClock
	RegistryPath string
	Backend      conversationBackend
}

type devicePlacementPolicyReadSource interface {
	ReadOwnedDevicePlacementPolicy(context.Context, model.Owner) (deviceplacement.PlacementPolicyRegistry, error)
}

type schedulerSelectionLeaseRequest struct {
	ConversationID string                       `json:"conversation_id"`
	RunID          string                       `json:"run_id"`
	AttemptID      string                       `json:"attempt_id"`
	Requirements   deviceplacement.Requirements `json:"requirements"`
	TTLMS          uint64                       `json:"ttl_ms"`
}

type schedulerSelectionLeaseRenewalRequest struct {
	ConversationID string `json:"conversation_id"`
	RunID          string `json:"run_id"`
	AttemptID      string `json:"attempt_id"`
	TargetID       string `json:"target_id"`
	Epoch          uint64 `json:"epoch"`
	FencingToken   string `json:"fencing_token"`
	TTLMS          uint64 `json:"ttl_ms"`
}

type schedulerSelectionLeaseReleaseRequest struct {
	ConversationID string `json:"conversation_id"`
	RunID          string `json:"run_id"`
	AttemptID      string `json:"attempt_id"`
	TargetID       string `json:"target_id"`
	Epoch          uint64 `json:"epoch"`
	FencingToken   string `json:"fencing_token"`
}

type schedulerSelectionLeaseAuthority struct {
	PlacementSelected   bool `json:"placement_selected"`
	ReservationCreated  bool `json:"reservation_created"`
	LeaseIssued         bool `json:"lease_issued"`
	ExecutionAuthorized bool `json:"execution_authorized"`
	DispatchPerformed   bool `json:"dispatch_performed"`
	AuditPublished      bool `json:"audit_published"`
}

type schedulerSelectionLeaseResponse struct {
	SchemaVersion     string                           `json:"schema_version"`
	EvaluationMode    string                           `json:"evaluation_mode"`
	Owner             deviceplacement.Owner            `json:"owner"`
	ConversationID    string                           `json:"conversation_id"`
	RunID             string                           `json:"run_id"`
	AttemptID         string                           `json:"attempt_id"`
	DeviceID          string                           `json:"device_id"`
	InstanceID        string                           `json:"instance_id"`
	InventoryRevision uint64                           `json:"inventory_revision"`
	Generation        uint64                           `json:"generation"`
	HeartbeatSequence uint64                           `json:"heartbeat_sequence"`
	Grant             executionlease.LeaseGrant        `json:"grant"`
	Replayed          bool                             `json:"replayed"`
	Authority         schedulerSelectionLeaseAuthority `json:"authority"`
}

type schedulerSelectionLeaseReleaseResponse struct {
	SchemaVersion  string                           `json:"schema_version"`
	EvaluationMode string                           `json:"evaluation_mode"`
	Owner          deviceplacement.Owner            `json:"owner"`
	ConversationID string                           `json:"conversation_id"`
	RunID          string                           `json:"run_id"`
	AttemptID      string                           `json:"attempt_id"`
	DeviceID       string                           `json:"device_id"`
	InstanceID     string                           `json:"instance_id"`
	Epoch          uint64                           `json:"epoch"`
	ReleasedAtMS   uint64                           `json:"released_at_ms"`
	Replayed       bool                             `json:"replayed"`
	Authority      schedulerSelectionLeaseAuthority `json:"authority"`
}

func newSchedulerSelectionLeaseRoutes(config *schedulerSelectionLeaseConfig) http.Handler {
	if config == nil || !config.Enabled || config.Source == nil || config.Now == nil || config.RegistryPath == "" {
		return http.HandlerFunc(serveDisabledSchedulerSelectionLease)
	}
	return authn.RequireScopes(http.HandlerFunc(schedulerSelectionLeaseHandler{
		source: config.Source, policySource: config.PolicySource, now: config.Now, registryPath: config.RegistryPath,
		backend: config.Backend,
	}.ServeHTTP), schedulerSelectionLeaseScope)
}

func newSchedulerSelectionLeaseRenewalRoutes(config *schedulerSelectionLeaseConfig) http.Handler {
	if config == nil || !config.Enabled || config.Now == nil || config.RegistryPath == "" {
		return http.HandlerFunc(serveDisabledSchedulerSelectionLeaseRenewal)
	}
	return authn.RequireScopes(http.HandlerFunc(schedulerSelectionLeaseRenewalHandler{
		now: config.Now, registryPath: config.RegistryPath, backend: config.Backend,
	}.ServeHTTP), schedulerSelectionLeaseScope)
}

func newSchedulerSelectionLeaseReleaseRoutes(config *schedulerSelectionLeaseConfig) http.Handler {
	if config == nil || !config.Enabled || config.Now == nil || config.RegistryPath == "" {
		return http.HandlerFunc(serveDisabledSchedulerSelectionLeaseRelease)
	}
	return authn.RequireScopes(http.HandlerFunc(schedulerSelectionLeaseReleaseHandler{
		now: config.Now, registryPath: config.RegistryPath,
	}.ServeHTTP), schedulerSelectionLeaseScope)
}

func serveDisabledSchedulerSelectionLease(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, r, http.StatusNotFound, notFoundBody)
}

func serveDisabledSchedulerSelectionLeaseRenewal(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, r, http.StatusNotFound, notFoundBody)
}

func serveDisabledSchedulerSelectionLeaseRelease(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, r, http.StatusNotFound, notFoundBody)
}

type schedulerSelectionLeaseHandler struct {
	source       deviceInventoryReadV2Source
	policySource devicePlacementPolicyReadSource
	now          devicePlacementRegistryCandidateClock
	registryPath string
	backend      conversationBackend
}

type schedulerSelectionLeaseRenewalHandler struct {
	now          devicePlacementRegistryCandidateClock
	registryPath string
	backend      conversationBackend
}

type schedulerSelectionLeaseReleaseHandler struct {
	now          devicePlacementRegistryCandidateClock
	registryPath string
}

func (handler schedulerSelectionLeaseHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if r.URL.EscapedPath() != schedulerSelectionLeasePath {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return
	}
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "scheduler lease does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	idempotencyKey, err := requestIdempotencyKey(r)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	var request schedulerSelectionLeaseRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if !hasExactRequiredFields(body, "conversation_id", "run_id", "attempt_id", "requirements", "ttl_ms") || request.TTLMS == 0 {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "scheduler lease request is invalid")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if err := verifyOwnedRunReference(r.Context(), handler.backend, owner, request.ConversationID, request.RunID); err != nil {
		writeOwnedRunReferenceError(w, r, err)
		return
	}
	evaluatedAtMS, err := handler.now(r.Context())
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if evaluatedAtMS <= 0 || evaluatedAtMS > deviceplacement.MaxSafeIntegerMS {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "scheduler lease clock response is invalid")
		return
	}
	observation, err := handler.source.ReadOwnedDeviceInventoryV2(r.Context(), owner)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if !validDeviceInventoryReadCandidateV2(observation, owner) {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "scheduler lease source response is invalid")
		return
	}
	evaluationOwner := deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
	var placement deviceplacement.PersistedInventoryPlacementV2Evaluation
	if handler.policySource == nil {
		placement, err = deviceplacement.EvaluatePersistedInventoryObservationV2(
			observation, evaluationOwner, request.Requirements, evaluatedAtMS,
		)
	} else {
		policy, policyErr := handler.policySource.ReadOwnedDevicePlacementPolicy(r.Context(), owner)
		if policyErr != nil {
			writeConversationBackendError(w, r, policyErr)
			return
		}
		placement, err = deviceplacement.EvaluatePolicyCompleteInventoryObservationV2(
			observation, policy, evaluationOwner, request.Requirements, evaluatedAtMS,
		)
	}
	if err != nil {
		if errors.Is(err, deviceplacement.ErrPlacementPolicyRegistryBinding) {
			writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "scheduler policy source does not match the inventory observation")
			return
		}
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "scheduler lease request is invalid")
		return
	}
	candidates := make([]executionlease.ClaimCandidate, 0, placement.EligibleCandidateCount)
	for _, decision := range placement.Decisions {
		if !decision.MatchesRequirements {
			continue
		}
		candidates = append(candidates, executionlease.ClaimCandidate{
			DeviceID: decision.DeviceID, InstanceID: decision.InstanceID,
			Revision: decision.Revision, Generation: decision.Generation,
			HeartbeatSequence: decision.HeartbeatSequence,
		})
	}
	registry, err := deviceinventory.NewPersistedExecutionLeaseRegistryFileSetWriteAdapter(
		handler.registryPath,
		deviceidentity.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
	)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	// Bind idempotency to the strict decoded request rather than the wire's
	// object-member order. Go, Rust, and Flutter may serialize the same JSON
	// object with different key order while preserving its meaning; retries
	// from another client must still replay the original fenced grant. The
	// strict decoder above has already rejected unknown, duplicate, and
	// trailing fields, so this marshal is the one canonical request image used
	// by every authenticated adapter.
	canonicalRequest, err := json.Marshal(request)
	if err != nil {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "scheduler lease request cannot be canonicalized")
		return
	}
	requestDigest := sha256.Sum256(canonicalRequest)
	claim, replayed, err := registry.Claim(r.Context(), executionlease.ClaimRequest{
		ConversationID: request.ConversationID, RunID: request.RunID, AttemptID: request.AttemptID,
		IdempotencyKey: idempotencyKey, RequestSHA256: hex.EncodeToString(requestDigest[:]),
		IssuedAtMS: uint64(evaluatedAtMS), TTLMS: request.TTLMS,
	}, candidates)
	if err != nil {
		schedulerSelectionLeaseError(w, r, err)
		return
	}
	response := schedulerSelectionLeaseResponse{
		SchemaVersion:  executionlease.RegistrySchemaVersion,
		EvaluationMode: executionlease.RegistryEvaluationMode,
		Owner:          deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		ConversationID: claim.ConversationID, RunID: claim.RunID, AttemptID: claim.AttemptID,
		DeviceID: claim.DeviceID, InstanceID: claim.InstanceID,
		InventoryRevision: claim.Revision, Generation: claim.Generation,
		HeartbeatSequence: claim.HeartbeatSequence, Grant: claim.Grant, Replayed: replayed,
		Authority: schedulerSelectionLeaseAuthority{
			PlacementSelected: true, ReservationCreated: true, LeaseIssued: true,
		},
	}
	encoded, err := json.Marshal(response)
	if err != nil {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "scheduler lease response is invalid")
		return
	}
	writeJSON(w, r, http.StatusOK, append(encoded, '\n'))
}

func (handler schedulerSelectionLeaseRenewalHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if r.URL.EscapedPath() != schedulerSelectionLeaseRenewalPath {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return
	}
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "scheduler lease renewal does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	idempotencyKey, err := requestIdempotencyKey(r)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	var request schedulerSelectionLeaseRenewalRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if !hasExactRequiredFields(
		body,
		"conversation_id", "run_id", "attempt_id", "target_id", "epoch", "fencing_token", "ttl_ms",
	) || request.TTLMS == 0 {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "scheduler lease renewal request is invalid")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	// Renewal extends a device reservation, so it must still be tied to a
	// durable owner-scoped Run. A nil backend remains valid for value-only
	// route tests; the production EXECUTE assembly supplies the Runtime
	// backend. Release deliberately has no such read so deleting a Run cannot
	// strand the cleanup path for an already-issued lease.
	if err := verifyOwnedRunReference(r.Context(), handler.backend, owner, request.ConversationID, request.RunID); err != nil {
		writeOwnedRunReferenceError(w, r, err)
		return
	}
	evaluatedAtMS, err := handler.now(r.Context())
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if evaluatedAtMS <= 0 || evaluatedAtMS > deviceplacement.MaxSafeIntegerMS {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "scheduler lease renewal clock response is invalid")
		return
	}
	canonicalRequest, err := json.Marshal(request)
	if err != nil {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "scheduler lease renewal request cannot be canonicalized")
		return
	}
	digest := sha256.Sum256(canonicalRequest)
	registry, err := deviceinventory.NewPersistedExecutionLeaseRegistryFileSetWriteAdapter(
		handler.registryPath,
		deviceidentity.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
	)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	entry, replayed, err := registry.Renew(r.Context(), executionlease.RenewRequest{
		ConversationID: request.ConversationID, RunID: request.RunID, AttemptID: request.AttemptID,
		Proof: executionlease.LeaseProof{
			AttemptID: request.AttemptID, TargetID: request.TargetID,
			Epoch: request.Epoch, FencingToken: request.FencingToken,
		},
		IdempotencyKey: idempotencyKey, RequestSHA256: hex.EncodeToString(digest[:]),
		IssuedAtMS: uint64(evaluatedAtMS), TTLMS: request.TTLMS,
	})
	if err != nil {
		schedulerSelectionLeaseError(w, r, err)
		return
	}
	response := schedulerSelectionLeaseResponse{
		SchemaVersion: executionlease.RegistrySchemaVersion, EvaluationMode: executionlease.RegistryEvaluationMode,
		Owner:          deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		ConversationID: entry.ConversationID, RunID: entry.RunID, AttemptID: entry.AttemptID,
		DeviceID: entry.DeviceID, InstanceID: entry.InstanceID,
		InventoryRevision: entry.Revision, Generation: entry.Generation,
		HeartbeatSequence: entry.HeartbeatSequence, Grant: entry.Grant, Replayed: replayed,
		Authority: schedulerSelectionLeaseAuthority{PlacementSelected: true, ReservationCreated: true, LeaseIssued: true},
	}
	encoded, err := json.Marshal(response)
	if err != nil {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "scheduler lease renewal response is invalid")
		return
	}
	writeJSON(w, r, http.StatusOK, append(encoded, '\n'))
}

func (handler schedulerSelectionLeaseReleaseHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if r.URL.EscapedPath() != schedulerSelectionLeaseReleasePath {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return
	}
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "scheduler lease release does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	idempotencyKey, err := requestIdempotencyKey(r)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	var request schedulerSelectionLeaseReleaseRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if !hasExactRequiredFields(body, "conversation_id", "run_id", "attempt_id", "target_id", "epoch", "fencing_token") {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "scheduler lease release request is invalid")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	releasedAtMS, err := handler.now(r.Context())
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if releasedAtMS <= 0 || releasedAtMS > deviceplacement.MaxSafeIntegerMS {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "scheduler lease release clock response is invalid")
		return
	}
	canonicalRequest, err := json.Marshal(request)
	if err != nil {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "scheduler lease release request cannot be canonicalized")
		return
	}
	digest := sha256.Sum256(canonicalRequest)
	registry, err := deviceinventory.NewPersistedExecutionLeaseRegistryFileSetWriteAdapter(
		handler.registryPath,
		deviceidentity.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
	)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	entry, replayed, err := registry.Release(r.Context(), executionlease.ReleaseRequest{
		ConversationID: request.ConversationID, RunID: request.RunID, AttemptID: request.AttemptID,
		Proof: executionlease.LeaseProof{
			AttemptID: request.AttemptID, TargetID: request.TargetID,
			Epoch: request.Epoch, FencingToken: request.FencingToken,
		},
		IdempotencyKey: idempotencyKey, RequestSHA256: hex.EncodeToString(digest[:]), ReleasedAtMS: uint64(releasedAtMS),
	})
	if err != nil {
		schedulerSelectionLeaseError(w, r, err)
		return
	}
	response := schedulerSelectionLeaseReleaseResponse{
		SchemaVersion:  executionlease.RegistrySchemaVersion,
		EvaluationMode: executionlease.RegistryReleaseEvaluationMode,
		Owner:          deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		ConversationID: entry.ConversationID, RunID: entry.RunID, AttemptID: entry.AttemptID,
		DeviceID: entry.DeviceID, InstanceID: entry.InstanceID, Epoch: entry.Grant.Epoch,
		ReleasedAtMS: entry.ReleasedAtMS, Replayed: replayed,
	}
	encoded, err := json.Marshal(response)
	if err != nil {
		writeConversationError(w, r, http.StatusBadGateway, "device_placement_invalid", "scheduler lease release response is invalid")
		return
	}
	writeJSON(w, r, http.StatusOK, append(encoded, '\n'))
}

func schedulerSelectionLeaseError(w http.ResponseWriter, r *http.Request, err error) {
	switch {
	case errors.Is(err, executionlease.ErrIdempotencyConflict):
		writeConversationError(w, r, http.StatusConflict, "idempotency_conflict", "idempotency key conflicts with an existing lease")
	case errors.Is(err, executionlease.ErrTargetReserved):
		writeConversationError(w, r, http.StatusConflict, "target_reserved", "all eligible targets are currently reserved")
	case errors.Is(err, executionlease.ErrNoEligibleTarget):
		writeConversationError(w, r, http.StatusConflict, "no_eligible_target", "no eligible target is available")
	case errors.Is(err, executionlease.ErrInvalidClaimRequest):
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "scheduler lease request is invalid")
	case errors.Is(err, executionlease.ErrInvalidRenewRequest):
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "scheduler lease renewal request is invalid")
	case errors.Is(err, executionlease.ErrLeaseNotFound):
		writeConversationError(w, r, http.StatusConflict, "lease_not_found", "scheduler lease proof is not registered")
	case errors.Is(err, executionlease.ErrLeaseStale):
		writeConversationError(w, r, http.StatusConflict, "lease_stale", "scheduler lease proof is stale")
	case errors.Is(err, executionlease.ErrLeaseExpired):
		writeConversationError(w, r, http.StatusConflict, "lease_expired", "scheduler lease has expired")
	case errors.Is(err, executionlease.ErrInvalidReleaseRequest):
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "scheduler lease release request is invalid")
	case errors.Is(err, executionlease.ErrLeaseReleased):
		writeConversationError(w, r, http.StatusConflict, "lease_released", "scheduler lease is already released")
	default:
		writeConversationBackendError(w, r, err)
	}
}

func validSchedulerSelectionLeaseOwner(value model.Owner, owner deviceidentity.Owner) bool {
	return value == (model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID})
}
