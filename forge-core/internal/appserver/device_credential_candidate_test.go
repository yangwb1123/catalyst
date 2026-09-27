package appserver

import (
	"context"
	"encoding/json"
	"net/http"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/devicecredential"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"forgeos/forge-core/internal/statefs"
)

func TestLifecycleCredentialCandidateDefaultsClosedAndProductionRemainsUnregistered(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	body := lifecycleCredentialCandidateBody(t, devicecredential.ActionIssue, 1, "credential-a", "key-a", strings.Repeat("a", 64), 1, "", "", "", 120000, 180000, 120000)
	request := func(handler http.Handler) int {
		return requestConversationAPI(t, handler, identity, http.MethodPost,
			lifecycleCredentialCandidatePath, lifecycleCredentialCandidateScope,
			"application/json", "", body).Code
	}
	if status := request(authenticator.Handler(newLifecycleRegistryCandidateRoutes(nil))); status != http.StatusNotFound {
		t.Fatalf("disabled candidate status=%d, want 404", status)
	}
	if status := request(authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))); status != http.StatusNotFound {
		t.Fatalf("production route status=%d, want 404", status)
	}
}

func TestLifecycleCredentialCandidateIssuesAndPersistsMetadataOnlyPlan(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	root := t.TempDir()
	if err := statefs.EnsurePrivateDir(root); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "lifecycle-registry.json")
	config := &lifecycleRegistryCandidateConfig{
		Enabled:    true,
		Store:      newPersistedLifecycleRegistryCandidateStore(path),
		Credential: &lifecycleCredentialCandidateConfig{Enabled: true},
		Heartbeat: &lifecycleHeartbeatCandidateConfig{
			Enabled:            true,
			AllowUnsignedProof: true,
			Now:                func(context.Context) (uint64, error) { return 120_000, nil },
			StaleAfterMS:       90_000,
		},
	}
	handler := authenticator.Handler(newLifecycleRegistryCandidateRoutes(config))
	seed := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", lifecycleHeartbeatCandidateBody(t, identity.issuer, "account-42", "tenant-slate", 1, 1, 0))
	if seed.Code != http.StatusOK {
		t.Fatalf("seed heartbeat status=%d body=%q", seed.Code, seed.Body.String())
	}
	body := lifecycleCredentialCandidateBody(t, devicecredential.ActionIssue, 1, "credential-a", "key-a", strings.Repeat("a", 64), 1, "", "", "", 120000, 180000, 120000)
	unknown := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleCredentialCandidatePath, lifecycleCredentialCandidateScope,
		"application/json", "", strings.TrimSuffix(body, "}")+`,"authority":true}`)
	if unknown.Code != http.StatusBadRequest {
		t.Fatalf("unknown field status=%d body=%q", unknown.Code, unknown.Body.String())
	}
	wrongScope := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleCredentialCandidatePath, lifecycleRegistryCandidateReadScope,
		"application/json", "", body)
	if wrongScope.Code != http.StatusForbidden {
		t.Fatalf("wrong scope status=%d body=%q", wrongScope.Code, wrongScope.Body.String())
	}
	issued := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleCredentialCandidatePath, lifecycleCredentialCandidateScope,
		"application/json", "", body)
	if issued.Code != http.StatusOK {
		t.Fatalf("issue status=%d body=%q", issued.Code, issued.Body.String())
	}
	var response lifecycleCredentialCandidateResponse
	if err := json.Unmarshal(issued.Body.Bytes(), &response); err != nil {
		t.Fatal(err)
	}
	owner := model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	if response.Owner.Issuer != owner.Issuer || response.Owner.Subject != owner.Subject || response.Owner.TenantID != owner.TenantID ||
		response.Next.CredentialID != "credential-a" || response.Next.CredentialState != devicecredential.CredentialActive ||
		response.Next.ApprovalState != devicecredential.ApprovalApproved || !response.PreviewOnly ||
		!response.CandidatePublished || response.Authority != (devicecredential.TransitionAuthority{}) {
		t.Fatalf("unexpected credential response=%#v", response)
	}
	registryResponse := requestConversationAPI(t, handler, identity, http.MethodGet,
		lifecycleRegistryCandidatePath, lifecycleRegistryCandidateReadScope, "", "", "")
	if registryResponse.Code != http.StatusOK || strings.Contains(registryResponse.Body.String(), `"credential_candidate"`) {
		t.Fatalf("candidate registry response status=%d body=%q", registryResponse.Code, registryResponse.Body.String())
	}
	adapter := newPersistedLifecycleRegistryCandidateStore(path)
	stored, err := adapter.ReadSnapshot(context.Background(), owner)
	if err != nil {
		t.Fatal(err)
	}
	if len(stored.States()) != 1 || stored.States()[0].CredentialCandidate == nil ||
		stored.States()[0].CredentialCandidate.CredentialID != "credential-a" ||
		stored.States()[0].Device.CredentialState != "active" {
		t.Fatalf("unexpected stored credential candidate=%#v", stored.States())
	}
}

