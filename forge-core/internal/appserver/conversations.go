package appserver

import (
	"context"
	"errors"
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
	"net/http"
	"net/url"
	"strings"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/executionprofile"
	"forgeos/forge-core/internal/runtimebridge"
)

const (
	conversationCollectionPath        = "/api/v1/conversations"
	conversationBodyMaxBytes          = 2 * 1024 * 1024
	conversationPageMax               = 128
	conversationPageDefault           = 50
	conversationIDMaxBytes            = 128
	idempotencyKeyMaxBytes            = 256
	promptContentMaxBytes             = 256 * 1024
	conversationImportPromptMax       = 128
	conversationImportContentMaxBytes = 256 * 1024
	conversationJSONMaxDepth          = 64
	maxSQLiteCursor                   = uint64(1<<63 - 1)
)

type conversationBackend interface {
	CreateOwnedConversation(
		context.Context,
		model.Owner,
		model.ConversationScope,
		string,
		string,
	) (model.Conversation, error)
	ImportOwnedConversation(
		context.Context,
		model.Owner,
		string,
		[]model.ConversationImportPrompt,
		string,
	) (model.OwnedConversationImportResult, error)
	ListOwnedConversations(context.Context, model.Owner, string, int) (model.OwnedConversationPage, error)
	OwnedConversationPrompts(
		context.Context,
		model.Owner,
		string,
		*model.PromptPageCursor,
		int,
	) (model.ConversationPromptPage, error)
	OwnedConversationRuns(
		context.Context,
		model.Owner,
		string,
		*runmodel.OwnedRunPageCursor,
		int,
	) (runmodel.OwnedRunPage, error)
	OwnedConversationRunTimeline(
		context.Context,
		model.Owner,
		string,
		string,
		uint64,
		int,
	) (runmodel.OwnedRunTimelinePage, error)
	AppendOwnedPrompt(
		context.Context,
		model.Owner,
		string,
		string,
		string,
		uint64,
	) (model.ConversationPrompt, uint64, bool, error)
}

type conversationRoutes struct {
	backend            conversationBackend
	profiles           *executionprofile.Catalog
	changes            http.Handler
	list               http.Handler
	create             http.Handler
	importConversation http.Handler
	listPrompts        http.Handler
	appendPrompt       http.Handler
	listRuns           http.Handler
	listRunTimeline    http.Handler
}

type conversationErrorResponse struct {
	APIVersion string `json:"api_version"`
	Code       string `json:"code"`
	Message    string `json:"message"`
}

type createConversationRequest struct {
	Scope model.ConversationScope `json:"scope"`
	Title string                  `json:"title"`
}

type importConversationRequest struct {
	Title   string                           `json:"title"`
	Prompts []model.ConversationImportPrompt `json:"prompts"`
}

type appendPromptRequest struct {
	Content         string `json:"content"`
	ExpectedVersion uint64 `json:"expected_version"`
}

var (
	errConversationContentType = errors.New("unsupported content type")
	errConversationBodyTooBig  = errors.New("request body too large")
	errConversationJSON        = errors.New("invalid JSON request")
)

// newConversationRoutes builds the authenticated-session API surface. The
// caller must wrap it in authn.Authenticator.Handler; each supported route
// adds its own route-specific scope check here.
func newConversationRoutes(client *runtimebridge.Client) http.Handler {
	if client == nil {
		return newConversationRoutesWithBackend(nil)
	}
	return newConversationRoutesWithBackend(client)
}

func newConversationRoutesWithExecutionProfiles(
	client *runtimebridge.Client,
	profiles *executionprofile.Catalog,
) http.Handler {
	if client == nil {
		return newConversationRoutesWithBackendAndExecutionProfiles(nil, profiles)
	}
	return newConversationRoutesWithBackendAndExecutionProfiles(client, profiles)
}

func newConversationRoutesWithBackend(backend conversationBackend) http.Handler {
	return newConversationRoutesWithBackendAndExecutionProfiles(backend, nil)
}

func newConversationRoutesWithBackendAndExecutionProfiles(
	backend conversationBackend,
	profiles *executionprofile.Catalog,
) http.Handler {
	routes := conversationRoutes{backend: backend, profiles: profiles}
	routes.changes = authn.RequireScopes(http.HandlerFunc(routes.ownedConversationChanges), "forge:conversations:read")
	routes.list = authn.RequireScopes(http.HandlerFunc(routes.listConversations), "forge:conversations:read")
	routes.create = authn.RequireScopes(http.HandlerFunc(routes.createConversation), "forge:conversations:write")
	routes.importConversation = authn.RequireScopes(http.HandlerFunc(routes.importConversationHandler), "forge:conversations:write")
	routes.listPrompts = authn.RequireScopes(http.HandlerFunc(routes.listConversationPrompts), "forge:conversations:read")
	routes.appendPrompt = authn.RequireScopes(http.HandlerFunc(routes.appendConversationPrompt), "forge:conversations:write")
	routes.listRuns = authn.RequireScopes(http.HandlerFunc(routes.listConversationRuns), "forge:conversations:read")
	routes.listRunTimeline = authn.RequireScopes(http.HandlerFunc(routes.listConversationRunTimeline), "forge:conversations:read")
	return routes
}

