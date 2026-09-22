package appserver

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// clientInstanceSessionViewCandidatePath is mounted only by the accepted
// device-fabric activation assembler. The ordinary production constructor
// remains closed while instance registration and execution governance remain
// closed.
const clientInstanceSessionViewCandidatePath = "/api/v1/client-instances/session-view"

// Session view access reuses the owner-scoped conversation read scope. The
// candidate returns metadata only and never grants Prompt write, instance
// registration, device, scheduling, or execution authority.
const clientInstanceSessionViewCandidateScope = "forge:conversations:read"

type clientInstanceSessionViewReadSource interface {
	ReadOwnedClientInstanceSessionView(
		context.Context,
		model.Owner,
	) (deviceplacement.ClientInstanceSessionViewObservation, error)
}

type clientInstanceSessionViewCandidateConfig struct {
	Enabled bool
	Source  clientInstanceSessionViewReadSource
}

// newClientInstanceSessionViewCandidateRoutes is used by the accepted
// activation assembler and by focused fixture injection. A nil or disabled
// config keeps the same default-closed 404 surface as production.
func newClientInstanceSessionViewCandidateRoutes(config *clientInstanceSessionViewCandidateConfig) http.Handler {
	if config == nil || !config.Enabled || config.Source == nil {
		return http.HandlerFunc(serveDisabledClientInstanceSessionViewCandidate)
	}
	return authn.RequireScopes(
		http.HandlerFunc(clientInstanceSessionViewCandidateHandler{source: config.Source}.ServeHTTP),
		clientInstanceSessionViewCandidateScope,
	)
}

func serveDisabledClientInstanceSessionViewCandidate(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, r, http.StatusNotFound, notFoundBody)
}

type clientInstanceSessionViewCandidateHandler struct {
	source clientInstanceSessionViewReadSource
}

func (handler clientInstanceSessionViewCandidateHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if r.URL.EscapedPath() != clientInstanceSessionViewCandidatePath {
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
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "client-instance session view query is invalid")
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
	view, err := handler.source.ReadOwnedClientInstanceSessionView(r.Context(), owner)
	if err != nil {
		writeConversationBackendError(w, r, err)
		return
	}
	if !validClientInstanceSessionViewCandidate(view, owner) {
		writeConversationError(w, r, http.StatusBadGateway, "client_instance_session_view_invalid", "client-instance session view source response is invalid")
		return
	}
	body, err := json.Marshal(view)
	if err != nil || len(body) > deviceplacement.MaxRequestBytes {
		writeConversationError(w, r, http.StatusBadGateway, "client_instance_session_view_invalid", "client-instance session view source response is invalid")
		return
	}
	writeJSON(w, r, http.StatusOK, append(body, '\n'))
}

func validClientInstanceSessionViewCandidate(
	value deviceplacement.ClientInstanceSessionViewObservation,
	owner model.Owner,
) bool {
	return value.Owner == (deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) &&
		value.Validate() == nil
}

// staticClientInstanceSessionViewSource makes explicit that this candidate
// accepts an injected observation and does not discover or persist clients.
// It is useful for small integration tests without creating a second authority.
type staticClientInstanceSessionViewSource struct {
	value deviceplacement.ClientInstanceSessionViewObservation
}

func (source staticClientInstanceSessionViewSource) ReadOwnedClientInstanceSessionView(
	ctx context.Context,
	_ model.Owner,
) (deviceplacement.ClientInstanceSessionViewObservation, error) {
	if ctx == nil {
		return deviceplacement.ClientInstanceSessionViewObservation{}, fmt.Errorf("client-instance session view source requires a context")
	}
	if err := ctx.Err(); err != nil {
		return deviceplacement.ClientInstanceSessionViewObservation{}, err
	}
	return source.value, nil
}
