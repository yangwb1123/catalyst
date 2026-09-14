package appserver

import (
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/executionprofile"
)

func testConfig(t *testing.T) Config {
	t.Helper()
	return Config{
		ListenAddress: "127.0.0.1:0",
		StateDir:      filepath.Join(t.TempDir(), "state"),
		Build:         BuildInfo{Version: "dev", Commit: "abc123"},
	}
}

func TestConfigAcceptsLiteralLoopbackAddresses(t *testing.T) {
	for _, address := range []string{"127.0.0.1:0", "127.0.0.1:7467", "[::1]:7467"} {
		config := testConfig(t)
		config.ListenAddress = address
		if err := config.Validate(); err != nil {
			t.Errorf("Validate(%q): %v", address, err)
		}
	}
}

func TestConfigRejectsUnsafeListenAddresses(t *testing.T) {
	addresses := []string{"", "localhost:7467", "0.0.0.0:7467", "[::]:7467", "127.0.0.1:-1", "127.0.0.1:65536"}
	for _, address := range addresses {
		config := testConfig(t)
		config.ListenAddress = address
		if err := config.Validate(); err == nil {
			t.Errorf("Validate(%q) succeeded", address)
		}
	}
}

func TestConfigRequiresSessionAPIForExecutionProfileBindings(t *testing.T) {
	config := testConfig(t)
	config.ExecutionProfiles = []executionprofile.Binding{{
		ProjectID: "project-1",
		Profile:   intentmodel.ServerExecutionProfile{ID: "profile-1", SHA256: [32]byte{1}},
	}}
	if err := config.Validate(); err == nil || !strings.Contains(err.Error(), "session API requires") {
		t.Fatalf("profile policy without authenticated Hub API error=%v", err)
	}
}

func TestConfigRejectsUnsafeStateDirectories(t *testing.T) {
	base := t.TempDir()
	paths := []string{
		"", "relative/state", string(filepath.Separator),
		base + string(filepath.Separator),
		base + "//state",
		base + "/missing/../state",
		base + "/./state",
	}
	for _, path := range paths {
		config := testConfig(t)
		config.StateDir = path
		if err := config.Validate(); err == nil {
			t.Errorf("Validate state-dir %q succeeded", path)
		}
	}
}

func TestConfigBoundsStateDirectoryBeforeCleaning(t *testing.T) {
	config := testConfig(t)
	config.StateDir = string(filepath.Separator) + strings.Repeat("a", maxStateDirBytes-1)
	if err := config.Validate(); err != nil {
		t.Fatalf("state-dir byte boundary: %v", err)
	}
	config.StateDir += "a"
	if err := config.Validate(); err == nil {
		t.Fatal("oversized state-dir succeeded")
	}
	components := make([]string, maxStateDirComponents)
	for index := range components {
		components[index] = "a"
	}
	config.StateDir = string(filepath.Separator) + filepath.Join(components...)
	if err := config.Validate(); err != nil {
		t.Fatalf("state-dir component boundary: %v", err)
	}
	config.StateDir += string(filepath.Separator) + "a"
	if err := config.Validate(); err == nil {
		t.Fatal("state-dir with too many components succeeded")
	}
}

func TestConfigRejectsUnboundedBuildIdentity(t *testing.T) {
	config := testConfig(t)
	config.Build.Version = ""
	if err := config.Validate(); err == nil {
		t.Fatal("empty version succeeded")
	}
	config.Build.Version = "dev"
	config.Build.Commit = strings.Repeat("a", 129)
	if err := config.Validate(); err == nil {
		t.Fatal("oversized commit succeeded")
	}
	config.Build.Commit = "bad commit"
	if err := config.Validate(); err == nil {
		t.Fatal("commit with whitespace succeeded")
	}
}

