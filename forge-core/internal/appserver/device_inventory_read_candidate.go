package appserver

import (
	"context"
	"encoding/json"
	"net/http"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// deviceInventoryReadCandidatePath is the owner-scoped inventory read path.
// The ordinary server constructor leaves it unregistered; the activated
// device-fabric assembly mounts it only after the fail-closed gate accepts.
const deviceInventoryReadCandidatePath = "/api/v1/devices"

// deviceInventoryReadCandidateV2Path is a private, test-only route for the
// lossless observation envelope. It is intentionally separate from the v1
// candidate so a future production rollout cannot silently change the wire
// shape consumed by existing clients.
const deviceInventoryReadCandidateV2Path = "/api/v1/devices/observations/v2"

// deviceInventoryReadCandidateScope is separate from conversation scopes so a
// future inventory decision cannot accidentally inherit human-session access.
const deviceInventoryReadCandidateScope = "forge:devices:read"

// deviceInventoryReadSource is the only dependency of the candidate. A
// source receives the verified owner derived from the bearer claims and must
// return a complete, bounded observation. It must not be implemented by a
// caller-supplied owner or by a network-discovery side effect in this slice.
type deviceInventoryReadSource interface {
	ReadOwnedDeviceInventory(
		context.Context,
		model.Owner,
	) (deviceplacement.SessionDeviceObservationInventory, error)
}

// deviceInventoryReadV2Source is the owner-bound source for the lossless v2
// observation candidate. It receives only the verified bearer owner and must
// return a complete, bounded declaration; it cannot select a target or grant
// execution authority.
type deviceInventoryReadV2Source interface {
	ReadOwnedDeviceInventoryV2(
		context.Context,
		model.Owner,
	) (deviceplacement.SessionDeviceObservationInventoryV2, error)
}

// deviceInventoryReadCandidateConfig is intentionally private and injected
// only by focused tests. Enabled must be explicit and a source must be
// present before the candidate is installed. A missing or disabled config
// returns the same unregistered 404 surface as production, so a future
// inventory source cannot become reachable through a zero-value default.
type deviceInventoryReadCandidateConfig struct {
	Enabled bool
	Source  deviceInventoryReadSource
}

type deviceInventoryReadCandidateV2Config struct {
	Enabled bool
	Source  deviceInventoryReadV2Source
}

// newDeviceInventoryReadCandidateRoutes constructs an authenticated,
// owner-scoped read handler. The ordinary production constructor does not call
// it; device_inventory_activation_routes.go mounts it only with an accepted
// activation and a private lifecycle source. The nil/disabled path remains
// fail-closed.
func newDeviceInventoryReadCandidateRoutes(config *deviceInventoryReadCandidateConfig) http.Handler {
	if config == nil || !config.Enabled || config.Source == nil {
		return http.HandlerFunc(serveDisabledDeviceInventoryReadCandidate)
	}
	return authn.RequireScopes(
		http.HandlerFunc(deviceInventoryReadCandidateHandler{source: config.Source}.ServeHTTP),
		deviceInventoryReadCandidateScope,
	)
}

// newDeviceInventoryReadCandidateV2Routes constructs the lossless v2 handler.
// It is mounted only by the accepted device-fabric assembly; the zero-value
// and disabled paths remain the same unregistered 404 surface.
func newDeviceInventoryReadCandidateV2Routes(config *deviceInventoryReadCandidateV2Config) http.Handler {
	if config == nil || !config.Enabled || config.Source == nil {
		return http.HandlerFunc(serveDisabledDeviceInventoryReadCandidate)
	}
	return authn.RequireScopes(
		http.HandlerFunc(deviceInventoryReadCandidateV2Handler{source: config.Source}.ServeHTTP),
		deviceInventoryReadCandidateScope,
	)
}

func serveDisabledDeviceInventoryReadCandidate(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, r, http.StatusNotFound, notFoundBody)
}

