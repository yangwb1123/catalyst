package appserver

import (
	"encoding/hex"
	"errors"
	consentmodel "forgeos/forge-core/internal/runtimebridge/consentmodel"
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"net/http"
	"net/url"

	"forgeos/forge-core/internal/executionprofile"
	"forgeos/forge-core/internal/runtimebridge"
)

const maxExecutionConsentTTLMS uint64 = 30 * 24 * 60 * 60 * 1000

type grantExecutionConsentRequest struct {
	ConfirmExecutionProfile bool   `json:"confirm_execution_profile"`
	ExpectedProjectID       string `json:"expected_project_id"`
	ExpectedProfileID       string `json:"expected_profile_id"`
	ExpectedProfileSHA256   string `json:"expected_profile_sha256"`
	ExpiresAtMS             uint64 `json:"expires_at_ms"`
}

type executionConsentReceipt struct {
	GrantID       string `json:"grant_id"`
	ProjectID     string `json:"project_id"`
	ProfileID     string `json:"profile_id"`
	ProfileSHA256 string `json:"profile_sha256"`
	GrantedAtMS   uint64 `json:"granted_at_ms"`
	ExpiresAtMS   uint64 `json:"expires_at_ms"`
}

type executionConsentGrantResponse struct {
	Grant    executionConsentReceipt `json:"grant"`
	Replayed bool                    `json:"replayed"`
}

type executionConsentPreviewResponse struct {
	ConversationID string `json:"conversation_id"`
	ProjectID      string `json:"project_id"`
	ProfileID      string `json:"profile_id"`
	ProfileSHA256  string `json:"profile_sha256"`
	MaximumTTLMS   uint64 `json:"maximum_ttl_ms"`
}

type revokeExecutionConsentResponse struct {
	Revocation consentmodel.ProjectExecutionConsentRevocation `json:"revocation"`
	Replayed   bool                                           `json:"replayed"`
}

func (routes executionRoutes) previewProjectConsent(w http.ResponseWriter, r *http.Request) {
	if requestHasBody(r) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "GET requests must not include a body")
		return
	}
	if _, err := parseConversationQuery(r); err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "execution consent preview does not accept query parameters")
		return
	}
	conversationID, _ := r.Context().Value(conversationIDContextKey{}).(string)
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if routes.reader == nil {
		writeConversationBackendUnavailable(w, r)
		return
	}
	identity, err := routes.reader.OwnedProjectConversationIdentity(r.Context(), owner, conversationID)
	if err != nil {
		writeExecutionBackendError(w, r, err)
		return
	}
	if identity.ConversationID != conversationID || !validExecutionRouteID(identity.ProjectID) {
		writeConversationError(w, r, http.StatusBadGateway, "conversation_service_error", "conversation service request failed")
		return
	}
	if routes.profiles == nil {
		writeConversationError(w, r, http.StatusServiceUnavailable, "execution_profile_unavailable", "execution profile is unavailable")
		return
	}
	profile, err := routes.profiles.ProfileForProject(identity.ProjectID)
	if err != nil {
		writeExecutionBackendError(w, r, err)
		return
	}
	writeConversationJSON(w, r, http.StatusOK, executionConsentPreviewResponse{
		ConversationID: conversationID, ProjectID: identity.ProjectID,
		ProfileID: profile.ID, ProfileSHA256: hex.EncodeToString(profile.SHA256[:]),
		MaximumTTLMS: maxExecutionConsentTTLMS,
	})
}

func (routes executionRoutes) grantProjectConsent(w http.ResponseWriter, r *http.Request) {
	conversationID, _ := r.Context().Value(conversationIDContextKey{}).(string)
	key, request, ok := readGrantConsentRequest(w, r)
	if !ok {
		return
	}
	owner, projectID, profile, ok := routes.confirmedGrantProfile(w, r, conversationID, request)
	if !ok {
		return
	}
	result, err := routes.backend.GrantProjectExecutionConsent(
		r.Context(), owner, projectID, profile.ID, profile.SHA256, request.ExpiresAtMS, key,
	)
	if err != nil {
		writeExecutionBackendError(w, r, err)
		return
	}
	writeGrantConsentReceipt(w, r, result)
}

func readGrantConsentRequest(
	w http.ResponseWriter,
	r *http.Request,
) (string, grantExecutionConsentRequest, bool) {
	key, err := requestIdempotencyKey(r)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return "", grantExecutionConsentRequest{}, false
	}
	var request grantExecutionConsentRequest
	body, err := readStrictConversationJSON(w, r, &request)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return "", grantExecutionConsentRequest{}, false
	}
	valid := hasExactRequiredFields(body,
		"confirm_execution_profile", "expected_project_id", "expected_profile_id",
		"expected_profile_sha256", "expires_at_ms",
	) && request.ConfirmExecutionProfile && request.ExpiresAtMS > 0 &&
		request.ExpiresAtMS <= maxSQLiteCursor && validExecutionRouteID(request.ExpectedProjectID) &&
		validExecutionRouteID(request.ExpectedProfileID) && validProfileDigest(request.ExpectedProfileSHA256)
	if !valid {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "execution consent request is invalid")
		return "", grantExecutionConsentRequest{}, false
	}
	return key, request, true
}

