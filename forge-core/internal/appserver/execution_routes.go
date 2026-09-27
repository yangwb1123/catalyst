package appserver

import (
	"context"
	consentmodel "forgeos/forge-core/internal/runtimebridge/consentmodel"
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"net/http"
	"net/url"
	"strings"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/executionprofile"
)

const executionConsentCollectionPath = "/api/v1/execution-consents"

// executionBackend is the private app-server seam for Project consent and
// inert pending Run intents. Production route wiring remains behind the ADR
// lifecycle and same-tree acceptance gate; an accepted EXECUTE assembly only
// exposes admission records and still has no Runner dispatch authority.
type executionBackend interface {
	OwnedProjectConversationIdentity(
		context.Context,
		model.Owner,
		string,
	) (model.OwnedProjectConversationIdentity, error)
	GrantProjectExecutionConsent(
		context.Context,
		model.Owner,
		string,
		string,
		[32]byte,
		uint64,
		string,
	) (consentmodel.ProjectExecutionConsentGrantResult, error)
	RevokeProjectExecutionConsent(
		context.Context,
		model.Owner,
		string,
		string,
	) (consentmodel.ProjectExecutionConsentRevocationResult, error)
	SubmitOwnedPromptRunIntent(
		context.Context,
		model.Owner,
		string,
		string,
		string,
		uint64,
		intentmodel.ServerExecutionProfile,
	) (intentmodel.PendingRunIntentSubmissionResult, error)
	OwnedConversationPendingRunIntents(
		context.Context,
		model.Owner,
		string,
		*intentmodel.PendingRunIntentCursor,
		int,
	) (intentmodel.OwnedPendingRunIntentPage, error)
	OwnedConversationPendingRunIntentTimeline(
		context.Context,
		model.Owner,
		string,
		string,
		uint64,
		int,
	) (intentmodel.OwnedPendingRunIntentTimelinePage, error)
}

type executionProjectReader interface {
	OwnedProjectConversationIdentity(
		context.Context,
		model.Owner,
		string,
	) (model.OwnedProjectConversationIdentity, error)
}

// executionSurface is mounted by the dedicated accepted EXECUTE assembly. Its
// execution handler contains consent and pending-intent admission only; the
// ordinary server constructor and INVENTORY/OBSERVE assemblies keep it closed.
type executionSurface struct {
	sessions  http.Handler
	execution http.Handler
}

func (surface executionSurface) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if isExecutionRoute(r.URL.EscapedPath()) {
		surface.execution.ServeHTTP(w, r)
		return
	}
	surface.sessions.ServeHTTP(w, r)
}

func newConversationRoutesWithInertExecutionAPI(
	backend conversationBackend,
	profiles *executionprofile.Catalog,
) http.Handler {
	sessions := newConversationRoutesWithBackendAndExecutionProfiles(backend, profiles)
	return executionSurface{
		sessions: authenticatedSessionRoutes{
			conversations:                    sessions,
			placement:                        newDevicePlacementPreviewRoutes(),
			deviceObservation:                newSessionDeviceObservationRoutes(),
			sessionRunnerReceipt:             newSessionRunnerReceiptObservationRoutes(),
			runAttemptLeaseDispatchPreflight: newRunAttemptLeaseDispatchPreflightRoutes(),
			runnerDispatchPlanPreview:        newRunnerDispatchPlanPreviewRoutes(),
			runnerExecutionIntentPreview:     newRunnerExecutionIntentPreviewRoutes(),
			runExecutionEvidence:             newRunExecutionEvidencePreviewRoutes(),
			executionReconciliation:          newExecutionReconciliationPreviewRoutes(),
		},
		execution: newExecutionRoutes(backend, profiles),
	}
}

func newExecutionRoutes(backend conversationBackend, profiles *executionprofile.Catalog) http.Handler {
	implementation, ok := backend.(executionBackend)
	if !ok {
		implementation = nil
	}
	reader, _ := backend.(executionProjectReader)
	routes := executionRoutes{backend: implementation, reader: reader, profiles: profiles}
	routes.grant = authn.RequireScopes(http.HandlerFunc(routes.grantProjectConsent), "forge:conversations:write")
	routes.preview = authn.RequireScopes(http.HandlerFunc(routes.previewProjectConsent), "forge:conversations:read")
	routes.revoke = authn.RequireScopes(http.HandlerFunc(routes.revokeProjectConsent), "forge:conversations:write")
	routes.submit = authn.RequireScopes(http.HandlerFunc(routes.submitPendingIntent), "forge:conversations:write")
	routes.list = authn.RequireScopes(http.HandlerFunc(routes.listPendingIntents), "forge:conversations:read")
	routes.timeline = authn.RequireScopes(http.HandlerFunc(routes.pendingIntentTimeline), "forge:conversations:read")
	return routes
}