type deviceInventoryReadCandidateHandler struct {
	source deviceInventoryReadSource
}

type deviceInventoryReadCandidateV2Handler struct {
	source deviceInventoryReadV2Source
}

func (handler deviceInventoryReadCandidateHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if r.URL.EscapedPath() != deviceInventoryReadCandidatePath {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return
	}
	if r.Method != http.MethodGet {
		w.Header().Set("Allow", http.MethodGet)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	if requestHasBody(r) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "GET requests must not include a body")
		return
	}
	if _, err := parseConversationQuery(r); err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "inventory query is invalid")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if handler.source == nil {
		writeConversationBackendUnavailable(w, r)
		return
	}
	inventory, err := handler.source.ReadOwnedDeviceInventory(r.Context(), owner)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if !validDeviceInventoryReadCandidate(inventory, owner) {
		writeConversationError(w, r, http.StatusBadGateway, "device_inventory_invalid", "device inventory source response is invalid")
		return
	}
	writeDeviceInventoryReadCandidateJSON(w, r, http.StatusOK, inventory)
}

func (handler deviceInventoryReadCandidateV2Handler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if r.URL.EscapedPath() != deviceInventoryReadCandidateV2Path {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return
	}
	if r.Method != http.MethodGet {
		w.Header().Set("Allow", http.MethodGet)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	if requestHasBody(r) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "GET requests must not include a body")
		return
	}
	if _, err := parseConversationQuery(r); err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "inventory query is invalid")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if handler.source == nil {
		writeConversationBackendUnavailable(w, r)
		return
	}
	inventory, err := handler.source.ReadOwnedDeviceInventoryV2(r.Context(), owner)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if !validDeviceInventoryReadCandidateV2(inventory, owner) {
		writeConversationError(w, r, http.StatusBadGateway, "device_inventory_invalid", "device inventory source response is invalid")
		return
	}
	writeDeviceInventoryReadCandidateJSON(w, r, http.StatusOK, inventory)
}

func writeDeviceInventoryReadCandidateJSON(w http.ResponseWriter, r *http.Request, status int, value any) {
	body, err := json.Marshal(value)
	if err != nil || len(body) > deviceplacement.MaxRequestBytes {
		writeConversationError(w, r, http.StatusBadGateway, "device_inventory_invalid", "device inventory source response is invalid")
		return
	}
	writeJSON(w, r, status, append(body, '\n'))
}

func validDeviceInventoryReadCandidate(
	value deviceplacement.SessionDeviceObservationInventory,
	owner model.Owner,
) bool {
	declaredOwner := deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
	if value.Owner != declaredOwner {
		return false
	}
	if deviceplacement.ValidateSessionDeviceObservationInventory(value) != nil {
		return false
	}
	for _, candidate := range value.Devices {
		device := candidate.Device
		if device.SnapshotObservedAtMS < 0 || device.SnapshotObservedAtMS > deviceplacement.MaxSafeIntegerMS ||
			device.LeaseExpiresAtMS < 0 || device.LeaseExpiresAtMS > deviceplacement.MaxSafeIntegerMS ||
			uint64(device.AvailableCPUCores) > uint64(deviceplacement.MaxSafeIntegerMS) ||
			device.AvailableMemoryBytes > uint64(deviceplacement.MaxSafeIntegerMS) ||
			device.AvailableStorage > uint64(deviceplacement.MaxSafeIntegerMS) ||
			device.GPU.MemoryBytes > uint64(deviceplacement.MaxSafeIntegerMS) {
			return false
		}
	}
	return true
}

func validDeviceInventoryReadCandidateV2(
	value deviceplacement.SessionDeviceObservationInventoryV2,
	owner model.Owner,
) bool {
	declaredOwner := deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
	return value.Owner == declaredOwner && deviceplacement.ValidateSessionDeviceObservationInventoryV2(value) == nil
}
