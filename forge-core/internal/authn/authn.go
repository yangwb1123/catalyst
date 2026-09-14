// Package authn provides the authenticated HTTP boundary for Forge APIs.
// Principals are derived only from Snaplink-verified access-token claims;
// request paths, query parameters, and bodies are never identity sources.
package authn

import (
	"context"
	"errors"
	"fmt"
	"net/http"
	"net/url"
	"strings"
	"time"
	"unicode"

	"github.com/yangwb1123/snaplink/interfaces/ssoclient/remote"
	"github.com/yangwb1123/snaplink/interfaces/ssoclient/rs"
)

const (
	defaultJWKSMaxBytes = 1 << 20
	defaultHTTPTimeout  = 5 * time.Second
)

// Config pins the expected token issuer and audience. JWKSURL may be omitted,
// in which case Snaplink's conventional JWKS path is derived from Issuer.
// ExpectedTenantID and ExpectedSubjectID, when set, further restrict this
// resource server to one personal Coordinator owner. Every accepted token must
// still carry a non-empty tenant_id claim and subject.
//
// JWKSHTTPClient is only used for retrieving the configured JWKS document.
// Its transport may be customized (for example, to install private roots),
// while redirects, cookies, timeouts, and response size remain controlled by
// this package.
// IntrospectURL opts in to per-request Snaplink introspection. It must be an
// HTTPS URL on the issuer origin and requires a client ID plus a protected
// secret file. This mode does not fall back to local JWKS validation.
type Config struct {
	Issuer            string
	Audience          string
	JWKSURL           string
	ExpectedTenantID  string
	ExpectedSubjectID string

	JWKSHTTPClient      *http.Client
	JWKSMaxBytes        int64
	JWKSRefreshInterval time.Duration

	IntrospectURL        string
	IntrospectClientID   string
	IntrospectSecretFile string
	IntrospectHTTPClient *http.Client
}

// Principal is the identity tuple extracted from a validated access token.
// The strings are exact, case-sensitive issuer, subject, and tenant values.
type Principal struct {
	Issuer   string
	Subject  string
	TenantID string
}

type principalContextKey struct{}

// Authenticator validates Snaplink access tokens and owns the JWKS cache.
// Call Close when the owning server shuts down to stop background refresh.
type Authenticator struct {
	issuer   string
	audience string
	tenant   string
	subject  string
	cache    *rs.JWKSCache
	config   rs.Config
}

// New validates static configuration and creates either local JWKS validation
// or default-off, per-request HTTPS introspection.
func New(cfg Config) (*Authenticator, error) {
	if cfg.JWKSMaxBytes <= 0 {
		cfg.JWKSMaxBytes = defaultJWKSMaxBytes
	}
	_, jwksURL, err := validateConfig(cfg)
	if err != nil {
		return nil, err
	}
	if cfg.IntrospectURL != "" {
		secret, err := loadCredentialFile(cfg.IntrospectSecretFile)
		if err != nil {
			return nil, err
		}
		client := safeRemoteClient(cfg.IntrospectHTTPClient, cfg.JWKSMaxBytes)
		return &Authenticator{
			issuer: cfg.Issuer, audience: cfg.Audience,
			tenant: cfg.ExpectedTenantID, subject: cfg.ExpectedSubjectID,
			config: rs.Config{
				Issuer: cfg.Issuer, ExpectedAud: cfg.Audience,
				IntrospectURL:   cfg.IntrospectURL,
				IntrospectCreds: &rs.ClientCreds{ID: cfg.IntrospectClientID, Secret: secret},
				HTTPClient:      client,
			},
		}, nil
	}

	client := safeRemoteClient(cfg.JWKSHTTPClient, cfg.JWKSMaxBytes)
	options := []rs.JWKSOption{remote.WithJWKSHTTPClient(client)}
	if cfg.JWKSRefreshInterval > 0 {
		options = append(options, remote.WithJWKSRefreshInterval(cfg.JWKSRefreshInterval))
	}
	cache := rs.NewJWKSCache(jwksURL.String(), options...)
	return &Authenticator{
		issuer:   cfg.Issuer,
		audience: cfg.Audience,
		tenant:   cfg.ExpectedTenantID,
		subject:  cfg.ExpectedSubjectID,
		cache:    cache,
		config: rs.Config{
			Issuer:      cfg.Issuer,
			ExpectedAud: cfg.Audience,
			JWKSCache:   cache,
		},
	}, nil
}

