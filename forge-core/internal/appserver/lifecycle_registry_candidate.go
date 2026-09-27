package appserver

import (
	"context"
	"errors"
	"fmt"
	"net/http"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/executionprofile"
	"forgeos/forge-core/internal/runtimebridge"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// lifecycleRegistryCandidatePath is a private, test-only transport for one
// complete owner-scoped enrollment/heartbeat lifecycle image.  It is not
// registered by the production server while the lifecycle and inventory ADR
// gates remain closed.
const lifecycleRegistryCandidatePath = "/api/v1/device-enrollment-heartbeat/lifecycle-registry"

// lifecycleApprovalCandidatePath is an explicitly injected owner approval
// plan boundary. It is never mounted by production route construction.
const lifecycleApprovalCandidatePath = "/api/v1/device-enrollment-heartbeat/approval-candidate"

// The lifecycle image contains device identity and resource observations, so
// it deliberately does not inherit the conversation or generic inventory
// scopes.  GET and PUT use separate scopes even though both are candidate-only.
const (
	lifecycleRegistryCandidateReadScope  = "forge:devices:lifecycle:read"
	lifecycleRegistryCandidateWriteScope = "forge:devices:lifecycle:write"
	lifecycleApprovalCandidateScope      = "forge:devices:lifecycle:approval"
)

// lifecycleRegistryCandidateStore is an owner-bound value adapter.  The
// authenticated owner is supplied on every operation so an injected store
// cannot select an owner from the request body or retain a caller's token.
// The snapshot is opaque and is only used for the store's exact-image CAS.
type lifecycleRegistryCandidateStore interface {
	ReadSnapshot(context.Context, model.Owner) (deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot, error)
	ReplaceStatesIfUnchanged(
		context.Context,
		model.Owner,
		deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot,
		[]deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
	) (deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot, error)
}

type lifecycleRegistryCandidateConfig struct {
	Enabled    bool
	Store      lifecycleRegistryCandidateStore
	Heartbeat  *lifecycleHeartbeatCandidateConfig
	Challenge  *lifecycleChallengeCandidateConfig
	Approval   *lifecycleApprovalCandidateConfig
	Credential *lifecycleCredentialCandidateConfig
}

// persistedLifecycleRegistryCandidateStore bridges the already reviewed
// private-file CAS adapter to this candidate HTTP surface.  It creates no
// directories, listener, credential, clock, or inventory authority.
type persistedLifecycleRegistryCandidateStore struct {
	path string
}

func newPersistedLifecycleRegistryCandidateStore(path string) lifecycleRegistryCandidateStore {
	return persistedLifecycleRegistryCandidateStore{path: path}
}

func (store persistedLifecycleRegistryCandidateStore) adapter(owner model.Owner) (deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter, error) {
	return deviceinventory.NewPersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter(
		store.path,
		deviceidentity.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
	)
}

func (store persistedLifecycleRegistryCandidateStore) ReadSnapshot(
	ctx context.Context,
	owner model.Owner,
) (deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot, error) {
	if ctx == nil {
		return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, fmt.Errorf("lifecycle registry candidate store requires a context")
	}
	if err := ctx.Err(); err != nil {
		return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, err
	}
	adapter, err := store.adapter(owner)
	if err != nil {
		return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, err
	}
	return adapter.ReadSnapshot()
}

func (store persistedLifecycleRegistryCandidateStore) ReplaceStatesIfUnchanged(
	ctx context.Context,
	owner model.Owner,
	snapshot deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot,
	next []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
) (deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot, error) {
	if ctx == nil {
		return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, fmt.Errorf("lifecycle registry candidate store requires a context")
	}
	if err := ctx.Err(); err != nil {
		return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, err
	}
	adapter, err := store.adapter(owner)
	if err != nil {
		return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, err
	}
	return adapter.ReplaceStatesIfUnchanged(snapshot, next)
}

// lifecycleRegistryCandidateEnvelope is intentionally the same value shape
// as the private file envelope, with no CAS token or authority marker exposed
// over HTTP.  A subsequent PUT re-reads the current image and lets the
// injected adapter perform exact-image CAS before replacing the full set.
type lifecycleRegistryCandidateEnvelope struct {
	SchemaVersion string                                                       `json:"schema_version"`
	Owner         deviceidentity.Owner                                         `json:"owner"`
	States        []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState `json:"states"`
}

type lifecycleRegistryCandidateWriteRequest struct {
	States []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState `json:"states"`
}

// newLifecycleRegistryCandidateRoutes constructs the candidate only for
// focused tests and explicit fixture injection.  A missing or disabled config
// preserves the ordinary 404 response and does not consult the store.
func newLifecycleRegistryCandidateRoutes(config *lifecycleRegistryCandidateConfig) http.Handler {
	if config == nil || !config.Enabled || config.Store == nil {
		return http.HandlerFunc(serveDisabledLifecycleRegistryCandidate)
	}
	registry := requireLifecycleRegistryCandidateScopes(
		http.HandlerFunc(lifecycleRegistryCandidateHandler{store: config.Store}.ServeHTTP),
	)
	heartbeat := newLifecycleHeartbeatCandidateRoutes(config)
	signedHeartbeat := newLifecycleSignedHeartbeatCandidateRoutes(config)
	challenge := newLifecycleChallengeCandidateRoutes(config)
	approval := newLifecycleApprovalCandidateRoutes(config)
	credential := newLifecycleCredentialCandidateRoutes(config)
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.EscapedPath() == lifecycleHeartbeatCandidatePath {
			heartbeat.ServeHTTP(w, r)
			return
		}
		if r.URL.EscapedPath() == lifecycleSignedHeartbeatCandidatePath {
			signedHeartbeat.ServeHTTP(w, r)
			return
		}
		if r.URL.EscapedPath() == lifecycleChallengeCandidatePath {
			challenge.ServeHTTP(w, r)
			return
		}
		if r.URL.EscapedPath() == lifecycleApprovalCandidatePath {
			approval.ServeHTTP(w, r)
			return
		}
		if r.URL.EscapedPath() == lifecycleCredentialCandidatePath {
			credential.ServeHTTP(w, r)
			return
		}
		registry.ServeHTTP(w, r)
	})
}

