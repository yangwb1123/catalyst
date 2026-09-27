package appserver

// This test exercises the opt-in Forge resource-server introspection path
// against a real Snaplink SSO HTTPS handler. It is deliberately test-only:
// all users, clients, keys, and state are in memory or a temporary 0600
// credential file, and no device or Runner surface is registered.

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/yangwb1123/snaplink/domains/authenticators"
	"github.com/yangwb1123/snaplink/infrastructure/defaultimpl"
	"github.com/yangwb1123/snaplink/interfaces/sso"

	"forgeos/forge-core/internal/authn"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

const (
	snaplinkForgeIntrospectionClient = "forge-introspector"
	// secret-scan:ignore — in-memory introspection fixture credential.
	snaplinkForgeIntrospectionClientSecret = "snaplink-forge-introspection-secret"
	snaplinkForgeOtherTenant               = "snaplink-forge-other-tenant"
)

type snaplinkIntrospectionCounter struct {
	mu    sync.Mutex
	paths []string
}

func TestSnaplinkIntrospectionAuthenticatedConversationOwnerParity(t *testing.T) {
	fixture := newSnaplinkIntrospectionFixture(t)

	backend := &fakeConversationBackend{
		appendPrompt: model.ConversationPrompt{
			ID: "snaplink-introspection-prompt", ConversationID: "snaplink-introspection-conversation",
			Role: "user", Content: "prompt through Snaplink introspection", CreatedAtMS: 2,
		},
		appendAggVer: 2,
	}
	forgeHTTP := httptest.NewServer(fixture.authenticator.Handler(newConversationRoutesWithBackend(backend)))
	t.Cleanup(forgeHTTP.Close)

	created := snaplinkConversationRequest(t, forgeHTTP.Client(), forgeHTTP.URL, fixture.userToken,
		http.MethodPost, conversationCollectionPath, "introspection-create",
		`{"scope":{"kind":"global"},"title":"Snaplink introspection"}`)
	if created.StatusCode != http.StatusCreated {
		t.Fatalf("introspection create status=%d body=%q", created.StatusCode, readConversationClientBody(t, created))
	}
	_ = created.Body.Close()

	listed := snaplinkConversationRequest(t, forgeHTTP.Client(), forgeHTTP.URL, fixture.userToken,
		http.MethodGet, conversationCollectionPath+"?limit=50", "", "")
	if listed.StatusCode != http.StatusOK {
		t.Fatalf("introspection list status=%d body=%q", listed.StatusCode, readConversationClientBody(t, listed))
	}
	_ = listed.Body.Close()

	prompt := snaplinkConversationRequest(t, forgeHTTP.Client(), forgeHTTP.URL, fixture.userToken,
		http.MethodPost, conversationCollectionPath+"/snaplink-introspection-conversation/prompts", "introspection-prompt",
		`{"content":"prompt through Snaplink introspection","expected_version":1}`)
	if prompt.StatusCode != http.StatusCreated {
		t.Fatalf("introspection prompt status=%d body=%q", prompt.StatusCode, readConversationClientBody(t, prompt))
	}
	_ = prompt.Body.Close()

	wantOwner := model.Owner{Issuer: fixture.issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	if backend.createOwner != wantOwner || backend.listOwner != wantOwner || backend.appendOwner != wantOwner {
		t.Fatalf("owner parity create/list/prompt=%#v/%#v/%#v, want %#v", backend.createOwner, backend.listOwner, backend.appendOwner, wantOwner)
	}
	fixture.assertIntrospectionPaths(t, 3)

	// Revocation is performed by the client that minted the access token. The
	// following Forge request must call Snaplink again and observe active:false;
	// a cached JWKS verdict or a positive introspection cache would incorrectly
	// allow the request through.
	revoke := snaplinkRevokeToken(t, fixture.ssoClient, fixture.issuer, snaplinkForgeConsoleClient, "", fixture.userToken)
	if revoke.StatusCode != http.StatusOK {
		t.Fatalf("Snaplink revoke status=%d body=%q", revoke.StatusCode, readConversationClientBody(t, revoke))
	}
	_ = revoke.Body.Close()
	afterRevoke := snaplinkConversationRequest(t, forgeHTTP.Client(), forgeHTTP.URL, fixture.userToken,
		http.MethodGet, conversationCollectionPath+"?limit=50", "", "")
	if afterRevoke.StatusCode != http.StatusUnauthorized {
		t.Fatalf("revoked token status=%d body=%q, want 401", afterRevoke.StatusCode, readConversationClientBody(t, afterRevoke))
	}
	_ = afterRevoke.Body.Close()
	fixture.assertIntrospectionPaths(t, 4)
}

func TestSnaplinkIntrospectionFailsClosedForAudienceTenantSecretAndScope(t *testing.T) {
	fixture := newSnaplinkIntrospectionFixture(t)
	backend := &fakeConversationBackend{}
	forgeHTTP := httptest.NewServer(fixture.authenticator.Handler(newConversationRoutesWithBackend(backend)))
	t.Cleanup(forgeHTTP.Close)

	wrongAudience := snaplinkForgeLoginWithPolicy(t, fixture.ssoClient, fixture.issuer,
		snaplinkForgeWrongAudienceClient, []string{"openid", "profile", "forge:conversations:read", "forge:conversations:write"}, []string{"other-api"})
	wrongTenant := snaplinkForgeLoginWithPolicy(t, fixture.ssoClient, fixture.issuer,
		snaplinkForgeWrongTenantClient, []string{"openid", "profile", "forge:conversations:read", "forge:conversations:write"}, []string{snaplinkForgeTestAudience})
	readOnly := snaplinkForgeLoginWithPolicy(t, fixture.ssoClient, fixture.issuer,
		snaplinkForgeReadOnlyClient, []string{"openid", "profile", "forge:conversations:read"}, []string{snaplinkForgeTestAudience})

	tests := []struct {
		name   string
		token  string
		status int
		path   string
	}{
		{name: "wrong audience", token: wrongAudience, status: http.StatusUnauthorized, path: conversationCollectionPath + "?limit=50"},
		{name: "wrong tenant", token: wrongTenant, status: http.StatusForbidden, path: conversationCollectionPath + "?limit=50"},
		{name: "missing write scope", token: readOnly, status: http.StatusForbidden, path: conversationCollectionPath},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			response := snaplinkConversationRequest(t, forgeHTTP.Client(), forgeHTTP.URL, test.token,
				methodForIntrospectionCase(test.path), test.path, "fail-closed-"+strings.ReplaceAll(test.name, " ", "-"),
				`{"scope":{"kind":"global"},"title":"should fail"}`)
			if response.StatusCode != test.status {
				t.Fatalf("status=%d body=%q, want %d", response.StatusCode, readConversationClientBody(t, response), test.status)
			}
			_ = response.Body.Close()
		})
	}
	if backend.createCalls != 0 || backend.listCalls != 0 {
		t.Fatalf("fail-closed requests reached backend: create=%d list=%d", backend.createCalls, backend.listCalls)
	}
	fixture.assertIntrospectionPaths(t, len(tests))

	wrongSecretFile := filepath.Join(t.TempDir(), "wrong-secret")
	if err := os.WriteFile(wrongSecretFile, []byte("wrong-secret\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	wrongSecretAuth, err := authn.New(fixture.authConfig(wrongSecretFile))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(wrongSecretAuth.Close)
	wrongSecretHTTP := httptest.NewServer(wrongSecretAuth.Handler(newConversationRoutesWithBackend(&fakeConversationBackend{})))
	t.Cleanup(wrongSecretHTTP.Close)
	wrongSecret := snaplinkConversationRequest(t, wrongSecretHTTP.Client(), wrongSecretHTTP.URL, fixture.userToken,
		http.MethodGet, conversationCollectionPath+"?limit=50", "", "")
	if wrongSecret.StatusCode != http.StatusUnauthorized {
		t.Fatalf("wrong introspection secret status=%d body=%q, want 401", wrongSecret.StatusCode, readConversationClientBody(t, wrongSecret))
	}
	_ = wrongSecret.Body.Close()
	fixture.assertIntrospectionPaths(t, len(tests)+1)
}

const (
	snaplinkForgeConsoleClient       = "forge-console"
	snaplinkForgeWrongAudienceClient = "forge-wrong-audience"
	snaplinkForgeWrongTenantClient   = "forge-wrong-tenant"
	snaplinkForgeReadOnlyClient      = "forge-read-only"
)

type snaplinkIntrospectionFixture struct {
	issuer        string
	ssoClient     *http.Client
	userToken     string
	authenticator *authn.Authenticator
	counter       *snaplinkIntrospectionCounter
	secretFile    string
	remoteClient  *http.Client
}

func newSnaplinkIntrospectionFixture(t *testing.T) *snaplinkIntrospectionFixture {
	t.Helper()
	ctx := context.Background()
	ssoHTTP := httptest.NewUnstartedServer(nil)
	issuer := "https://" + ssoHTTP.Listener.Addr().String()

	users := defaultimpl.NewMemoryUserProvider()
	if err := users.CreateOrUpdate(ctx, &sso.User{ID: snaplinkForgeTestUser, Name: "Forge Snaplink Introspection User"}); err != nil {
		t.Fatal(err)
	}
	clients := defaultimpl.NewMemoryClientStore()
	forgeScopes := []string{"openid", "profile", "forge:conversations:read", "forge:conversations:write"}
	seed := func(client *sso.Client) {
		client.AllowedAuthenticators = []string{"password"}
		client.TokenStrategy = "jwt"
		client.Active = true
		client.SkipConsent = true
		clients.AddSeed(client)
	}
	seed(&sso.Client{
		ID:   snaplinkForgeConsoleClient,
		Name: snaplinkForgeConsoleClient, TenantID: snaplinkForgeTestTenant,
		AllowedScopes: forgeScopes, AllowedResources: []string{snaplinkForgeTestAudience},
		TokenEndpointAuthMethod: "none",
	})
	seed(&sso.Client{
		ID: snaplinkForgeWrongAudienceClient, Name: snaplinkForgeWrongAudienceClient,
		TenantID: snaplinkForgeTestTenant, AllowedScopes: forgeScopes,
		AllowedResources: []string{"other-api"}, TokenEndpointAuthMethod: "none",
	})
	seed(&sso.Client{
		ID: snaplinkForgeWrongTenantClient, Name: snaplinkForgeWrongTenantClient,
		TenantID: snaplinkForgeOtherTenant, AllowedScopes: forgeScopes,
		AllowedResources: []string{snaplinkForgeTestAudience}, TokenEndpointAuthMethod: "none",
	})
	seed(&sso.Client{
		ID: snaplinkForgeReadOnlyClient, Name: snaplinkForgeReadOnlyClient,
		TenantID:         snaplinkForgeTestTenant,
		AllowedScopes:    []string{"openid", "profile", "forge:conversations:read"},
		AllowedResources: []string{snaplinkForgeTestAudience}, TokenEndpointAuthMethod: "none",
	})
	seed(&sso.Client{
		ID: snaplinkForgeIntrospectionClient, Secret: snaplinkForgeIntrospectionClientSecret,
		Name: snaplinkForgeIntrospectionClient, TenantID: snaplinkForgeTestTenant,
		TokenEndpointAuthMethod: "client_secret_basic",
	})

	password := authenticators.NewPasswordAuthenticator(authenticators.PasswordVerifierFunc(
		func(_ context.Context, username, supplied string) (*sso.AuthResult, error) {
			if username != snaplinkForgeTestUser || supplied != snaplinkForgeTestPassword {
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
		sso.WithIssuer(issuer), sso.WithUserProvider(users),
		sso.WithSessionManager(defaultimpl.NewMemorySessionManager()),
		sso.WithClientStore(clients), sso.WithAuthenticator(password),
		sso.WithTokenIssuer("jwt", issuerImpl), sso.WithDefaultTokenStrategy("jwt"),
	)
	ssoHTTP.Config.Handler = snaplink.Handler()
	ssoHTTP.StartTLS()
	t.Cleanup(ssoHTTP.Close)

	ssoClient := ssoHTTP.Client()
	secretFile := filepath.Join(t.TempDir(), "snaplink-introspection.secret")
	if err := os.WriteFile(secretFile, []byte(snaplinkForgeIntrospectionClientSecret+"\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(secretFile)
	if err != nil || info.Mode().Perm() != 0o600 {
		t.Fatalf("introspection secret mode=%v err=%v, want 0600", info.Mode().Perm(), err)
	}
	counter := &snaplinkIntrospectionCounter{}
	// Keep the httptest TLS transport and wrap it only to count Forge's
	// per-request calls. Authn.New adds its own bounded response transport.
	baseTransport := ssoClient.Transport
	countingTransport := &countingRoundTripper{base: baseTransport, counter: counter}
	introspectClient := &http.Client{Transport: countingTransport, Timeout: ssoClient.Timeout}

	authenticator, err := authn.New(authn.Config{
		Issuer: issuer, Audience: snaplinkForgeTestAudience,
		ExpectedTenantID: snaplinkForgeTestTenant, ExpectedSubjectID: snaplinkForgeTestUser,
		IntrospectURL:        issuer + "/token/introspect",
		IntrospectClientID:   snaplinkForgeIntrospectionClient,
		IntrospectSecretFile: secretFile, IntrospectHTTPClient: introspectClient,
	})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(authenticator.Close)
	userToken := snaplinkForgeLoginWithPolicy(t, ssoClient, issuer, snaplinkForgeConsoleClient,
		forgeScopes, []string{snaplinkForgeTestAudience})
	return &snaplinkIntrospectionFixture{
		issuer: issuer, ssoClient: ssoClient,
		userToken:     userToken,
		authenticator: authenticator, counter: counter, secretFile: secretFile, remoteClient: introspectClient,
	}
}

// countingRoundTripper keeps the actual TLS transport and records only the
// path. A Forge introspection client must never silently fall back to JWKS.
type countingRoundTripper struct {
	base    http.RoundTripper
	counter *snaplinkIntrospectionCounter
}

func (t *countingRoundTripper) RoundTrip(request *http.Request) (*http.Response, error) {
	t.counter.mu.Lock()
	t.counter.paths = append(t.counter.paths, request.URL.Path)
	t.counter.mu.Unlock()
	return t.base.RoundTrip(request)
}

func (f *snaplinkIntrospectionFixture) authConfig(secretFile string) authn.Config {
	return authn.Config{
		Issuer: f.issuer, Audience: snaplinkForgeTestAudience,
		ExpectedTenantID: snaplinkForgeTestTenant, ExpectedSubjectID: snaplinkForgeTestUser,
		IntrospectURL:        f.issuer + "/token/introspect",
		IntrospectClientID:   snaplinkForgeIntrospectionClient,
		IntrospectSecretFile: secretFile,
		IntrospectHTTPClient: f.remoteClient,
	}
}

func (f *snaplinkIntrospectionFixture) assertIntrospectionPaths(t *testing.T, want int) {
	t.Helper()
	f.counter.mu.Lock()
	defer f.counter.mu.Unlock()
	if len(f.counter.paths) != want {
		t.Fatalf("introspection request count=%d, want %d (paths=%v)", len(f.counter.paths), want, f.counter.paths)
	}
	for _, path := range f.counter.paths {
		if path != "/token/introspect" {
			t.Fatalf("Forge remote auth request path=%q, want /token/introspect (paths=%v)", path, f.counter.paths)
		}
	}
}

func snaplinkForgeLoginWithPolicy(t *testing.T, client *http.Client, issuer, clientID string, scopes, resources []string) string {
	t.Helper()
	body, err := json.Marshal(map[string]any{
		"provider": "password", "client_id": clientID,
		"credential": map[string]string{"username": snaplinkForgeTestUser, "password": snaplinkForgeTestPassword},
		"scope":      scopes, "resource": resources,
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
	if err := json.Unmarshal(responseBody, &result); err != nil || result.AccessToken == "" {
		t.Fatalf("decode Snaplink login client=%s: %v body=%q", clientID, err, responseBody)
	}
	return result.AccessToken
}

func snaplinkRevokeToken(t *testing.T, client *http.Client, issuer, clientID, secret, token string) *http.Response {
	t.Helper()
	form := url.Values{"token": {token}, "token_type_hint": {"access_token"}}
	if secret == "" {
		form.Set("client_id", clientID)
	}
	request, err := http.NewRequest(http.MethodPost, issuer+"/token/revoke", strings.NewReader(form.Encode()))
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	if secret != "" {
		request.SetBasicAuth(clientID, secret)
	}
	response, err := client.Do(request)
	if err != nil {
		t.Fatal(err)
	}
	return response
}

func methodForIntrospectionCase(path string) string {
	if path == conversationCollectionPath {
		return http.MethodPost
	}
	return http.MethodGet
}