// ValidateConfig checks resource-server policy without starting JWKS refresh
// or performing network activity.
func ValidateConfig(cfg Config) error {
	_, _, err := validateConfig(cfg)
	return err
}

func validateConfig(cfg Config) (*url.URL, *url.URL, error) {
	issuerURL, err := validateHTTPSURL("issuer", cfg.Issuer)
	if err != nil {
		return nil, nil, err
	}
	if err := validateIdentityConfig(cfg); err != nil {
		return nil, nil, err
	}
	var sourceURL *url.URL
	if cfg.IntrospectURL != "" {
		sourceURL, err = validateIntrospectionConfig(cfg, issuerURL)
	} else {
		sourceURL, err = validateJWKSConfig(cfg, issuerURL)
	}
	if err != nil {
		return nil, nil, err
	}
	return issuerURL, sourceURL, nil
}

func validateIdentityConfig(cfg Config) error {
	if cfg.Audience == "" || len(cfg.Audience) > 1024 || strings.TrimSpace(cfg.Audience) != cfg.Audience {
		return errors.New("authn: audience is required and must be bounded")
	}
	if len(cfg.Issuer) > 2048 || len(cfg.ExpectedTenantID) > 256 || len(cfg.ExpectedSubjectID) > 255 ||
		!validPrincipalPin(cfg.ExpectedTenantID) || !validPrincipalPin(cfg.ExpectedSubjectID) {
		return errors.New("authn: issuer or principal restriction is invalid")
	}
	return nil
}

func validateIntrospectionConfig(cfg Config, issuerURL *url.URL) (*url.URL, error) {
	if cfg.JWKSURL != "" || cfg.JWKSRefreshInterval != 0 || cfg.JWKSHTTPClient != nil {
		return nil, errors.New("authn: introspection mode cannot be combined with JWKS configuration")
	}
	if !validCredentialIdentifier(cfg.IntrospectClientID) || cfg.IntrospectSecretFile == "" {
		return nil, errors.New("authn: introspection requires a bounded client ID and protected secret file")
	}
	if err := validateCredentialFile(cfg.IntrospectSecretFile); err != nil {
		return nil, err
	}
	introspectURL, err := validateHTTPSURL("introspection URL", cfg.IntrospectURL)
	if err != nil {
		return nil, err
	}
	if origin(issuerURL) != origin(introspectURL) {
		return nil, errors.New("authn: introspection URL must use the issuer's HTTPS origin")
	}
	if cfg.JWKSMaxBytes > defaultJWKSMaxBytes {
		return nil, fmt.Errorf("authn: maximum remote response size cannot exceed %d bytes", defaultJWKSMaxBytes)
	}
	return introspectURL, nil
}

func validateJWKSConfig(cfg Config, issuerURL *url.URL) (*url.URL, error) {
	if cfg.IntrospectClientID != "" || cfg.IntrospectSecretFile != "" || cfg.IntrospectHTTPClient != nil {
		return nil, errors.New("authn: introspection credentials require an introspection URL")
	}
	jwksURLText := cfg.JWKSURL
	if jwksURLText == "" {
		jwksURLText = rs.IssuerJWKSURL(cfg.Issuer)
	}
	jwksURL, err := validateHTTPSURL("JWKS URL", jwksURLText)
	if err != nil {
		return nil, err
	}
	if origin(issuerURL) != origin(jwksURL) {
		return nil, errors.New("authn: JWKS URL must use the issuer's HTTPS origin")
	}
	if cfg.JWKSMaxBytes <= 0 {
		cfg.JWKSMaxBytes = defaultJWKSMaxBytes
	}
	if cfg.JWKSMaxBytes > defaultJWKSMaxBytes {
		return nil, fmt.Errorf("authn: JWKS maximum body size cannot exceed %d bytes", defaultJWKSMaxBytes)
	}
	return jwksURL, nil
}

func validCredentialIdentifier(value string) bool {
	return value != "" && len(value) <= 256 && !strings.Contains(value, ":") &&
		!strings.ContainsFunc(value, unicode.IsControl) && strings.TrimSpace(value) == value
}

func validPrincipalPin(value string) bool {
	return value == "" || (strings.TrimSpace(value) == value &&
		!strings.ContainsFunc(value, unicode.IsControl))
}

// Close stops the background JWKS refresh loop when local validation is used.
// It is safe to call more than once.
func (a *Authenticator) Close() {
	if a != nil && a.cache != nil {
		a.cache.Close()
	}
}

