package authn

import (
	"crypto/ed25519"
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

type testIssuer struct {
	server *httptest.Server
	priv   ed25519.PrivateKey
	keyID  string
}

func newTestIssuer(t *testing.T, jwksHandler func(http.ResponseWriter, *http.Request, []byte)) *testIssuer {
	t.Helper()
	pub, priv, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	keyID := "test-key-1"
	jwk := map[string]string{
		"kty": "OKP", "crv": "Ed25519", "kid": keyID, "use": "sig", "alg": "EdDSA",
		"x": base64.RawURLEncoding.EncodeToString(pub),
	}
	keys, err := json.Marshal(map[string]any{"keys": []any{jwk}})
	if err != nil {
		t.Fatal(err)
	}
	srv := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if jwksHandler != nil {
			jwksHandler(w, r, keys)
			return
		}
		if r.URL.Path != "/jwks" {
			http.NotFound(w, r)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write(keys)
	}))
	t.Cleanup(srv.Close)
	return &testIssuer{server: srv, priv: priv, keyID: keyID}
}

func (i *testIssuer) config() Config {
	return Config{
		Issuer:              i.server.URL,
		Audience:            "forge-api",
		JWKSURL:             i.server.URL + "/jwks",
		JWKSHTTPClient:      i.server.Client(),
		JWKSRefreshInterval: 24 * time.Hour,
	}
}

func (i *testIssuer) token(claims map[string]any) string {
	header, _ := json.Marshal(map[string]string{"typ": "at+jwt", "alg": "EdDSA", "kid": i.keyID})
	body, _ := json.Marshal(claims)
	input := base64.RawURLEncoding.EncodeToString(header) + "." + base64.RawURLEncoding.EncodeToString(body)
	sig := ed25519.Sign(i.priv, []byte(input))
	return input + "." + base64.RawURLEncoding.EncodeToString(sig)
}

func validClaims(issuer string) map[string]any {
	return map[string]any{
		"iss":       issuer,
		"sub":       "user-17",
		"aud":       "forge-api",
		"exp":       time.Now().Add(5 * time.Minute).Unix(),
		"iat":       time.Now().Add(-time.Minute).Unix(),
		"tenant_id": "tenant-blue",
		"scope":     "forge:conversations:read forge:conversations:write",
	}
}

func TestHandlerInstallsVerifiedPrincipalAndScopes(t *testing.T) {
	issuer := newTestIssuer(t, nil)
	auth, err := New(issuer.config())
	if err != nil {
		t.Fatal(err)
	}
	defer auth.Close()

	called := false
	endpoint := auth.Handler(RequireScopes(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		called = true
		principal, ok := PrincipalFromContext(r.Context())
		if !ok {
			t.Error("principal missing from context")
		}
		want := Principal{Issuer: issuer.server.URL, Subject: "user-17", TenantID: "tenant-blue"}
		if principal != want {
			t.Errorf("principal = %#v, want %#v", principal, want)
		}
		if !HasScope(r.Context(), "forge:conversations:read") {
			t.Error("verified read scope missing")
		}
		if err := CheckScopes(r.Context(), "forge:conversations:read", "forge:conversations:write"); err != nil {
			t.Errorf("CheckScopes: %v", err)
		}
		w.WriteHeader(http.StatusNoContent)
	}), "forge:conversations:read"))

	req := httptest.NewRequest(http.MethodGet, "/api/v1/conversations", nil)
	req.Header.Set("Authorization", "Bearer "+issuer.token(validClaims(issuer.server.URL)))
	resp := httptest.NewRecorder()
	endpoint.ServeHTTP(resp, req)
	if resp.Code != http.StatusNoContent || !called {
		t.Fatalf("status=%d called=%v body=%q", resp.Code, called, resp.Body.String())
	}
}

