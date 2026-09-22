// Package appserver owns the local Forge Workspace HTTP process boundary.
package appserver

import (
	"fmt"
	"net"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/executionprofile"
)

const (
	// APIVersion identifies the App Server JSON API envelope.
	APIVersion = "forgeos.app-server/v1"
	// DefaultListenAddress is loopback-only by construction.
	DefaultListenAddress  = "127.0.0.1:7467"
	maxStateDirBytes      = 4096
	maxStateDirComponents = 64
)

// BuildInfo is the bounded public build identity exposed by health checks.
type BuildInfo struct {
	Version string
	Commit  string
}

// Config freezes the process-local server configuration before any state or
// listener is opened.
type Config struct {
	ListenAddress                string
	StateDir                     string
	Build                        BuildInfo
	RuntimeExecutable            string
	RuntimeStateDir              string
	SnaplinkIssuer               string
	SnaplinkAudience             string
	SnaplinkJWKSURL              string
	SnaplinkIntrospectURL        string
	SnaplinkIntrospectClientID   string
	SnaplinkIntrospectSecretFile string
	ExpectedTenantID             string
	ExpectedSubjectID            string
	ExecutionProfiles            []executionprofile.Binding
	// DeviceFabricActivation is optional so the zero-value server remains
	// default-off. If a future deployment attempts to mount a device-aware
	// mode, it must provide the complete, accepted decision/evidence bundle.
	DeviceFabricActivation *devicefabricgate.Request
	// DeviceInventoryLifecycleRegistryFile is the owner-private, atomically
	// replaced lifecycle image used by an explicitly accepted inventory,
	// observe, or execute admission activation. It is never read while the
	// fabric gate is off.
	DeviceInventoryLifecycleRegistryFile string
	// DeviceClientInstanceSessionViewFile is an optional owner-private,
	// atomically replaced declaration image for the five Forge client kinds.
	// It is only read by an accepted activation and never grants instance or
	// session authority.
	DeviceClientInstanceSessionViewFile string
	BrowserOrigins                      []string
	TLSCertificateFile                  string
	TLSPrivateKeyFile                   string
	JWKSHTTPClient                      *http.Client
	JWKSMaxBytes                        int64
	JWKSRefreshInterval                 time.Duration
	IntrospectHTTPClient                *http.Client
}

// Validate rejects ambiguous state and unsafe listener, TLS, auth, or browser-origin policy.
func (c Config) Validate() error {
	if _, err := executionprofile.New(c.ExecutionProfiles); err != nil {
		return fmt.Errorf("server execution-profile policy: %w", err)
	}
	if c.DeviceFabricActivation != nil {
		decision := devicefabricgate.Evaluate(*c.DeviceFabricActivation)
		if !decision.Allowed {
			return fmt.Errorf("device fabric activation blocked (%s): %s",
				decision.Mode, strings.Join(decision.Reasons, ","))
		}
		if decision.Mode != devicefabricgate.ModeOff && c.DeviceInventoryLifecycleRegistryFile == "" {
			return fmt.Errorf("device fabric activation requires an owner-private lifecycle registry file")
		}
		if decision.Mode != devicefabricgate.ModeOff &&
			decision.Mode != devicefabricgate.ModeInventory &&
			decision.Mode != devicefabricgate.ModeObserve &&
			decision.Mode != devicefabricgate.ModeExecute {
			return fmt.Errorf("device fabric activation mode %q has no production route assembly", decision.Mode)
		}
		if decision.Mode != devicefabricgate.ModeOff && c.RuntimeExecutable == "" {
			return fmt.Errorf("device fabric activation requires the authenticated session API")
		}
	}
	if c.DeviceInventoryLifecycleRegistryFile != "" {
		if c.DeviceFabricActivation == nil || devicefabricgate.Evaluate(*c.DeviceFabricActivation).Mode == devicefabricgate.ModeOff {
			return fmt.Errorf("device inventory lifecycle registry file requires an enabled device fabric activation")
		}
		if err := validatePrivateFilePath(c.DeviceInventoryLifecycleRegistryFile); err != nil {
			return fmt.Errorf("device inventory lifecycle registry file: %w", err)
		}
	}
	if c.DeviceClientInstanceSessionViewFile != "" {
		if c.DeviceFabricActivation == nil || devicefabricgate.Evaluate(*c.DeviceFabricActivation).Mode == devicefabricgate.ModeOff {
			return fmt.Errorf("device client-instance session view file requires an enabled device fabric activation")
		}
		if err := validatePrivateFilePath(c.DeviceClientInstanceSessionViewFile); err != nil {
			return fmt.Errorf("device client-instance session view file: %w", err)
		}
	}
	if err := validateStateDir(c.StateDir); err != nil {
		return err
	}
	if err := validateListenAddress(c.ListenAddress); err != nil {
		return err
	}
	remote := !isLoopbackListenAddress(c.ListenAddress)
	if remote && !hasCompleteTLS(c) {
		return fmt.Errorf("private-interface listeners require TLS certificate and key")
	}
	if (c.TLSCertificateFile == "") != (c.TLSPrivateKeyFile == "") {
		return fmt.Errorf("TLS certificate and key must be configured together")
	}
	if c.TLSCertificateFile != "" {
		if err := validateTLSFiles(c.TLSCertificateFile, c.TLSPrivateKeyFile); err != nil {
			return err
		}
	}
	if err := validateBrowserOrigins(c.BrowserOrigins); err != nil {
		return err
	}
	if len(c.BrowserOrigins) > 0 && c.RuntimeExecutable == "" {
		return fmt.Errorf("browser origins require the authenticated session API")
	}
	if err := c.validateSessionAPI(remote); err != nil {
		return err
	}
	return c.Build.validate()
}