// Handler wraps next in strict Bearer handling and Snaplink JWT validation.
// Tokens with a missing subject or tenant_id claim are rejected after
// signature, issuer, audience, and lifetime verification succeeds.
func (a *Authenticator) Handler(next http.Handler) http.Handler {
	validated := rs.HTTPMiddleware(a.config, http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		claims, ok := rs.ClaimsFromContext(r.Context())
		if !ok || claims == nil {
			writeUnauthorized(w)
			return
		}
		subject, err := rs.RequireSubject(claims)
		if err != nil || strings.TrimSpace(subject) == "" || len(subject) > 255 ||
			strings.TrimSpace(claims.TenantID) == "" || len(claims.TenantID) > 256 ||
			claims.Issuer != a.issuer {
			writeUnauthorized(w)
			return
		}
		if a.tenant != "" && claims.TenantID != a.tenant {
			writeForbidden(w)
			return
		}
		if a.subject != "" && subject != a.subject {
			writeForbidden(w)
			return
		}
		principal := Principal{Issuer: claims.Issuer, Subject: subject, TenantID: claims.TenantID}
		ctx := context.WithValue(r.Context(), principalContextKey{}, principal)
		next.ServeHTTP(w, r.WithContext(ctx))
	}))

	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		values := r.Header.Values("Authorization")
		if len(values) > 1 || (len(values) == 1 && !isStrictBearer(values[0])) || r.Header.Get("DPoP") != "" {
			writeUnauthorized(w)
			return
		}
		validated.ServeHTTP(w, r)
	})
}

// RequireScopes requires every listed scope from the already-validated token.
// It should be placed inside Handler, so only JWT-derived context is accepted.
func RequireScopes(next http.Handler, required ...string) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		claims, ok := rs.ClaimsFromContext(r.Context())
		if !ok || rs.CheckScope(claims, required...) != nil {
			writeForbidden(w)
			return
		}
		next.ServeHTTP(w, r)
	})
}

// PrincipalFromContext returns the trusted issuer/subject/tenant tuple that
// Handler installed after Snaplink validation. It never consults request data.
func PrincipalFromContext(ctx context.Context) (Principal, bool) {
	principal, ok := ctx.Value(principalContextKey{}).(Principal)
	return principal, ok
}

// HasScope reports whether a validated token in ctx carries the exact scope.
func HasScope(ctx context.Context, scope string) bool {
	claims, ok := rs.ClaimsFromContext(ctx)
	return ok && rs.HasScope(claims, scope)
}

// CheckScopes returns nil only when the validated token carries every exact
// requested scope. A context without claims always fails closed.
func CheckScopes(ctx context.Context, required ...string) error {
	claims, ok := rs.ClaimsFromContext(ctx)
	if !ok {
		return errors.New("authn: no validated access token in context")
	}
	if err := rs.CheckScope(claims, required...); err != nil {
		return fmt.Errorf("authn: %w", err)
	}
	return nil
}

func isStrictBearer(value string) bool {
	if strings.ContainsAny(value, "\r\n\t") {
		return false
	}
	parts := strings.Split(value, " ")
	return len(parts) == 2 && strings.EqualFold(parts[0], "Bearer") && parts[1] != "" && !strings.ContainsAny(parts[1], " \t")
}

func validateHTTPSURL(label, raw string) (*url.URL, error) {
	u, err := url.Parse(raw)
	if err != nil || u.Scheme != "https" || u.Hostname() == "" || u.User != nil || u.Opaque != "" || u.RawQuery != "" || u.Fragment != "" {
		return nil, fmt.Errorf("authn: %s must be an absolute HTTPS URL without userinfo, query, or fragment", label)
	}
	return u, nil
}

func origin(u *url.URL) string {
	port := u.Port()
	if port == "" || port == "443" {
		port = "443"
	}
	return strings.ToLower(u.Scheme) + "://" + strings.ToLower(u.Hostname()) + ":" + port
}

func writeUnauthorized(w http.ResponseWriter) {
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("Pragma", "no-cache")
	w.Header().Set("WWW-Authenticate", "Bearer error=\"invalid_token\"")
	writeJSONError(w, http.StatusUnauthorized, `{"error":"invalid_token"}`)
}

func writeForbidden(w http.ResponseWriter) {
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("Pragma", "no-cache")
	writeJSONError(w, http.StatusForbidden, `{"error":"forbidden"}`)
}

func writeJSONError(w http.ResponseWriter, status int, body string) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_, _ = w.Write([]byte(body))
}