func TestHandlerRejectsInvalidIdentityClaims(t *testing.T) {
	issuer := newTestIssuer(t, nil)
	cases := []struct {
		name   string
		mutate func(map[string]any)
	}{
		{name: "missing subject", mutate: func(c map[string]any) { delete(c, "sub") }},
		{name: "missing tenant", mutate: func(c map[string]any) { delete(c, "tenant_id") }},
		{name: "wrong issuer", mutate: func(c map[string]any) { c["iss"] = "https://other.example" }},
		{name: "wrong audience", mutate: func(c map[string]any) { c["aud"] = "other-api" }},
		{name: "expired", mutate: func(c map[string]any) { c["exp"] = time.Now().Add(-2 * time.Minute).Unix() }},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			auth, err := New(issuer.config())
			if err != nil {
				t.Fatal(err)
			}
			defer auth.Close()
			claims := validClaims(issuer.server.URL)
			tc.mutate(claims)
			nextCalled := false
			endpoint := auth.Handler(http.HandlerFunc(func(http.ResponseWriter, *http.Request) { nextCalled = true }))
			req := httptest.NewRequest(http.MethodGet, "/private", nil)
			req.Header.Set("Authorization", "Bearer "+issuer.token(claims))
			resp := httptest.NewRecorder()
			endpoint.ServeHTTP(resp, req)
			if resp.Code != http.StatusUnauthorized || nextCalled {
				t.Fatalf("status=%d nextCalled=%v body=%q", resp.Code, nextCalled, resp.Body.String())
			}
		})
	}
}

func TestHandlerCanPinCoordinatorPrincipal(t *testing.T) {
	issuer := newTestIssuer(t, nil)
	cfg := issuer.config()
	cfg.ExpectedTenantID = "tenant-blue"
	cfg.ExpectedSubjectID = "user-17"
	auth, err := New(cfg)
	if err != nil {
		t.Fatal(err)
	}
	defer auth.Close()
	called := false
	endpoint := auth.Handler(http.HandlerFunc(func(http.ResponseWriter, *http.Request) { called = true }))
	for name, mutate := range map[string]func(map[string]any){
		"wrong tenant":  func(claims map[string]any) { claims["tenant_id"] = "tenant-red" },
		"wrong subject": func(claims map[string]any) { claims["sub"] = "user-18" },
	} {
		t.Run(name, func(t *testing.T) {
			claims := validClaims(issuer.server.URL)
			mutate(claims)
			req := httptest.NewRequest(http.MethodGet, "/private", nil)
			req.Header.Set("Authorization", "Bearer "+issuer.token(claims))
			resp := httptest.NewRecorder()
			endpoint.ServeHTTP(resp, req)
			if resp.Code != http.StatusForbidden || called {
				t.Fatalf("status=%d called=%v, want owner-pinned 403", resp.Code, called)
			}
		})
	}
}

func TestValidateConfigRejectsMalformedCoordinatorPrincipalPins(t *testing.T) {
	issuer := newTestIssuer(t, nil)
	for _, test := range []struct {
		name   string
		mutate func(*Config)
	}{
		{name: "tenant whitespace", mutate: func(config *Config) { config.ExpectedTenantID = " tenant-blue" }},
		{name: "tenant control", mutate: func(config *Config) { config.ExpectedTenantID = "tenant\x1fblue" }},
		{name: "subject whitespace", mutate: func(config *Config) { config.ExpectedSubjectID = " user-17" }},
		{name: "subject control", mutate: func(config *Config) { config.ExpectedSubjectID = "user\x1f17" }},
	} {
		t.Run(test.name, func(t *testing.T) {
			config := issuer.config()
			config.ExpectedTenantID = "tenant-blue"
			config.ExpectedSubjectID = "user-17"
			test.mutate(&config)
			if err := ValidateConfig(config); err == nil {
				t.Fatal("malformed Coordinator principal pin succeeded")
			}
		})
	}
}

func TestHandlerRequiresStrictBearerAndScope(t *testing.T) {
	issuer := newTestIssuer(t, nil)
	auth, err := New(issuer.config())
	if err != nil {
		t.Fatal(err)
	}
	defer auth.Close()

	endpoint := auth.Handler(RequireScopes(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusNoContent)
	}), "forge:devices:write"))
	for _, authorization := range []string{"Basic abc", "Bearer one two", "DPoP abc"} {
		req := httptest.NewRequest(http.MethodGet, "/private", nil)
		req.Header.Set("Authorization", authorization)
		resp := httptest.NewRecorder()
		endpoint.ServeHTTP(resp, req)
		if resp.Code != http.StatusUnauthorized {
			t.Errorf("Authorization %q: status=%d, want 401", authorization, resp.Code)
		}
	}
	parts := strings.Split(issuer.token(validClaims(issuer.server.URL)), ".")
	if parts[2][0] == 'A' {
		parts[2] = "B" + parts[2][1:]
	} else {
		parts[2] = "A" + parts[2][1:]
	}
	tampered := strings.Join(parts, ".")
	req := httptest.NewRequest(http.MethodGet, "/private", nil)
	req.Header.Set("Authorization", "Bearer "+tampered)
	resp := httptest.NewRecorder()
	endpoint.ServeHTTP(resp, req)
	if resp.Code != http.StatusUnauthorized {
		t.Fatalf("tampered signature status=%d, want 401", resp.Code)
	}

	req = httptest.NewRequest(http.MethodGet, "/private", nil)
	req.Header.Set("Authorization", "Bearer "+issuer.token(validClaims(issuer.server.URL)))
	resp = httptest.NewRecorder()
	endpoint.ServeHTTP(resp, req)
	if resp.Code != http.StatusForbidden {
		t.Fatalf("missing scope status=%d, want 403", resp.Code)
	}
}