func TestBrowserOriginsAreExactAndHTTPSExceptLoopbackDevelopment(t *testing.T) {
	for _, origin := range []string{
		"https://console.example",
		"https://console.example:8443",
		"http://localhost:3000",
		"http://127.0.0.1:3000",
		"http://[::1]:3000",
	} {
		if err := validateBrowserOrigins([]string{origin}); err != nil {
			t.Errorf("validateBrowserOrigins(%q): %v", origin, err)
		}
	}
	for _, origin := range []string{
		"*", "https://*", "https://console.example/", "https://user@console.example",
		"https://console.example:443", "https://console.example:70000", "http://console.example",
	} {
		if err := validateBrowserOrigins([]string{origin}); err == nil {
			t.Errorf("validateBrowserOrigins(%q) succeeded", origin)
		}
	}
	if err := validateBrowserOrigins([]string{"https://console.example", "https://console.example"}); err == nil {
		t.Fatal("duplicate browser origins succeeded")
	}
}

func TestConfigRequiresSessionAPIWhenBrowserOriginsAreConfigured(t *testing.T) {
	config := testConfig(t)
	config.BrowserOrigins = []string{"https://console.example"}
	if err := config.Validate(); err == nil {
		t.Fatal("browser origin without authenticated session API succeeded")
	}
}

func TestPrivateInterfaceRequiresTLSAndSnaplinkSessionAPI(t *testing.T) {
	config := testConfig(t)
	config.ListenAddress = "10.20.30.40:7467"
	if err := config.Validate(); err == nil {
		t.Fatal("private listener without TLS/API configuration succeeded")
	}

	root := t.TempDir()
	certificate := filepath.Join(root, "server.crt")
	privateKey := filepath.Join(root, "server.key")
	runtimeExecutable := filepath.Join(root, "forge-runtime")
	for path, contents := range map[string]string{
		certificate:       "test certificate file",
		privateKey:        "test private key file",
		runtimeExecutable: "test executable",
	} {
		if err := os.WriteFile(path, []byte(contents), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.Chmod(privateKey, 0o600); err != nil {
		t.Fatal(err)
	}
	config.TLSCertificateFile = certificate
	config.TLSPrivateKeyFile = privateKey
	config.RuntimeExecutable = runtimeExecutable
	config.RuntimeStateDir = filepath.Join(root, "runtime-state")
	config.SnaplinkIssuer = "https://identity.example"
	config.SnaplinkAudience = "forge-api"
	config.ExpectedTenantID = "tenant-slate"
	config.ExpectedSubjectID = "account-42"
	if err := config.Validate(); err != nil {
		t.Fatalf("valid private HTTPS/API config rejected: %v", err)
	}
	config.ExpectedSubjectID = ""
	if err := config.Validate(); err == nil {
		t.Fatal("session API without the exact Coordinator subject succeeded")
	}
	config.ExpectedSubjectID = "account-42"
	config.ExpectedTenantID = ""
	if err := config.Validate(); err == nil {
		t.Fatal("session API without the exact Coordinator tenant succeeded")
	}
	config.ExpectedTenantID = "tenant-slate"
	config.SnaplinkAudience = ""
	if err := config.Validate(); err == nil {
		t.Fatal("private listener without complete Snaplink config succeeded")
	}
}

func TestConfigAcceptsOptInIntrospectionWithPrivateCredentialFile(t *testing.T) {
	root := t.TempDir()
	executable := filepath.Join(root, "forge-runtime")
	if err := os.WriteFile(executable, []byte("runtime"), 0o700); err != nil {
		t.Fatal(err)
	}
	secretFile := filepath.Join(root, "snaplink-introspect.secret")
	if err := os.WriteFile(secretFile, []byte("resource-secret\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	config := testConfig(t)
	config.RuntimeExecutable = executable
	config.RuntimeStateDir = filepath.Join(root, "runtime-state")
	config.SnaplinkIssuer = "https://identity.example"
	config.SnaplinkAudience = "forge-api"
	config.ExpectedTenantID = "tenant-slate"
	config.ExpectedSubjectID = "account-42"
	config.SnaplinkIntrospectURL = "https://identity.example/token/introspect"
	config.SnaplinkIntrospectClientID = "forge-resource-server"
	config.SnaplinkIntrospectSecretFile = secretFile
	if err := config.Validate(); err != nil {
		t.Fatalf("valid opt-in introspection config rejected: %v", err)
	}
	config.SnaplinkIntrospectClientID = ""
	if err := config.Validate(); err == nil {
		t.Fatal("introspection endpoint without client credentials succeeded")
	}
}
