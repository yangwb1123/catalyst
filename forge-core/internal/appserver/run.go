package appserver

import (
	"context"
	"crypto/tls"
	"errors"
	"fmt"
	"net"
	"net/http"
	"time"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/controlstore"
	"forgeos/forge-core/internal/executionprofile"
	"forgeos/forge-core/internal/runtimebridge"
)

const shutdownTimeout = 5 * time.Second

const announceTimeout = 5 * time.Second

var errAnnouncerRequired = errors.New("server readiness announcer must be non-nil")

// Ready is the metadata-only startup receipt emitted after listener binding
// and before Serve begins accepting HTTP requests.
type Ready struct {
	APIVersion string `json:"api_version"`
	Event      string `json:"event"`
	Listen     string `json:"listen"`
}

// Announce publishes the startup receipt without granting server authority.
type Announce func(Ready) error

// Run owns one loopback or private-interface listener and one state-directory instance lock until
// cancellation or an HTTP serving failure.
func Run(ctx context.Context, config Config, announce Announce) (runErr error) {
	if ctx == nil {
		return fmt.Errorf("server context must be non-nil")
	}
	if err := config.Validate(); err != nil {
		return err
	}
	profileCatalog, err := executionprofile.New(config.ExecutionProfiles)
	if err != nil {
		return fmt.Errorf("configure server execution profiles: %w", err)
	}
	if err := ctx.Err(); err != nil {
		return fmt.Errorf("server context ended before startup: %w", err)
	}
	if announce == nil {
		return errAnnouncerRequired
	}
	certificate, err := loadServerCertificate(config)
	if err != nil {
		return err
	}
	lock, err := acquireInstanceLock(config.StateDir)
	if err != nil {
		return err
	}
	defer func() { _ = lock.Close() }()
	store, err := controlstore.OpenBound(ctx, lock.root)
	if err != nil {
		return fmt.Errorf("open control store: %w", err)
	}
	defer closeControlStore(store, &runErr)
	if err := verifyControlStateLayout(lock.root); err != nil {
		return fmt.Errorf("verify control store files: %w", err)
	}
	sessionHandler, closeAuthenticator, err := configureSessionHandler(config, profileCatalog)
	if err != nil {
		return err
	}
	defer closeAuthenticator()
	return serveConfiguredServer(ctx, config, announce, certificate, sessionHandler)
}

func loadServerCertificate(config Config) (*tls.Certificate, error) {
	if !hasCompleteTLS(config) {
		return nil, nil
	}
	loaded, err := tls.LoadX509KeyPair(config.TLSCertificateFile, config.TLSPrivateKeyFile)
	if err != nil {
		return nil, fmt.Errorf("load TLS certificate: %w", err)
	}
	return &loaded, nil
}

func configureSessionHandler(
	config Config,
	profileCatalog *executionprofile.Catalog,
) (http.Handler, func(), error) {
	if config.RuntimeExecutable == "" {
		return nil, func() {}, nil
	}
	runtimeClient, err := runtimebridge.New(runtimebridge.Config{
		Executable: config.RuntimeExecutable, AppServerStateDir: config.StateDir,
		RuntimeStateDir: config.RuntimeStateDir,
	})
	if err != nil {
		return nil, nil, fmt.Errorf("configure Runtime bridge: %w", err)
	}
	authenticator, err := authn.New(authn.Config{
		Issuer: config.SnaplinkIssuer, Audience: config.SnaplinkAudience,
		JWKSURL: config.SnaplinkJWKSURL, ExpectedTenantID: config.ExpectedTenantID,
		ExpectedSubjectID: config.ExpectedSubjectID,
		JWKSHTTPClient:    config.JWKSHTTPClient, JWKSMaxBytes: config.JWKSMaxBytes,
		JWKSRefreshInterval: config.JWKSRefreshInterval, IntrospectURL: config.SnaplinkIntrospectURL,
		IntrospectClientID:   config.SnaplinkIntrospectClientID,
		IntrospectSecretFile: config.SnaplinkIntrospectSecretFile,
		IntrospectHTTPClient: config.IntrospectHTTPClient,
	})
	if err != nil {
		return nil, nil, fmt.Errorf("configure Snaplink resource server: %w", err)
	}
	sessions, err := newAuthenticatedSessionRoutesWithDeviceFabricActivationAndRunnerAuthorityWithBackend(
		runtimeClient, profileCatalog, config.DeviceFabricActivation,
		config.DeviceInventoryLifecycleRegistryFile,
		config.DeviceClientInstanceSessionViewFile,
		config.RunnerExecutionAuthority,
		runtimeClient,
		config.DeviceExecutionLeaseRegistryFile,
		config.DeviceExecutionPolicyRegistryFile,
	)
	if err != nil {
		return nil, nil, fmt.Errorf("configure device fabric session routes: %w", err)
	}
	handler := allowBrowserOrigins(
		authenticator.Handler(sessions), config.BrowserOrigins,
	)
	return handler, authenticator.Close, nil
}

