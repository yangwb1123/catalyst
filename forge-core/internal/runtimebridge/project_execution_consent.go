package runtimebridge

import (
	"context"
	"encoding/json"
	consentmodel "forgeos/forge-core/internal/runtimebridge/consentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"strings"
	"unicode"
)

// GrantProjectExecutionConsent sends a trusted owner's model.Project/profile grant
// to the private Runtime v2 process bridge. Profile selection must be made by
// server policy, never by an untrusted HTTP request.
func (client *Client) GrantProjectExecutionConsent(
	ctx context.Context,
	owner model.Owner,
	projectID string,
	profileID string,
	profileSHA256 [32]byte,
	expiresAtMS uint64,
	idempotencyKey string,
) (consentmodel.ProjectExecutionConsentGrantResult, error) {
	if !validConsentOwner(owner) || !validConsentEntityID(projectID) || !validConsentEntityID(profileID) ||
		expiresAtMS > maxSQLiteInteger || !validConsentIdempotencyKey(idempotencyKey) {
		return consentmodel.ProjectExecutionConsentGrantResult{}, &Error{Code: "invalid_project_execution_consent_request"}
	}
	ownerCopy, digestCopy, expiryCopy := owner, profileSHA256, expiresAtMS
	response, err := client.callWrite(ctx, request{
		Operation: "grant_project_execution_consent", Owner: &ownerCopy,
		ProjectID: projectID, ProfileID: profileID, ProfileSHA256: &digestCopy,
		ExpiresAtMS: &expiryCopy, IdempotencyKey: idempotencyKey,
	})
	if err != nil {
		return consentmodel.ProjectExecutionConsentGrantResult{}, err
	}
	var result consentmodel.ProjectExecutionConsentGrantResult
	if err := decodeStrict(response, &result); err != nil ||
		!validProjectExecutionConsentGrant(response, result, projectID, profileID, profileSHA256, expiresAtMS) {
		return consentmodel.ProjectExecutionConsentGrantResult{}, &Error{Code: "invalid_runtime_response"}
	}
	return result, nil
}

// RevokeProjectExecutionConsent sends an exact owner-scoped grant revocation
// to the private Runtime v2 process bridge.
func (client *Client) RevokeProjectExecutionConsent(
	ctx context.Context,
	owner model.Owner,
	grantID string,
	idempotencyKey string,
) (consentmodel.ProjectExecutionConsentRevocationResult, error) {
	if !validConsentOwner(owner) || !validConsentEntityID(grantID) ||
		!validConsentIdempotencyKey(idempotencyKey) {
		return consentmodel.ProjectExecutionConsentRevocationResult{}, &Error{Code: "invalid_project_execution_consent_request"}
	}
	ownerCopy := owner
	response, err := client.callWrite(ctx, request{
		Operation: "revoke_project_execution_consent", Owner: &ownerCopy,
		GrantID: grantID, IdempotencyKey: idempotencyKey,
	})
	if err != nil {
		return consentmodel.ProjectExecutionConsentRevocationResult{}, err
	}
	var result consentmodel.ProjectExecutionConsentRevocationResult
	if err := decodeStrict(response, &result); err != nil ||
		!validProjectExecutionConsentRevocation(response, result, grantID) {
		return consentmodel.ProjectExecutionConsentRevocationResult{}, &Error{Code: "invalid_runtime_response"}
	}
	return result, nil
}

func validConsentOwner(owner model.Owner) bool {
	return validConsentText(owner.Issuer, 2048) && validConsentText(owner.Subject, 255) &&
		validConsentText(owner.TenantID, 256)
}

func validConsentEntityID(value string) bool {
	return validConsentText(value, maxEntityIDBytes)
}

func validConsentIdempotencyKey(value string) bool {
	return validConsentText(value, 256)
}

func validConsentText(value string, maximumBytes int) bool {
	return strings.TrimSpace(value) != "" && len(value) <= maximumBytes &&
		!strings.ContainsFunc(value, unicode.IsControl)
}

func validProjectExecutionConsentGrant(
	data []byte,
	result consentmodel.ProjectExecutionConsentGrantResult,
	projectID string,
	profileID string,
	profileSHA256 [32]byte,
	expiresAtMS uint64,
) bool {
	grant := result.Grant
	if requireObjectFieldSet(data, "grant", "replayed") != nil ||
		!validConsentEntityID(grant.GrantID) || grant.ProjectID != projectID ||
		grant.ProfileID != profileID || grant.ProfileSHA256 != profileSHA256 ||
		grant.GrantedAtMS > maxSQLiteInteger || grant.ExpiresAtMS != expiresAtMS ||
		grant.ExpiresAtMS > maxSQLiteInteger || grant.GrantedAtMS >= grant.ExpiresAtMS {
		return false
	}
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil ||
		requireObjectFieldSet(root["grant"], "grant_id", "project_id", "profile_id", "profile_sha256",
			"granted_at_ms", "expires_at_ms") != nil {
		return false
	}
	var grantFields map[string]json.RawMessage
	var digest []uint8
	if json.Unmarshal(root["grant"], &grantFields) != nil ||
		json.Unmarshal(grantFields["profile_sha256"], &digest) != nil || len(digest) != len(profileSHA256) {
		return false
	}
	for index, value := range profileSHA256 {
		if digest[index] != value {
			return false
		}
	}
	return true
}

func validProjectExecutionConsentRevocation(
	data []byte,
	result consentmodel.ProjectExecutionConsentRevocationResult,
	requestedGrantID string,
) bool {
	revocation := result.Revocation
	if requireObjectFieldSet(data, "revocation", "replayed") != nil ||
		!validConsentEntityID(revocation.EventID) || revocation.GrantID != requestedGrantID ||
		revocation.RevokedAtMS > maxSQLiteInteger {
		return false
	}
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil {
		return false
	}
	return requireObjectFieldSet(root["revocation"], "event_id", "grant_id", "revoked_at_ms") == nil
}