func validatePrivateFilePath(path string) error {
	if path == "" || len(path) > maxStateDirBytes || strings.ContainsRune(path, 0) || !filepath.IsAbs(path) {
		return fmt.Errorf("path must be a canonical absolute path")
	}
	clean := filepath.Clean(path)
	if path != clean || filepath.Base(clean) == "." || filepath.Base(clean) == string(filepath.Separator) {
		return fmt.Errorf("path must be a canonical absolute file path")
	}
	if err := validateStateDir(filepath.Dir(clean)); err != nil {
		return fmt.Errorf("parent directory: %w", err)
	}
	return nil
}

func (c Config) validateSessionAPI(remote bool) error {
	apiConfigured := c.RuntimeExecutable != "" || c.RuntimeStateDir != "" || c.SnaplinkIssuer != "" ||
		c.SnaplinkAudience != "" || c.SnaplinkJWKSURL != "" || c.SnaplinkIntrospectURL != "" ||
		c.SnaplinkIntrospectClientID != "" || c.SnaplinkIntrospectSecretFile != "" || c.ExpectedTenantID != "" ||
		c.ExpectedSubjectID != "" || c.IntrospectHTTPClient != nil || len(c.ExecutionProfiles) > 0
	if remote && !apiConfigured {
		return fmt.Errorf("private-interface listeners require Runtime and Snaplink API configuration")
	}
	if apiConfigured {
		if c.RuntimeExecutable == "" || c.RuntimeStateDir == "" || c.SnaplinkIssuer == "" || c.SnaplinkAudience == "" {
			return fmt.Errorf("session API requires Runtime path, Runtime state, Snaplink issuer, and audience")
		}
		if c.ExpectedTenantID == "" || c.ExpectedSubjectID == "" {
			return fmt.Errorf("session API requires the exact Coordinator tenant and subject")
		}
		if err := validateRuntimeExecutable(c.RuntimeExecutable); err != nil {
			return fmt.Errorf("Runtime executable must be a canonical absolute path")
		}
		if err := validateStateDir(c.RuntimeStateDir); err != nil {
			return fmt.Errorf("Runtime state directory: %w", err)
		}
		if err := authn.ValidateConfig(authn.Config{
			Issuer: c.SnaplinkIssuer, Audience: c.SnaplinkAudience, JWKSURL: c.SnaplinkJWKSURL,
			ExpectedTenantID: c.ExpectedTenantID, ExpectedSubjectID: c.ExpectedSubjectID,
			JWKSHTTPClient: c.JWKSHTTPClient,
			JWKSMaxBytes:   c.JWKSMaxBytes, JWKSRefreshInterval: c.JWKSRefreshInterval,
			IntrospectURL: c.SnaplinkIntrospectURL, IntrospectClientID: c.SnaplinkIntrospectClientID,
			IntrospectSecretFile: c.SnaplinkIntrospectSecretFile, IntrospectHTTPClient: c.IntrospectHTTPClient,
		}); err != nil {
			return fmt.Errorf("Snaplink resource-server configuration: %w", err)
		}
	}
	return nil
}

func validateBrowserOrigins(origins []string) error {
	seen := make(map[string]struct{}, len(origins))
	for _, origin := range origins {
		parsed, err := url.Parse(origin)
		if err != nil || parsed.Scheme == "" || parsed.Host == "" || parsed.Hostname() == "" || strings.Contains(parsed.Host, "*") ||
			parsed.User != nil || parsed.Opaque != "" ||
			parsed.Path != "" || parsed.RawQuery != "" || parsed.Fragment != "" || parsed.ForceQuery ||
			parsed.Scheme != "https" && !isLocalHTTPOrigin(parsed) ||
			origin != strings.ToLower(parsed.Scheme)+"://"+strings.ToLower(parsed.Host) {
			return fmt.Errorf("browser origins must be exact HTTPS origins (local HTTP is allowed only on loopback)")
		}
		if port := parsed.Port(); port != "" {
			portNumber, parseErr := strconv.Atoi(port)
			if parseErr != nil || portNumber < 1 || portNumber > 65535 ||
				parsed.Scheme == "https" && port == "443" || parsed.Scheme == "http" && port == "80" {
				return fmt.Errorf("browser origins must use a canonical, non-default port")
			}
		}
		if _, exists := seen[origin]; exists {
			return fmt.Errorf("browser origins must be unique")
		}
		seen[origin] = struct{}{}
	}
	return nil
}