func TestNewRequiresHTTPSAndSameOriginJWKS(t *testing.T) {
	issuer := newTestIssuer(t, nil)
	base := issuer.config()
	base.Issuer = strings.Replace(base.Issuer, "https://", "http://", 1)
	if _, err := New(base); err == nil {
		t.Fatal("expected HTTP issuer to be rejected")
	}
	base = issuer.config()
	base.JWKSURL = "https://keys.example.invalid/jwks"
	if _, err := New(base); err == nil {
		t.Fatal("expected cross-origin JWKS URL to be rejected")
	}
	base = issuer.config()
	base.JWKSMaxBytes = defaultJWKSMaxBytes + 1
	if _, err := New(base); err == nil {
		t.Fatal("expected excessive JWKS body limit to be rejected")
	}
}

func TestJWKSRedirectIsNotFollowed(t *testing.T) {
	var targetHits int
	issuer := newTestIssuer(t, func(w http.ResponseWriter, r *http.Request, keys []byte) {
		switch r.URL.Path {
		case "/redirect":
			http.Redirect(w, r, "/target", http.StatusFound)
		case "/target":
			targetHits++
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write(keys)
		default:
			http.NotFound(w, r)
		}
	})
	cfg := issuer.config()
	cfg.JWKSURL = issuer.server.URL + "/redirect"
	auth, err := New(cfg)
	if err != nil {
		t.Fatal(err)
	}
	defer auth.Close()
	endpoint := auth.Handler(http.HandlerFunc(func(http.ResponseWriter, *http.Request) {
		t.Error("handler must not accept token when JWKS endpoint redirects")
	}))
	req := httptest.NewRequest(http.MethodGet, "/private", nil)
	req.Header.Set("Authorization", "Bearer "+issuer.token(validClaims(issuer.server.URL)))
	resp := httptest.NewRecorder()
	endpoint.ServeHTTP(resp, req)
	if resp.Code != http.StatusUnauthorized {
		t.Fatalf("status=%d, want 401", resp.Code)
	}
	if targetHits != 0 {
		t.Fatalf("redirect target hit %d times, want 0", targetHits)
	}
}

func TestJWKSResponseBodyIsBounded(t *testing.T) {
	var jwksHits int
	issuer := newTestIssuer(t, func(w http.ResponseWriter, r *http.Request, _ []byte) {
		if r.URL.Path != "/jwks" {
			http.NotFound(w, r)
			return
		}
		jwksHits++
		// No Content-Length is set; the body limit must apply to streamed data.
		w.Header().Set("Content-Type", "application/json")
		w.(http.Flusher).Flush()
		_, _ = fmt.Fprintf(w, `{"keys":[],"padding":"%s"}`, strings.Repeat("x", 2048))
	})
	cfg := issuer.config()
	cfg.JWKSMaxBytes = 1024
	auth, err := New(cfg)
	if err != nil {
		t.Fatal(err)
	}
	defer auth.Close()
	endpoint := auth.Handler(http.HandlerFunc(func(http.ResponseWriter, *http.Request) {
		t.Error("handler must not accept token when JWKS body exceeds the limit")
	}))
	req := httptest.NewRequest(http.MethodGet, "/private", nil)
	req.Header.Set("Authorization", "Bearer "+issuer.token(validClaims(issuer.server.URL)))
	resp := httptest.NewRecorder()
	endpoint.ServeHTTP(resp, req)
	if resp.Code != http.StatusUnauthorized {
		t.Fatalf("status=%d, want 401", resp.Code)
	}
	if jwksHits == 0 {
		t.Fatal("expected JWKS endpoint to be requested")
	}
}