// resolveExecutionProfile is reserved for a future consent/intent handler.
// No current HTTP route calls it or converts a stored Prompt into work.
func (routes conversationRoutes) resolveExecutionProfile(
	ctx context.Context,
	owner model.Owner,
	conversationID string,
) (intentmodel.ServerExecutionProfile, error) {
	if routes.profiles == nil || routes.backend == nil {
		return intentmodel.ServerExecutionProfile{}, executionprofile.ErrProfileUnavailable
	}
	reader, ok := routes.backend.(interface {
		OwnedProjectConversationIdentity(
			context.Context,
			model.Owner,
			string,
		) (model.OwnedProjectConversationIdentity, error)
	})
	if !ok {
		return intentmodel.ServerExecutionProfile{}, executionprofile.ErrProfileUnavailable
	}
	return routes.profiles.ResolveConversationProfile(ctx, reader, owner, conversationID)
}

func (routes conversationRoutes) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	path := r.URL.EscapedPath()
	if routes.serveConversationAuxiliaryRoutes(w, r, path) || routes.serveConversationCollectionRoutes(w, r, path) {
		return
	}
	writeJSON(w, r, http.StatusNotFound, conversationErrorBody("not_found", "route not found"))
}

func (routes conversationRoutes) serveConversationAuxiliaryRoutes(w http.ResponseWriter, r *http.Request, path string) bool {
	if path == conversationCollectionPath+"/import" {
		if !requireConversationMethod(w, r, http.MethodPost) {
			return true
		}
		routes.importConversation.ServeHTTP(w, r)
		return true
	}
	if path == conversationChangesPath {
		if !requireConversationMethod(w, r, http.MethodGet) {
			return true
		}
		routes.changes.ServeHTTP(w, r)
		return true
	}
	if conversationID, runID, ok := conversationRunTimelineIDs(path); ok {
		if !requireConversationMethod(w, r, http.MethodGet) {
			return true
		}
		ctx := context.WithValue(r.Context(), conversationIDContextKey{}, conversationID)
		ctx = context.WithValue(ctx, runIDContextKey{}, runID)
		routes.listRunTimeline.ServeHTTP(w, r.WithContext(ctx))
		return true
	}
	if conversationID, ok := conversationRunsID(path); ok {
		if !requireConversationMethod(w, r, http.MethodGet) {
			return true
		}
		routes.listRuns.ServeHTTP(w, r.WithContext(context.WithValue(r.Context(), conversationIDContextKey{}, conversationID)))
		return true
	}
	return false
}

func (routes conversationRoutes) serveConversationCollectionRoutes(w http.ResponseWriter, r *http.Request, path string) bool {
	if path == conversationCollectionPath {
		switch r.Method {
		case http.MethodGet:
			routes.list.ServeHTTP(w, r)
		case http.MethodPost:
			routes.create.ServeHTTP(w, r)
		default:
			writeConversationMethodError(w, r)
		}
		return true
	}
	if conversationID, ok := promptConversationID(path); ok {
		switch r.Method {
		case http.MethodGet:
			routes.listPrompts.ServeHTTP(w, r.WithContext(context.WithValue(r.Context(), conversationIDContextKey{}, conversationID)))
		case http.MethodPost:
			routes.appendPrompt.ServeHTTP(w, r.WithContext(context.WithValue(r.Context(), conversationIDContextKey{}, conversationID)))
		default:
			writeConversationMethodError(w, r)
		}
		return true
	}
	return false
}

func requireConversationMethod(w http.ResponseWriter, r *http.Request, method string) bool {
	if r.Method == method {
		return true
	}
	w.Header().Set("Allow", method)
	writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
	return false
}

type conversationIDContextKey struct{}

type runIDContextKey struct{}

func conversationRunsID(escapedPath string) (string, bool) {
	conversationID, suffix, ok := conversationPathSuffix(escapedPath)
	if !ok || suffix != "runs" {
		return "", false
	}
	return conversationID, true
}

func conversationRunTimelineIDs(escapedPath string) (string, string, bool) {
	conversationID, suffix, ok := conversationPathSuffix(escapedPath)
	if !ok {
		return "", "", false
	}
	parts := strings.Split(suffix, "/")
	if len(parts) != 3 || parts[0] != "runs" || parts[1] == "" || parts[2] != "timeline" {
		return "", "", false
	}
	runID, err := url.PathUnescape(parts[1])
	if err != nil || strings.ContainsAny(runID, "/\x00") || strings.TrimSpace(runID) == "" || len(runID) > conversationIDMaxBytes {
		return "", "", false
	}
	return conversationID, runID, true
}

func conversationPathSuffix(escapedPath string) (string, string, bool) {
	prefix := conversationCollectionPath + "/"
	if !strings.HasPrefix(escapedPath, prefix) {
		return "", "", false
	}
	parts := strings.Split(strings.TrimPrefix(escapedPath, prefix), "/")
	if len(parts) < 2 || parts[0] == "" {
		return "", "", false
	}
	conversationID, err := url.PathUnescape(parts[0])
	if err != nil || strings.ContainsAny(conversationID, "/\x00") || strings.TrimSpace(conversationID) == "" || len(conversationID) > conversationIDMaxBytes {
		return "", "", false
	}
	return conversationID, strings.Join(parts[1:], "/"), true
}

func promptConversationID(escapedPath string) (string, bool) {
	prefix := conversationCollectionPath + "/"
	if !strings.HasPrefix(escapedPath, prefix) {
		return "", false
	}
	parts := strings.Split(strings.TrimPrefix(escapedPath, prefix), "/")
	if len(parts) != 2 || parts[0] == "" || parts[1] != "prompts" {
		return "", false
	}
	id, err := url.PathUnescape(parts[0])
	if err != nil || strings.ContainsAny(id, "/\x00") || strings.TrimSpace(id) == "" || len(id) > conversationIDMaxBytes {
		return "", false
	}
	return id, true
}
