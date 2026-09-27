package appserver

// This test wires the real Snaplink SSO HTTP server to the Forge authn and
// conversation HTTP boundary. It is deliberately test-only: the issuer uses
// an in-memory user/client store and an ephemeral TLS listener, and it does
// not register devices or execute work.

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/yangwb1123/snaplink/domains/authenticators"
	"github.com/yangwb1123/snaplink/infrastructure/defaultimpl"
	"github.com/yangwb1123/snaplink/interfaces/sso"

	"forgeos/forge-core/internal/authn"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

const (
	snaplinkForgeTestUser   = "snaplink-forge-user"
	snaplinkForgeTestTenant = "snaplink-forge-tenant"
	// secret-scan:ignore — in-memory Snaplink test credential.
	snaplinkForgeTestPassword = "snaplink-forge-password"
	snaplinkForgeTestAudience = "forge-api"
)

func TestSnaplinkAuthenticatedConversationOwnerParity(t *testing.T) {
	ctx := context.Background()
	// Allocate the listener before constructing Snaplink so its configured
	// issuer and the `iss` claim are the same HTTPS origin that serves JWKS.
	ssoHTTP := httptest.NewUnstartedServer(nil)
	issuer := "https://" + ssoHTTP.Listener.Addr().String()

	users := defaultimpl.NewMemoryUserProvider()
	if err := users.CreateOrUpdate(ctx, &sso.User{
		ID:   snaplinkForgeTestUser,
		Name: "Forge Snaplink Test User",
	}); err != nil {
		t.Fatal(err)
	}
	clients := defaultimpl.NewMemoryClientStore()
	forgeScopes := []string{"openid", "profile", "forge:conversations:read", "forge:conversations:write", deviceInventoryReadCandidateScope, devicePlacementRegistryCandidateScope, lifecycleRegistryCandidateReadScope, schedulerSelectionLeaseScope}
	for _, clientID := range []string{"forge-console", "forge-cli"} {
		clients.AddSeed(&sso.Client{
			ID:                      clientID,
			Name:                    clientID,
			TenantID:                snaplinkForgeTestTenant,
			SubjectType:             "public",
			AllowedScopes:           forgeScopes,
			AllowedResources:        []string{snaplinkForgeTestAudience},
			AllowedAuthenticators:   []string{"password"},
			TokenStrategy:           "jwt",
			Active:                  true,
			TokenEndpointAuthMethod: "none",
			SkipConsent:             true,
		})
	}
	password := authenticators.NewPasswordAuthenticator(authenticators.PasswordVerifierFunc(
		func(_ context.Context, username, supplied string) (*sso.AuthResult, error) {
			if username != "snaplink-forge-user" || supplied != snaplinkForgeTestPassword {
				return nil, errors.New("bad credentials")
			}
			return &sso.AuthResult{UserID: snaplinkForgeTestUser}, nil
		},
	))
	issuerImpl := defaultimpl.NewEd25519JWTIssuer(
		defaultimpl.WithEd25519Issuer(issuer),
		defaultimpl.WithEd25519TokenTTL(5*time.Minute),
	)
	snaplink := sso.NewServer(
		sso.WithIssuer(issuer),
		sso.WithUserProvider(users),
		sso.WithSessionManager(defaultimpl.NewMemorySessionManager()),
		sso.WithClientStore(clients),
		sso.WithAuthenticator(password),
		sso.WithTokenIssuer("jwt", issuerImpl),
		sso.WithDefaultTokenStrategy("jwt"),
	)
	ssoHTTP.Config.Handler = snaplink.Handler()
	ssoHTTP.StartTLS()
	t.Cleanup(ssoHTTP.Close)

	ssoClient := ssoHTTP.Client()
	tokenA := snaplinkForgeLogin(t, ssoClient, issuer, "forge-console")
	tokenB := snaplinkForgeLogin(t, ssoClient, issuer, "forge-cli")
	if tokenA == tokenB {
		t.Fatal("independent Snaplink clients returned identical access tokens")
	}

	authenticator, err := authn.New(authn.Config{
		Issuer:              issuer,
		Audience:            snaplinkForgeTestAudience,
		JWKSURL:             issuer + "/.well-known/jwks.json",
		ExpectedTenantID:    snaplinkForgeTestTenant,
		ExpectedSubjectID:   snaplinkForgeTestUser,
		JWKSHTTPClient:      ssoClient,
		JWKSRefreshInterval: 24 * time.Hour,
	})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(authenticator.Close)

	backend := &fakeConversationBackend{
		appendPrompt: model.ConversationPrompt{
			ID: "snaplink-prompt", ConversationID: "snaplink-conversation",
			Role: "user", Content: "prompt from Snaplink-authenticated client", CreatedAtMS: 2,
		},
		appendAggVer: 2,
	}
	forgeHTTP := httptest.NewServer(authenticator.Handler(newConversationRoutesWithBackend(backend)))
	t.Cleanup(forgeHTTP.Close)

	created := snaplinkConversationRequest(t, forgeHTTP.Client(), forgeHTTP.URL, tokenA,
		http.MethodPost, conversationCollectionPath, "create-snaplink-conversation",
		`{"scope":{"kind":"global"},"title":"Snaplink owner"}`)
	if created.StatusCode != http.StatusCreated {
		t.Fatalf("Snaplink-authenticated create status=%d body=%q", created.StatusCode, readConversationClientBody(t, created))
	}
	_ = created.Body.Close()
	wantOwner := model.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	if backend.createOwner != wantOwner {
		t.Fatalf("create owner=%#v, want %#v", backend.createOwner, wantOwner)
	}

	listed := snaplinkConversationRequest(t, forgeHTTP.Client(), forgeHTTP.URL, tokenB,
		http.MethodGet, conversationCollectionPath+"?limit=50", "", "")
	if listed.StatusCode != http.StatusOK {
		t.Fatalf("second Snaplink client list status=%d body=%q", listed.StatusCode, readConversationClientBody(t, listed))
	}
	_ = listed.Body.Close()
	if backend.listOwner != wantOwner {
		t.Fatalf("list owner=%#v, want %#v", backend.listOwner, wantOwner)
	}

	prompt := snaplinkConversationRequest(t, forgeHTTP.Client(), forgeHTTP.URL, tokenB,
		http.MethodPost, conversationCollectionPath+"/snaplink-conversation/prompts", "snaplink-prompt-1",
		`{"content":"prompt from Snaplink-authenticated client","expected_version":1}`)
	if prompt.StatusCode != http.StatusCreated {
		t.Fatalf("second Snaplink client prompt status=%d body=%q", prompt.StatusCode, readConversationClientBody(t, prompt))
	}
	_ = prompt.Body.Close()
	if backend.appendOwner != wantOwner || backend.appendID != "snaplink-conversation" {
		t.Fatalf("prompt owner/id=%#v/%q, want %#v/%q", backend.appendOwner, backend.appendID, wantOwner, "snaplink-conversation")
	}
}