func serveConfiguredServer(
	ctx context.Context,
	config Config,
	announce Announce,
	certificate *tls.Certificate,
	sessionHandler http.Handler,
) error {
	listener, err := net.Listen("tcp", config.ListenAddress)
	if err != nil {
		return fmt.Errorf("listen on %s: %w", config.ListenAddress, err)
	}
	defer func() { _ = listener.Close() }()
	routes, err := newRoutesWithSessions(config.Build, listener.Addr().String(), maxInFlightRequests, sessionHandler)
	if err != nil {
		return err
	}
	scheme := "http"
	if certificate != nil {
		scheme = "https"
	}
	if err := announceListener(ctx, listener, announce, scheme); err != nil {
		return err
	}
	if ctx.Err() != nil {
		return nil
	}
	bounded := limitConnections(listener, maxServerConnections)
	server := newHTTPServer(routes)
	if certificate != nil {
		server.TLSConfig = &tls.Config{
			MinVersion: tls.VersionTLS13, Certificates: []tls.Certificate{*certificate},
		}
	}
	return serve(ctx, server, bounded, certificate != nil)
}

func closeControlStore(store interface{ Close() error }, runErr *error) {
	if err := store.Close(); err != nil {
		*runErr = errors.Join(*runErr, fmt.Errorf("close control store: %w", err))
	}
}

func newHTTPServer(routes http.Handler) *http.Server {
	return &http.Server{
		Handler: routes, ReadHeaderTimeout: 5 * time.Second,
		ReadTimeout: 10 * time.Second, WriteTimeout: 10 * time.Second,
		IdleTimeout: 60 * time.Second, MaxHeaderBytes: 32 * 1024,
		DisableGeneralOptionsHandler: true,
	}
}

func announceListener(ctx context.Context, listener net.Listener, announce Announce, scheme string) error {
	ready := Ready{APIVersion: APIVersion, Event: "listening", Listen: scheme + "://" + listener.Addr().String()}
	return awaitAnnouncement(ctx, announce, ready, announceTimeout)
}

func awaitAnnouncement(ctx context.Context, announce Announce, ready Ready, timeout time.Duration) error {
	if announce == nil {
		return errAnnouncerRequired
	}
	result := make(chan error, 1)
	go func() { result <- announce(ready) }()
	timer := time.NewTimer(timeout)
	defer timer.Stop()
	select {
	case err := <-result:
		if err != nil {
			return fmt.Errorf("announce server readiness: %w", err)
		}
		return nil
	case <-ctx.Done():
		return nil
	case <-timer.C:
		return fmt.Errorf("announce server readiness: timed out after %s", timeout)
	}
}

func serve(ctx context.Context, server *http.Server, listener net.Listener, useTLS bool) error {
	result := make(chan error, 1)
	go func() {
		if useTLS {
			result <- normalizeServeError(server.ServeTLS(listener, "", ""))
			return
		}
		result <- normalizeServeError(server.Serve(listener))
	}()
	select {
	case err := <-result:
		return err
	case <-ctx.Done():
		return shutdown(server, result)
	}
}

func normalizeServeError(err error) error {
	if errors.Is(err, http.ErrServerClosed) {
		return nil
	}
	return err
}

func shutdown(server *http.Server, result <-chan error) error {
	ctx, cancel := context.WithTimeout(context.Background(), shutdownTimeout)
	defer cancel()
	shutdownErr := server.Shutdown(ctx)
	if shutdownErr != nil {
		_ = server.Close()
	}
	serveErr := <-result
	if shutdownErr != nil {
		return fmt.Errorf("shutdown server: %w", shutdownErr)
	}
	return serveErr
}