// newActivatedLifecycleRegistryReadRoutes exposes only the owner-scoped GET
// half of the lifecycle image for an Accepted Fabric assembly. It reuses the
// strict candidate decoder and private-file adapter while filtering PUT and
// every other method before the candidate handler can reach a write path.
// Enrollment, heartbeat, approval, and credential routes are never composed.
func newActivatedLifecycleRegistryReadRoutes(path string) http.Handler {
	if path == "" {
		return http.HandlerFunc(serveDisabledLifecycleRegistryCandidate)
	}
	registry := lifecycleRegistryCandidateHandler{
		store: newPersistedLifecycleRegistryCandidateStore(path),
	}
	return authn.RequireScopes(
		http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if r.Method != http.MethodGet {
				writeJSON(w, r, http.StatusNotFound, notFoundBody)
				return
			}
			registry.ServeHTTP(w, r)
		}),
		lifecycleRegistryCandidateReadScope,
	)
}

// newAuthenticatedSessionRoutesWithLifecycleRegistryCandidate is a focused
// test constructor.  Production uses newAuthenticatedSessionRoutes and never
// calls this function; the candidate is mounted by path on the returned test
// handler only.
func newAuthenticatedSessionRoutesWithLifecycleRegistryCandidate(
	client *runtimebridge.Client,
	profiles *executionprofile.Catalog,
	config *lifecycleRegistryCandidateConfig,
) http.Handler {
	sessions := newAuthenticatedSessionRoutesWithObservationCandidates(client, profiles)
	candidate := newLifecycleRegistryCandidateRoutes(config)
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if isLifecycleRegistryCandidatePath(r.URL.EscapedPath()) {
			candidate.ServeHTTP(w, r)
			return
		}
		sessions.ServeHTTP(w, r)
	})
}

func isLifecycleRegistryCandidatePath(path string) bool {
	return path == lifecycleRegistryCandidatePath || path == lifecycleHeartbeatCandidatePath || path == lifecycleSignedHeartbeatCandidatePath || path == lifecycleChallengeCandidatePath || path == lifecycleApprovalCandidatePath || path == lifecycleCredentialCandidatePath
}

func serveDisabledLifecycleRegistryCandidate(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, r, http.StatusNotFound, notFoundBody)
}

// requireLifecycleRegistryCandidateScopes keeps read and replacement access
// separate while using the existing authn scope middleware.  Scope selection
// happens after the bearer token has been validated by authn.Handler.
func requireLifecycleRegistryCandidateScopes(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		scope := lifecycleRegistryCandidateReadScope
		if r.Method == http.MethodPut {
			scope = lifecycleRegistryCandidateWriteScope
		}
		authn.RequireScopes(next, scope).ServeHTTP(w, r)
	})
}

type lifecycleRegistryCandidateHandler struct {
	store lifecycleRegistryCandidateStore
}

func (handler lifecycleRegistryCandidateHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if r.URL.EscapedPath() != lifecycleRegistryCandidatePath {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return
	}
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "lifecycle registry candidate does not accept query parameters")
		return
	}
	switch r.Method {
	case http.MethodGet:
		handler.serveRead(w, r)
	case http.MethodPut:
		handler.serveReplace(w, r)
	default:
		w.Header().Set("Allow", "GET, PUT")
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
	}
}