func introspectionConfig(t *testing.T, issuer *testIssuer) Config {
	t.Helper()
	path := filepath.Join(t.TempDir(), "snaplink-introspect.secret")
	if err := os.WriteFile(path, []byte("resource-secret\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	cfg := issuer.config()
	cfg.JWKSURL = ""
	cfg.JWKSHTTPClient = nil
	cfg.JWKSRefreshInterval = 0
	cfg.IntrospectURL = issuer.server.URL + "/introspect"
	cfg.IntrospectClientID = "forge-resource-server"
	cfg.IntrospectSecretFile = path
	cfg.IntrospectHTTPClient = issuer.server.Client()
	return cfg
}

func switchingIntrospectionHandler(active *bool, hits *int, issuerURL *string) func(http.ResponseWriter, *http.Request, []byte) {
	return func(w http.ResponseWriter, r *http.Request, _ []byte) {
		if r.URL.Path != "/introspect" {
			http.NotFound(w, r)
			return
		}
		*hits = *hits + 1
		id, secret, ok := r.BasicAuth()
		if !ok || id != "forge-resource-server" || secret != "resource-secret" {
			http.Error(w, "invalid client", http.StatusUnauthorized)
			return
		}
		if err := r.ParseForm(); err != nil || r.Form.Get("token") != "same-bearer-token" {
			http.Error(w, "invalid form", http.StatusBadRequest)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(map[string]any{
			"active": *active, "iss": *issuerURL, "sub": "user-17", "aud": "forge-api",
			"tenant_id": "tenant-blue", "exp": time.Now().Add(time.Minute).Unix(),
		})
	}
}

func serveBearer(t *testing.T, endpoint http.Handler, token string) *httptest.ResponseRecorder {
	t.Helper()
	req := httptest.NewRequest(http.MethodGet, "/private", nil)
	req.Header.Set("Authorization", "Bearer "+token)
	response := httptest.NewRecorder()
	endpoint.ServeHTTP(response, req)
	return response
}

func TestIntrospectionObservesRevocationOnNextRequest(t *testing.T) {
	active := true
	hits := 0
	issuerURL := ""
	issuer := newTestIssuer(t, switchingIntrospectionHandler(&active, &hits, &issuerURL))
	issuerURL = issuer.server.URL
	auth, err := New(introspectionConfig(t, issuer))
	if err != nil {
		t.Fatal(err)
	}
	defer auth.Close()
	calls := 0
	endpoint := auth.Handler(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		calls++
		w.WriteHeader(http.StatusNoContent)
	}))
	if response := serveBearer(t, endpoint, "same-bearer-token"); response.Code != http.StatusNoContent {
		t.Fatalf("active token status=%d body=%q", response.Code, response.Body.String())
	}
	active = false
	if response := serveBearer(t, endpoint, "same-bearer-token"); response.Code != http.StatusUnauthorized {
		t.Fatalf("revoked token status=%d body=%q, want immediate 401", response.Code, response.Body.String())
	}
	if hits != 2 || calls != 1 {
		t.Fatalf("introspection hits=%d handler calls=%d, want 2 and 1", hits, calls)
	}
}

func TestIntrospectionRejectsRedirectAndUnsafeSecretFiles(t *testing.T) {
	targetHits := 0
	issuer := newTestIssuer(t, func(w http.ResponseWriter, r *http.Request, _ []byte) {
		if r.URL.Path == "/target" {
			targetHits++
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"active":true,"sub":"user-17","aud":"forge-api"}`))
			return
		}
		http.Redirect(w, r, "/target", http.StatusFound)
	})
	cfg := introspectionConfig(t, issuer)
	auth, err := New(cfg)
	if err != nil {
		t.Fatal(err)
	}
	defer auth.Close()
	endpoint := auth.Handler(http.HandlerFunc(func(http.ResponseWriter, *http.Request) {
		t.Error("redirect response must not authenticate a token")
	}))
	req := httptest.NewRequest(http.MethodGet, "/private", nil)
	req.Header.Set("Authorization", "Bearer same-bearer-token")
	response := httptest.NewRecorder()
	endpoint.ServeHTTP(response, req)
	if response.Code != http.StatusUnauthorized || targetHits != 0 {
		t.Fatalf("status=%d redirect target hits=%d, want 401 and no redirect follow", response.Code, targetHits)
	}

	insecure := filepath.Join(t.TempDir(), "readable.secret")
	if err := os.WriteFile(insecure, []byte("secret"), 0o644); err != nil {
		t.Fatal(err)
	}
	cfg.IntrospectSecretFile = insecure
	if err := ValidateConfig(cfg); err == nil {
		t.Fatal("group/world-readable introspection credential file was accepted")
	}
	cfg = introspectionConfig(t, issuer)
	cfg.IntrospectURL = "https://other.example/introspect"
	if err := ValidateConfig(cfg); err == nil {
		t.Fatal("cross-origin introspection endpoint was accepted")
	}
}