func isLocalHTTPOrigin(origin *url.URL) bool {
	if origin.Scheme != "http" {
		return false
	}
	host := strings.TrimSuffix(strings.ToLower(origin.Hostname()), ".")
	if host == "localhost" {
		return true
	}
	ip := net.ParseIP(host)
	return ip != nil && ip.IsLoopback()
}

func validateStateDir(path string) error {
	if path == "" {
		return fmt.Errorf("state-dir must be a nonempty absolute path")
	}
	if len(path) > maxStateDirBytes {
		return fmt.Errorf("state-dir exceeds %d bytes", maxStateDirBytes)
	}
	if statePathComponents(path) > maxStateDirComponents {
		return fmt.Errorf("state-dir exceeds %d components", maxStateDirComponents)
	}
	if !filepath.IsAbs(path) || strings.ContainsRune(path, 0) {
		return fmt.Errorf("state-dir must be a nonempty absolute path")
	}
	clean := filepath.Clean(path)
	if path != clean {
		return fmt.Errorf("state-dir must be a canonical absolute path")
	}
	if filepath.Dir(clean) == clean {
		return fmt.Errorf("state-dir must not be a filesystem root")
	}
	return nil
}

func statePathComponents(path string) int {
	components, inside := 0, false
	for index := 0; index < len(path); index++ {
		separator := path[index] == byte(filepath.Separator) ||
			filepath.Separator == '\\' && path[index] == '/'
		if separator {
			inside = false
		} else if !inside {
			components++
			inside = true
		}
	}
	return components
}

func validateListenAddress(address string) error {
	host, portText, err := net.SplitHostPort(address)
	if err != nil {
		return fmt.Errorf("listen address must be host:port: %w", err)
	}
	ip := net.ParseIP(host)
	if ip == nil || (!ip.IsLoopback() && (!ip.IsPrivate() || ip.IsLinkLocalUnicast() || ip.IsMulticast())) {
		return fmt.Errorf("listen host must be a literal loopback or private unicast IP")
	}
	port, err := strconv.Atoi(portText)
	if err != nil || port < 0 || port > 65535 {
		return fmt.Errorf("listen port must be an integer in 0..65535")
	}
	if !ip.IsLoopback() && port == 0 {
		return fmt.Errorf("private-interface listener must use a fixed nonzero port")
	}
	return nil
}

func isLoopbackListenAddress(address string) bool {
	host, _, err := net.SplitHostPort(address)
	if err != nil {
		return false
	}
	ip := net.ParseIP(host)
	return ip != nil && ip.IsLoopback()
}

func hasCompleteTLS(config Config) bool {
	return config.TLSCertificateFile != "" && config.TLSPrivateKeyFile != ""
}

func validateTLSFiles(certificatePath, keyPath string) error {
	for _, path := range []string{certificatePath, keyPath} {
		if path == "" || !filepath.IsAbs(path) || strings.ContainsRune(path, 0) || filepath.Clean(path) != path {
			return fmt.Errorf("TLS certificate and key paths must be canonical absolute paths")
		}
		info, err := os.Lstat(path)
		if err != nil || !info.Mode().IsRegular() || info.Mode()&os.ModeSymlink != 0 {
			return fmt.Errorf("TLS certificate and key must be regular files")
		}
	}
	if err := validatePrivateKeyPermissions(keyPath); err != nil {
		return err
	}
	return nil
}

func validatePrivateKeyPermissions(path string) error {
	info, err := os.Stat(path)
	if err != nil {
		return fmt.Errorf("TLS private key cannot be inspected")
	}
	if info.Mode().Perm()&0o077 != 0 {
		return fmt.Errorf("TLS private key must not be accessible by group or others")
	}
	return nil
}

func validateRuntimeExecutable(path string) error {
	if path == "" || !filepath.IsAbs(path) || strings.ContainsRune(path, 0) || filepath.Clean(path) != path {
		return fmt.Errorf("Runtime executable must be a canonical absolute path")
	}
	info, err := os.Lstat(path)
	if err != nil || !info.Mode().IsRegular() || info.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("Runtime executable must be a regular file")
	}
	return nil
}

func (b BuildInfo) validate() error {
	if err := validateBuildValue("version", b.Version, true); err != nil {
		return err
	}
	return validateBuildValue("commit", b.Commit, false)
}

func validateBuildValue(name, value string, required bool) error {
	if required && value == "" {
		return fmt.Errorf("build %s must be nonempty", name)
	}
	if len(value) > 128 {
		return fmt.Errorf("build %s exceeds 128 bytes", name)
	}
	for _, character := range value {
		if !isBuildCharacter(character) {
			return fmt.Errorf("build %s contains an unsupported character", name)
		}
	}
	return nil
}

func isBuildCharacter(character rune) bool {
	return character >= 'a' && character <= 'z' ||
		character >= 'A' && character <= 'Z' ||
		character >= '0' && character <= '9' ||
		strings.ContainsRune(".+-_", character)
}