func snaplinkForgeLogin(t *testing.T, client *http.Client, issuer, clientID string) string {
	t.Helper()
	body, err := json.Marshal(map[string]any{
		"provider":   "password",
		"client_id":  clientID,
		"credential": map[string]string{"username": snaplinkForgeTestUser, "password": snaplinkForgeTestPassword},
		"scope":      []string{"openid", "profile", "forge:conversations:read", "forge:conversations:write", deviceInventoryReadCandidateScope, devicePlacementRegistryCandidateScope, lifecycleRegistryCandidateReadScope, schedulerSelectionLeaseScope},
		"resource":   []string{snaplinkForgeTestAudience},
	})
	if err != nil {
		t.Fatal(err)
	}
	request, err := http.NewRequest(http.MethodPost, issuer+"/auth/login", bytes.NewReader(body))
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Content-Type", "application/json")
	response, err := client.Do(request)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	responseBody, err := io.ReadAll(response.Body)
	if err != nil {
		t.Fatal(err)
	}
	if response.StatusCode != http.StatusOK {
		t.Fatalf("Snaplink login client=%s status=%d body=%q", clientID, response.StatusCode, responseBody)
	}
	var result struct {
		AccessToken string `json:"access_token"`
	}
	if err := json.Unmarshal(responseBody, &result); err != nil {
		t.Fatalf("decode Snaplink login client=%s: %v; body=%q", clientID, err, responseBody)
	}
	if result.AccessToken == "" {
		t.Fatalf("Snaplink login client=%s returned no access token: %q", clientID, responseBody)
	}
	return result.AccessToken
}

func snaplinkConversationRequest(
	t *testing.T,
	client *http.Client,
	baseURL, token, method, path, idempotencyKey, body string,
) *http.Response {
	t.Helper()
	request, err := http.NewRequest(method, baseURL+path, bytes.NewBufferString(body))
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Authorization", "Bearer "+token)
	if body != "" {
		request.Header.Set("Content-Type", "application/json")
	}
	if idempotencyKey != "" {
		request.Header.Set("Idempotency-Key", idempotencyKey)
	}
	response, err := client.Do(request)
	if err != nil {
		t.Fatal(err)
	}
	return response
}