func isExecutionRoute(path string) bool {
	if strings.HasPrefix(path, executionConsentCollectionPath+"/") {
		grantID := strings.TrimPrefix(path, executionConsentCollectionPath+"/")
		return grantID != "" && !strings.Contains(grantID, "/")
	}
	_, suffix, ok := conversationPathSuffix(path)
	if !ok {
		return false
	}
	if suffix == "execution-consents" || suffix == "run-intents" {
		return true
	}
	parts := strings.Split(suffix, "/")
	return len(parts) == 3 && parts[0] == "run-intents" && parts[1] != "" && parts[2] == "timeline"
}

type executionRoutes struct {
	backend  executionBackend
	reader   executionProjectReader
	profiles *executionprofile.Catalog
	grant    http.Handler
	preview  http.Handler
	revoke   http.Handler
	submit   http.Handler
	list     http.Handler
	timeline http.Handler
}

func (routes executionRoutes) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if routes.serveConsentCollectionRoute(w, r, r.URL.EscapedPath()) ||
		routes.serveConversationExecutionRoute(w, r, r.URL.EscapedPath()) {
		return
	}
	writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
}

func (routes executionRoutes) serveConsentCollectionRoute(w http.ResponseWriter, r *http.Request, path string) bool {
	if strings.HasPrefix(path, executionConsentCollectionPath+"/") {
		grantID := strings.TrimPrefix(path, executionConsentCollectionPath+"/")
		if strings.Contains(grantID, "/") {
			writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
			return true
		}
		ctx := context.WithValue(r.Context(), grantIDContextKey{}, grantID)
		if r.Method != http.MethodDelete {
			w.Header().Set("Allow", http.MethodDelete)
			writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
			return true
		}
		routes.revoke.ServeHTTP(w, r.WithContext(ctx))
		return true
	}
	return false
}

func (routes executionRoutes) serveConversationExecutionRoute(w http.ResponseWriter, r *http.Request, path string) bool {
	if conversationID, suffix, ok := conversationPathSuffix(path); ok {
		ctx := context.WithValue(r.Context(), conversationIDContextKey{}, conversationID)
		switch suffix {
		case "execution-consents":
			switch r.Method {
			case http.MethodGet:
				routes.preview.ServeHTTP(w, r.WithContext(ctx))
			case http.MethodPost:
				routes.grant.ServeHTTP(w, r.WithContext(ctx))
			default:
				w.Header().Set("Allow", "GET, POST")
				writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
			}
			return true
		case "run-intents":
			if r.Method == http.MethodPost {
				routes.submit.ServeHTTP(w, r.WithContext(ctx))
			} else if r.Method == http.MethodGet {
				routes.list.ServeHTTP(w, r.WithContext(ctx))
			} else {
				w.Header().Set("Allow", "GET, POST")
				writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
			}
			return true
		}
		if routes.serveIntentTimelineRoute(w, r, ctx, suffix) {
			return true
		}
	}
	return false
}

func (routes executionRoutes) serveIntentTimelineRoute(w http.ResponseWriter, r *http.Request, ctx context.Context, suffix string) bool {
	parts := strings.Split(suffix, "/")
	if len(parts) != 3 || parts[0] != "run-intents" || parts[2] != "timeline" {
		return false
	}
	if r.Method != http.MethodGet {
		w.Header().Set("Allow", http.MethodGet)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return true
	}
	intentID, err := url.PathUnescape(parts[1])
	if err != nil || !validExecutionRouteID(intentID) {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "route not found")
		return true
	}
	ctx = context.WithValue(ctx, intentIDContextKey{}, intentID)
	routes.timeline.ServeHTTP(w, r.WithContext(ctx))
	return true
}

type grantIDContextKey struct{}

type intentIDContextKey struct{}

func validExecutionRouteID(value string) bool {
	return strings.TrimSpace(value) != "" && len(value) <= conversationIDMaxBytes &&
		!strings.ContainsAny(value, "/\x00\r\n")
}
