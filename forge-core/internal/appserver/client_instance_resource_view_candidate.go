package appserver

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// clientInstanceResourceViewCandidatePath is mounted only by the accepted
// device-fabric activation assembler. The ordinary production constructor
// remains closed while the inventory and execution ADR gates are closed.
const clientInstanceResourceViewCandidatePath = "/api/v1/client-instances/resource-view"

// This view reuses the explicitly separate device-read scope. The scope
// grants access to an observation response only; it never grants device
// identity, reservation, scheduling, dispatch, or execution authority.
const clientInstanceResourceViewCandidateScope = deviceInventoryReadCandidateScope

type clientInstanceResourceViewReadSource interface {
	ReadOwnedClientInstanceResourceView(
		context.Context,
		model.Owner,
	) (deviceplacement.ClientInstanceResourceViewObservation, error)
}

type clientInstanceResourceViewCandidateConfig struct {
	Enabled bool
	Source  clientInstanceResourceViewReadSource
}

// persistedInventoryClientInstanceResourceViewSource joins a read-only
// persisted-state reader with caller-declared client instances. The client
// rows are copied on construction so this adapter has no mutable authority.
type persistedInventoryClientInstanceResourceViewSource struct {
	reader    clientInstanceResourceViewStateReader
	instances []deviceplacement.ClientInstanceSessionViewInstance
}

type clientInstanceResourceViewStateReader interface {
	readStates(context.Context, model.Owner) ([]deviceinventory.PersistedInventoryState, error)
}

func newPersistedInventoryFileSetClientInstanceResourceViewSource(
	path string,
	instances []deviceplacement.ClientInstanceSessionViewInstance,
) clientInstanceResourceViewReadSource {
	fileSource := newPersistedInventoryFileSetReadSource(path, 0)
	return newPersistedInventoryClientInstanceResourceViewSource(fileSource, instances)
}

func newPersistedInventoryClientInstanceResourceViewSource(
	reader clientInstanceResourceViewStateReader,
	instances []deviceplacement.ClientInstanceSessionViewInstance,
) clientInstanceResourceViewReadSource {
	copyInstances := make([]deviceplacement.ClientInstanceSessionViewInstance, len(instances))
	for index, instance := range instances {
		copyInstances[index] = instance
		copyInstances[index].SessionIDs = append([]string(nil), instance.SessionIDs...)
	}
	return persistedInventoryClientInstanceResourceViewSource{reader: reader, instances: copyInstances}
}

func (source persistedInventoryClientInstanceResourceViewSource) ReadOwnedClientInstanceResourceView(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.ClientInstanceResourceViewObservation, error) {
	if ctx == nil {
		return deviceplacement.ClientInstanceResourceViewObservation{}, fmt.Errorf("client-instance resource view source requires a context")
	}
	if err := ctx.Err(); err != nil {
		return deviceplacement.ClientInstanceResourceViewObservation{}, err
	}
	if source.reader == nil {
		return deviceplacement.ClientInstanceResourceViewObservation{}, fmt.Errorf("client-instance resource view source is unavailable")
	}
	states, err := source.reader.readStates(ctx, owner)
	if err != nil {
		return deviceplacement.ClientInstanceResourceViewObservation{}, err
	}
	return deviceplacement.BuildPersistedInventoryClientInstanceResourceView(
		states,
		deviceinventory.SnapshotOwner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		source.instances,
	)
}

// newClientInstanceResourceViewCandidateRoutes is used by the accepted
// activation assembler and by focused fixture injection. A nil or disabled
// config remains a 404.
func newClientInstanceResourceViewCandidateRoutes(config *clientInstanceResourceViewCandidateConfig) http.Handler {
	if config == nil || !config.Enabled || config.Source == nil {
		return http.HandlerFunc(serveDisabledClientInstanceResourceViewCandidate)
	}
	return authn.RequireScopes(
		http.HandlerFunc(clientInstanceResourceViewCandidateHandler{source: config.Source}.ServeHTTP),
		clientInstanceResourceViewCandidateScope,
	)
}

func serveDisabledClientInstanceResourceViewCandidate(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, r, http.StatusNotFound, notFoundBody)
}

type clientInstanceResourceViewCandidateHandler struct {
	source clientInstanceResourceViewReadSource
}

func (handler clientInstanceResourceViewCandidateHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if r.URL.EscapedPath() != clientInstanceResourceViewCandidatePath {
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
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "client-instance resource view query is invalid")
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
	view, err := handler.source.ReadOwnedClientInstanceResourceView(r.Context(), owner)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if !validClientInstanceResourceViewCandidate(view, owner) {
		writeConversationError(w, r, http.StatusBadGateway, "client_instance_resource_view_invalid", "client-instance resource view source response is invalid")
		return
	}
	body, err := json.Marshal(view)
	if err != nil || len(body) > deviceplacement.MaxRequestBytes {
		writeConversationError(w, r, http.StatusBadGateway, "client_instance_resource_view_invalid", "client-instance resource view source response is invalid")
		return
	}
	writeJSON(w, r, http.StatusOK, append(body, '\n'))
}

func validClientInstanceResourceViewCandidate(
	value deviceplacement.ClientInstanceResourceViewObservation,
	owner model.Owner,
) bool {
	return value.Owner == (deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) &&
		value.Validate() == nil
}