func TestLifecycleCredentialCandidateRotateRevokeAndStaleRevision(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	root := t.TempDir()
	if err := statefs.EnsurePrivateDir(root); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "lifecycle-registry.json")
	config := &lifecycleRegistryCandidateConfig{
		Enabled:    true,
		Store:      newPersistedLifecycleRegistryCandidateStore(path),
		Credential: &lifecycleCredentialCandidateConfig{Enabled: true},
		Heartbeat: &lifecycleHeartbeatCandidateConfig{
			Enabled:            true,
			AllowUnsignedProof: true,
			Now:                func(context.Context) (uint64, error) { return 120_000, nil },
			StaleAfterMS:       90_000,
		},
	}
	handler := authenticator.Handler(newLifecycleRegistryCandidateRoutes(config))
	seed := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", lifecycleHeartbeatCandidateBody(t, identity.issuer, "account-42", "tenant-slate", 1, 1, 0))
	if seed.Code != http.StatusOK {
		t.Fatalf("seed heartbeat status=%d body=%q", seed.Code, seed.Body.String())
	}
	issue := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleCredentialCandidatePath, lifecycleCredentialCandidateScope, "application/json", "",
		lifecycleCredentialCandidateBody(t, devicecredential.ActionIssue, 1, "credential-a", "key-a", strings.Repeat("a", 64), 1, "", "", "", 120000, 180000, 120000))
	if issue.Code != http.StatusOK {
		t.Fatalf("issue status=%d body=%q", issue.Code, issue.Body.String())
	}
	rotate := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleCredentialCandidatePath, lifecycleCredentialCandidateScope, "application/json", "",
		lifecycleCredentialCandidateBody(t, devicecredential.ActionRotate, 1, "", "", "", 0, "credential-b", "key-b", strings.Repeat("b", 64), 180000, 240000, 180000))
	if rotate.Code != http.StatusOK {
		t.Fatalf("rotate status=%d body=%q", rotate.Code, rotate.Body.String())
	}
	var rotated lifecycleCredentialCandidateResponse
	if err := json.Unmarshal(rotate.Body.Bytes(), &rotated); err != nil {
		t.Fatal(err)
	}
	if rotated.Next.CredentialID != "credential-b" || rotated.Next.KeyID != "key-b" || rotated.Next.KeyGeneration != 2 {
		t.Fatalf("unexpected rotation=%#v", rotated)
	}
	revoke := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleCredentialCandidatePath, lifecycleCredentialCandidateScope, "application/json", "",
		lifecycleCredentialCandidateBody(t, devicecredential.ActionRevoke, 1, "", "", "", 0, "", "", "", 0, 0, 0))
	if revoke.Code != http.StatusOK {
		t.Fatalf("revoke status=%d body=%q", revoke.Code, revoke.Body.String())
	}
	stale := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleCredentialCandidatePath, lifecycleCredentialCandidateScope, "application/json", "",
		lifecycleCredentialCandidateBody(t, devicecredential.ActionRevoke, 2, "", "", "", 0, "", "", "", 0, 0, 0))
	if stale.Code != http.StatusConflict {
		t.Fatalf("stale status=%d body=%q", stale.Code, stale.Body.String())
	}
}

func lifecycleCredentialCandidateBody(
	t *testing.T,
	action devicecredential.Action,
	revision uint64,
	credentialID, keyID, publicKeyDigest string,
	keyGeneration uint64,
	nextCredentialID, nextKeyID, nextPublicKeyDigest string,
	issuedAtMS, expiresAtMS, observedAtMS uint64,
) string {
	t.Helper()
	value := map[string]any{
		"device_id":                "device-a",
		"action":                   action,
		"approval_state":           "approved",
		"credential_id":            credentialID,
		"key_id":                   keyID,
		"public_key_sha256":        publicKeyDigest,
		"key_generation":           keyGeneration,
		"issued_at_ms":             issuedAtMS,
		"expires_at_ms":            expiresAtMS,
		"next_credential_id":       nextCredentialID,
		"next_key_id":              nextKeyID,
		"next_public_key_sha256":   nextPublicKeyDigest,
		"observed_at_ms":           observedAtMS,
		"expected_device_revision": revision,
	}
	body, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return string(body)
}