func (handler lifecycleRegistryCandidateHandler) serveRead(w http.ResponseWriter, r *http.Request) {
	if requestHasBody(r) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "GET requests must not include a body")
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
	snapshot, err := handler.store.ReadSnapshot(r.Context(), owner)
	if err != nil {
		writeLifecycleRegistryCandidateReadError(w, r, err)
		return
	}
	if !snapshot.Present() {
		writeConversationError(w, r, http.StatusNotFound, "lifecycle_registry_missing", "lifecycle registry image is not present")
		return
	}
	envelope, err := lifecycleRegistryCandidateEnvelopeFor(owner, snapshot.States())
	if err != nil {
		writeConversationError(w, r, http.StatusBadGateway, "lifecycle_registry_invalid", "lifecycle registry source response is invalid")
		return
	}
	writeConversationJSON(w, r, http.StatusOK, envelope)
}

func (handler lifecycleRegistryCandidateHandler) serveReplace(w http.ResponseWriter, r *http.Request) {
	var request lifecycleRegistryCandidateWriteRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	if !hasExactRequiredFields(body, "states") || request.States == nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "lifecycle registry replacement is invalid")
		return
	}
	for _, state := range request.States {
		if state.ChallengeCandidate != nil {
			writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "challenge candidates must use the challenge candidate route")
			return
		}
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
	snapshot, err := handler.store.ReadSnapshot(r.Context(), owner)
	if err != nil {
		writeLifecycleRegistryCandidateReadError(w, r, err)
		return
	}
	published, err := handler.store.ReplaceStatesIfUnchanged(r.Context(), owner, snapshot, request.States)
	if err != nil {
		writeLifecycleRegistryCandidateReplaceError(w, r, err)
		return
	}
	if !published.Present() {
		writeConversationError(w, r, http.StatusBadGateway, "lifecycle_registry_invalid", "lifecycle registry replacement did not publish an image")
		return
	}
	envelope, err := lifecycleRegistryCandidateEnvelopeFor(owner, published.States())
	if err != nil {
		writeConversationError(w, r, http.StatusBadGateway, "lifecycle_registry_invalid", "lifecycle registry replacement is invalid")
		return
	}
	writeConversationJSON(w, r, http.StatusOK, envelope)
}

func lifecycleRegistryCandidateEnvelopeFor(
	owner model.Owner,
	states []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
) (lifecycleRegistryCandidateEnvelope, error) {
	deviceOwner := deviceidentity.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
	value := deviceinventory.PersistedEnrollmentHeartbeatLifecycleRegistryState{
		Owner:  deviceOwner,
		States: states,
	}
	canonical, err := deviceinventory.CanonicalizePersistedEnrollmentHeartbeatLifecycleRegistry(value)
	if err != nil {
		return lifecycleRegistryCandidateEnvelope{}, err
	}
	// Keep the existing lifecycle-registry GET/PUT wire contract closed for
	// Rust/Flutter consumers. Approval plans have their own candidate response
	// and are read by their owner-scoped handler; they are stored in the
	// injected file image but are not projected as live lifecycle state.
	for index := range canonical.States {
		canonical.States[index].ApprovalCandidate = nil
		canonical.States[index].CredentialCandidate = nil
		canonical.States[index].ChallengeCandidate = nil
	}
	return lifecycleRegistryCandidateEnvelope{
		SchemaVersion: deviceinventory.PersistedLifecycleRegistryFileSchemaVersion,
		Owner:         deviceOwner,
		States:        canonical.States,
	}, nil
}

func writeLifecycleRegistryCandidateReadError(w http.ResponseWriter, r *http.Request, err error) {
	switch {
	case errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryOwnerMismatch):
		writeConversationError(w, r, http.StatusBadGateway, "lifecycle_registry_invalid", "lifecycle registry source response is invalid")
	case errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryFileCASInvalid):
		writeConversationError(w, r, http.StatusBadGateway, "lifecycle_registry_invalid", "lifecycle registry source is unavailable")
	default:
		writeConversationError(w, r, http.StatusBadGateway, "lifecycle_registry_error", "lifecycle registry request failed")
	}
}

func writeLifecycleRegistryCandidateReplaceError(w http.ResponseWriter, r *http.Request, err error) {
	switch {
	case errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryFileCASConflict):
		writeConversationError(w, r, http.StatusConflict, "lifecycle_registry_conflict", "lifecycle registry changed; retry with a fresh image")
	case errors.Is(err, deviceinventory.ErrLifecycleRegistryOwnerMismatch):
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "lifecycle registry state owner must match the authenticated principal")
	case errors.Is(err, deviceinventory.ErrLifecycleRegistryInvalidState),
		errors.Is(err, deviceinventory.ErrLifecycleRegistryDuplicateDevice),
		errors.Is(err, deviceinventory.ErrLifecycleRegistryDuplicateRunner),
		errors.Is(err, deviceinventory.ErrLifecycleRegistryCapacityExceeded):
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "lifecycle registry replacement is invalid")
	case errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryFileCASWrite):
		writeConversationBackendUnavailable(w, r)
	default:
		writeConversationError(w, r, http.StatusBadGateway, "lifecycle_registry_error", "lifecycle registry replacement failed")
	}
}