func (routes executionRoutes) confirmedGrantProfile(
	w http.ResponseWriter,
	r *http.Request,
	conversationID string,
	request grantExecutionConsentRequest,
) (model.Owner, string, intentmodel.ServerExecutionProfile, bool) {
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return model.Owner{}, "", intentmodel.ServerExecutionProfile{}, false
	}
	if routes.backend == nil {
		writeConversationBackendUnavailable(w, r)
		return model.Owner{}, "", intentmodel.ServerExecutionProfile{}, false
	}
	identity, err := routes.backend.OwnedProjectConversationIdentity(r.Context(), owner, conversationID)
	if err != nil {
		writeExecutionBackendError(w, r, err)
		return model.Owner{}, "", intentmodel.ServerExecutionProfile{}, false
	}
	if identity.ConversationID != conversationID || !validExecutionRouteID(identity.ProjectID) {
		writeConversationError(w, r, http.StatusBadGateway, "conversation_service_error", "conversation service request failed")
		return model.Owner{}, "", intentmodel.ServerExecutionProfile{}, false
	}
	if routes.profiles == nil {
		writeConversationError(w, r, http.StatusServiceUnavailable, "execution_profile_unavailable", "execution profile is unavailable")
		return model.Owner{}, "", intentmodel.ServerExecutionProfile{}, false
	}
	profile, err := routes.profiles.ProfileForProject(identity.ProjectID)
	if err != nil {
		writeExecutionBackendError(w, r, err)
		return model.Owner{}, "", intentmodel.ServerExecutionProfile{}, false
	}
	if request.ExpectedProjectID != identity.ProjectID || request.ExpectedProfileID != profile.ID ||
		request.ExpectedProfileSHA256 != hex.EncodeToString(profile.SHA256[:]) {
		writeConversationError(w, r, http.StatusConflict, "execution_profile_changed", "execution profile changed; preview it again before confirming")
		return model.Owner{}, "", intentmodel.ServerExecutionProfile{}, false
	}
	return owner, identity.ProjectID, profile, true
}

func writeGrantConsentReceipt(w http.ResponseWriter, r *http.Request, result consentmodel.ProjectExecutionConsentGrantResult) {
	grant := result.Grant
	status := http.StatusCreated
	if result.Replayed {
		status = http.StatusOK
	}
	writeConversationJSON(w, r, status, executionConsentGrantResponse{
		Grant: executionConsentReceipt{
			GrantID: grant.GrantID, ProjectID: grant.ProjectID, ProfileID: grant.ProfileID,
			ProfileSHA256: hex.EncodeToString(grant.ProfileSHA256[:]),
			GrantedAtMS:   grant.GrantedAtMS, ExpiresAtMS: grant.ExpiresAtMS,
		},
		Replayed: result.Replayed,
	})
}

func validProfileDigest(value string) bool {
	if len(value) != 64 {
		return false
	}
	decoded, err := hex.DecodeString(value)
	return err == nil && hex.EncodeToString(decoded) == value
}

func (routes executionRoutes) revokeProjectConsent(w http.ResponseWriter, r *http.Request) {
	if requestHasBody(r) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "DELETE requests must not include a body")
		return
	}
	grantID, _ := r.Context().Value(grantIDContextKey{}).(string)
	grantID, err := url.PathUnescape(grantID)
	if err != nil || !validExecutionRouteID(grantID) {
		writeConversationError(w, r, http.StatusNotFound, "not_found", "execution consent was not found")
		return
	}
	key, err := requestIdempotencyKey(r)
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if routes.backend == nil {
		writeConversationBackendUnavailable(w, r)
		return
	}
	result, err := routes.backend.RevokeProjectExecutionConsent(r.Context(), owner, grantID, key)
	if err != nil {
		writeExecutionBackendError(w, r, err)
		return
	}
	writeConversationJSON(w, r, http.StatusOK, revokeExecutionConsentResponse{
		Revocation: result.Revocation, Replayed: result.Replayed,
	})
}

func writeExecutionBackendError(w http.ResponseWriter, r *http.Request, err error) {
	if errors.Is(err, executionprofile.ErrProfileUnavailable) {
		writeConversationError(w, r, http.StatusServiceUnavailable, "execution_profile_unavailable", "execution profile is unavailable")
		return
	}
	if errors.Is(err, executionprofile.ErrInvalidProjectContext) {
		writeConversationError(w, r, http.StatusBadGateway, "conversation_service_error", "conversation service request failed")
		return
	}
	var bridgeErr *runtimebridge.Error
	if !errors.As(err, &bridgeErr) || bridgeErr == nil {
		writeConversationError(w, r, http.StatusBadGateway, "conversation_service_error", "conversation service request failed")
		return
	}
	switch bridgeErr.Code {
	case "invalid_project_execution_consent_request", "invalid_pending_run_intent_request", "invalid_owned_prompt_request":
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "request is invalid")
	case "not_found":
		writeConversationError(w, r, http.StatusNotFound, "not_found", "conversation or execution consent was not found")
	case "conflict":
		writeConversationError(w, r, http.StatusConflict, "conflict", "execution request conflicts with current state")
	default:
		writeConversationBackendError(w, r, err)
	}
}
