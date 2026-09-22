package appserver

// This opt-in test exercises the real RFC 8628 path end to end:
// a temporary Snaplink issuer serves device/code and token, the Rust CLI
// polls and stores its credential, and a second CLI process reads that saved
// credential for a Forge session request. It is intentionally gated because
// the final persistence leg needs an unlocked OS Secret Service/keyring.
// It does not register a Forge device, expose a device route, schedule work,
// dispatch a Runner, or publish audit output.

import (
	"bufio"
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/yangwb1123/snaplink/domains/authenticators"
	"github.com/yangwb1123/snaplink/infrastructure/defaultimpl"
	"github.com/yangwb1123/snaplink/interfaces/sso"
)

func TestSnaplinkRFC8628RustCLILoginWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_DEVICE_LOGIN_E2E") != "1" {
		t.Skip("set FORGE_DEVICE_LOGIN_E2E=1 for the opt-in Rust CLI RFC 8628/keyring integration")
	}
	executable := os.Getenv("FORGE_RUNTIME_BIN")
	if executable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for the opt-in Rust CLI RFC 8628/keyring integration")
	}
	if _, err := os.Stat(executable); err != nil {
		t.Fatalf("FORGE_RUNTIME_BIN=%q: %v", executable, err)
	}

	issuer, closeIssuer, credentialAccount := startSnaplinkDeviceLoginIssuer(t)
	t.Cleanup(closeIssuer)
	// The account is unique to this ephemeral issuer; clean up even when the
	// CLI fails before it can create the credential file.
	t.Cleanup(func() { clearForgeDeviceLoginKeyring(t, credentialAccount) })

	home, err := os.MkdirTemp(os.Getenv("HOME"), "forge-device-login-e2e-")
	if err != nil {
		t.Fatalf("create private test HOME: %v", err)
	}
	t.Cleanup(func() { _ = os.RemoveAll(home) })
	var stdout, stderr boundedCLIOutput
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	login := exec.CommandContext(ctx, executable, "remote", "login")
	login.Dir = home
	login.Env = forgeRuntimeDeviceLoginEnvironment(issuer, home)
	stdoutPipe, err := login.StdoutPipe()
	if err != nil {
		t.Fatal(err)
	}
	login.Stderr = &stderr
	if err := login.Start(); err != nil {
		t.Fatalf("start Rust remote login: %v", err)
	}
	line, err := bufio.NewReader(stdoutPipe).ReadString('\n')
	if err != nil {
		t.Fatalf("Rust remote login did not print the device verification prompt: %v", err)
	}
	stdout.Write([]byte(line))
	fields := strings.Fields(line)
	if len(fields) < 2 || fields[0] != "Open" {
		t.Fatalf("unexpected Rust remote login device prompt: %q", line)
	}
	userCode := fields[len(fields)-1]
	consoleToken := snaplinkForgeLogin(t, http.DefaultClient, issuer, "forge-console")
	verification := snaplinkConversationRequest(t, http.DefaultClient, issuer, consoleToken,
		http.MethodPost, "/device/verify", "", fmt.Sprintf(`{"user_code":%q,"approve":true}`, userCode))
	if verification.StatusCode != http.StatusOK {
		t.Fatalf("Snaplink device verification status=%d body=%q", verification.StatusCode, readConversationClientBody(t, verification))
	}
	if _, err := io.Copy(&stdout, stdoutPipe); err != nil {
		t.Fatalf("read Rust remote login output: %v", err)
	}
	if err := login.Wait(); err != nil {
		t.Fatalf("Rust remote login failed: stdout=%q stderr=%q err=%v", stdout.String(), stderr.String(), err)
	}
	if stdout.exceeded || stderr.exceeded {
		t.Fatal("Rust remote login output exceeded the size limit")
	}
	if !strings.Contains(stdout.String(), "Forge CLI login completed") {
		t.Fatalf("Rust remote login omitted completion message: %q", stdout.String())
	}
	if strings.Contains(stdout.String(), "refresh") || strings.Contains(stderr.String(), "refresh") {
		t.Fatalf("Rust remote login exposed refresh-token material: stdout=%q stderr=%q", stdout.String(), stderr.String())
	}

	credentialPath := findSavedForgeCredential(t, home)
	var saved struct {
		Issuer      string `json:"issuer"`
		ClientID    string `json:"client_id"`
		Subject     string `json:"subject"`
		TenantID    string `json:"tenant_id"`
		AccessToken string `json:"access_token"`
	}
	credentialBytes, err := os.ReadFile(credentialPath)
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(credentialBytes, &saved); err != nil {
		t.Fatalf("decode saved Forge credential: %v", err)
	}
	if saved.Issuer != issuer || saved.ClientID != "forge-cli" || saved.Subject != snaplinkForgeTestUser ||
		saved.TenantID != snaplinkForgeTestTenant || saved.AccessToken == "" {
		t.Fatalf("saved Forge credential binding=%#v, want issuer/client/owner from device token", saved)
	}
	if bytes.Contains(credentialBytes, []byte("refresh-secret")) {
		t.Fatal("saved Forge credential file contains refresh-token material")
	}

	var authorization string
	var changeCursors []string
	var forgeAPIMu sync.Mutex
	forgeAPI := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			http.Error(w, "unexpected Forge request", http.StatusNotFound)
			return
		}
		forgeAPIMu.Lock()
		defer forgeAPIMu.Unlock()
		authorization = r.Header.Get("Authorization")
		w.Header().Set("Content-Type", "application/json")
		switch r.URL.Path {
		case "/api/v1/conversations":
			_, _ = io.WriteString(w, `{"conversations":[],"has_more":false}`)
		case "/api/v1/conversation-changes":
			afterCursor := r.URL.Query().Get("after_cursor")
			changeCursors = append(changeCursors, afterCursor)
			switch afterCursor {
			case "0":
				_, _ = io.WriteString(w, `{"after_cursor":0,"scanned_through_cursor":1,"has_more":false,"changes":[{"cursor":1,"schema_version":1,"conversation_id":"credential-e2e-conversation","entity_id":"credential-e2e-conversation","aggregate_version":1,"kind":"conversation_created","created_at_ms":1}]}`)
			case "1":
				_, _ = io.WriteString(w, `{"after_cursor":1,"scanned_through_cursor":1,"has_more":false,"changes":[]}`)
			default:
				http.Error(w, "unexpected change cursor", http.StatusBadRequest)
			}
		default:
			http.Error(w, "unexpected Forge request", http.StatusNotFound)
		}
	}))
	t.Cleanup(forgeAPI.Close)

	list := exec.Command(executable, "--json", "remote", "sessions", "list")
	list.Dir = home
	list.Env = forgeRuntimeDeviceSessionEnvironment(issuer, forgeAPI.URL, home)
	var listStdout, listStderr boundedCLIOutput
	list.Stdout = &listStdout
	list.Stderr = &listStderr
	if err := list.Run(); err != nil {
		t.Fatalf("Rust saved-credential session list failed: stdout=%q stderr=%q err=%v", listStdout.String(), listStderr.String(), err)
	}
	forgeAPIMu.Lock()
	savedAuthorization := authorization
	forgeAPIMu.Unlock()
	if savedAuthorization != "Bearer "+saved.AccessToken {
		t.Fatalf("saved credential Authorization=%q, want Bearer token from login", savedAuthorization)
	}

	firstChanges := exec.Command(executable, "--json", "remote", "changes", "list")
	firstChanges.Dir = home
	firstChanges.Env = forgeRuntimeDeviceSessionEnvironment(issuer, forgeAPI.URL, home)
	var firstChangesStdout, firstChangesStderr boundedCLIOutput
	firstChanges.Stdout = &firstChangesStdout
	firstChanges.Stderr = &firstChangesStderr
	if err := firstChanges.Run(); err != nil {
		t.Fatalf("Rust saved-credential first change-feed read failed: stdout=%q stderr=%q err=%v", firstChangesStdout.String(), firstChangesStderr.String(), err)
	}
	var firstChangesPage struct {
		AfterCursor          uint64 `json:"after_cursor"`
		ScannedThroughCursor uint64 `json:"scanned_through_cursor"`
		HasMore              bool   `json:"has_more"`
		Changes              []struct {
			Cursor uint64 `json:"cursor"`
			Kind   string `json:"kind"`
		} `json:"changes"`
	}
	if err := json.Unmarshal([]byte(firstChangesStdout.String()), &firstChangesPage); err != nil ||
		firstChangesPage.AfterCursor != 0 || firstChangesPage.ScannedThroughCursor != 1 ||
		firstChangesPage.HasMore || len(firstChangesPage.Changes) != 1 ||
		firstChangesPage.Changes[0].Cursor != 1 || firstChangesPage.Changes[0].Kind != "conversation_created" {
		t.Fatalf("first saved-credential change-feed page=%#v stdout=%q decode=%v", firstChangesPage, firstChangesStdout.String(), err)
	}

	secondChanges := exec.Command(executable, "--json", "remote", "changes", "list")
	secondChanges.Dir = home
	secondChanges.Env = forgeRuntimeDeviceSessionEnvironment(issuer, forgeAPI.URL, home)
	var secondChangesStdout, secondChangesStderr boundedCLIOutput
	secondChanges.Stdout = &secondChangesStdout
	secondChanges.Stderr = &secondChangesStderr
	if err := secondChanges.Run(); err != nil {
		t.Fatalf("Rust second saved-credential change-feed read failed: stdout=%q stderr=%q err=%v", secondChangesStdout.String(), secondChangesStderr.String(), err)
	}
	var secondChangesPage struct {
		AfterCursor          uint64 `json:"after_cursor"`
		ScannedThroughCursor uint64 `json:"scanned_through_cursor"`
		HasMore              bool   `json:"has_more"`
		Changes              []any  `json:"changes"`
	}
	if err := json.Unmarshal([]byte(secondChangesStdout.String()), &secondChangesPage); err != nil ||
		secondChangesPage.AfterCursor != 1 || secondChangesPage.ScannedThroughCursor != 1 ||
		secondChangesPage.HasMore || len(secondChangesPage.Changes) != 0 {
		t.Fatalf("second saved-credential change-feed page=%#v stdout=%q decode=%v", secondChangesPage, secondChangesStdout.String(), err)
	}
	forgeAPIMu.Lock()
	gotCursors := append([]string(nil), changeCursors...)
	forgeAPIMu.Unlock()
	if len(gotCursors) != 2 || gotCursors[0] != "0" || gotCursors[1] != "1" {
		t.Fatalf("saved CLI cursor was not resumed across processes: requests=%#v", gotCursors)
	}
}

func startSnaplinkDeviceLoginIssuer(t *testing.T) (issuer string, closeIssuer func(), credentialAccount string) {
	t.Helper()
	ctx := context.Background()
	listener := httptest.NewUnstartedServer(nil)
	issuer = "http://" + listener.Listener.Addr().String()
	users := defaultimpl.NewMemoryUserProvider()
	if err := users.CreateOrUpdate(ctx, &sso.User{ID: snaplinkForgeTestUser, Name: "Forge Snaplink Device User"}); err != nil {
		t.Fatal(err)
	}
	clients := defaultimpl.NewMemoryClientStore()
	clients.AddSeed(&sso.Client{
		ID: "forge-console", Name: "forge-console", TenantID: snaplinkForgeTestTenant,
		SubjectType: "public", AllowedScopes: []string{"openid", "profile", "forge:conversations:read", "forge:conversations:write", deviceInventoryReadCandidateScope},
		AllowedResources: []string{snaplinkForgeTestAudience}, Active: true,
		TokenStrategy: "jwt", TokenEndpointAuthMethod: "none", SkipConsent: true,
	})
	clients.AddSeed(&sso.Client{
		ID: "forge-cli", Name: "forge-cli", TenantID: snaplinkForgeTestTenant,
		SubjectType: "public", AllowedScopes: []string{"forge:conversations:read", "forge:conversations:write"},
		AllowedResources: []string{snaplinkForgeTestAudience}, Active: true,
		TokenStrategy: "jwt", TokenEndpointAuthMethod: "none", SkipConsent: true,
		GrantTypes: []string{sso.GrantDeviceCode, sso.GrantRefreshToken},
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
		defaultimpl.WithEd25519Issuer(issuer), defaultimpl.WithEd25519TokenTTL(5*time.Minute),
	)
	snaplink := sso.NewServer(
		sso.WithIssuer(issuer), sso.WithUserProvider(users),
		sso.WithSessionManager(defaultimpl.NewMemorySessionManager()), sso.WithClientStore(clients),
		sso.WithAuthenticator(password), sso.WithTokenIssuer("jwt", issuerImpl),
		sso.WithDefaultTokenStrategy("jwt"),
		sso.WithRefreshTokenStore(defaultimpl.NewMemoryRefreshTokenStore(), time.Hour),
		sso.WithDeviceCodeStore(defaultimpl.NewMemoryDeviceCodeStore(), 2*time.Minute, time.Second, issuer+"/device/verify"),
	)
	listener.Config.Handler = snaplink.Handler()
	listener.Start()
	closeIssuer = listener.Close
	credentialAccount = forgeCredentialAccount(issuer, "forge-cli", snaplinkForgeTestTenant, snaplinkForgeTestUser)
	return issuer, closeIssuer, credentialAccount
}

func forgeRuntimeDeviceLoginEnvironment(issuer, home string) []string {
	return forgeRuntimeDeviceSessionEnvironment(issuer, "", home)
}

func forgeRuntimeDeviceSessionEnvironment(issuer, apiURL, home string) []string {
	env := make([]string, 0, 12)
	for _, value := range os.Environ() {
		key, _, _ := strings.Cut(value, "=")
		switch key {
		case "PATH", "SYSTEMROOT", "WINDIR", "TMP", "TEMP", "TMPDIR", "DBUS_SESSION_BUS_ADDRESS":
			env = append(env, value)
		}
	}
	if apiURL != "" {
		env = append(env, "FORGE_API_URL="+apiURL)
	}
	env = append(env,
		"HOME="+home, "USERPROFILE="+home,
		"XDG_CONFIG_HOME="+filepath.Join(home, "config"),
		"XDG_STATE_HOME="+filepath.Join(home, "state"),
	)
	if issuer != "" {
		env = append(env, "SNAPLINK_ISSUER_URL="+issuer, "SNAPLINK_CLIENT_ID=forge-cli")
	}
	return env
}

func findSavedForgeCredential(t *testing.T, home string) string {
	t.Helper()
	entries, err := os.ReadDir(filepath.Join(home, "config", "forge-runtime", "credentials"))
	if err != nil {
		t.Fatalf("read saved Forge credentials: %v", err)
	}
	var path string
	for _, entry := range entries {
		if entry.IsDir() || filepath.Ext(entry.Name()) != ".json" {
			continue
		}
		if path != "" {
			t.Fatalf("login saved more than one Forge credential: %q and %q", path, entry.Name())
		}
		path = filepath.Join(home, "config", "forge-runtime", "credentials", entry.Name())
	}
	if path == "" {
		t.Fatal("remote login did not save a Forge credential")
	}
	return path
}

func forgeCredentialAccount(issuer, client, tenant, subject string) string {
	digest := sha256.New()
	for _, field := range []string{issuer, client, tenant, subject} {
		var length [8]byte
		for index := uint(0); index < 8; index++ {
			length[7-index] = byte(uint64(len(field)) >> (index * 8))
		}
		_, _ = digest.Write(length[:])
		_, _ = digest.Write([]byte(field))
	}
	return hex.EncodeToString(digest.Sum(nil))
}

func clearForgeDeviceLoginKeyring(t *testing.T, account string) {
	t.Helper()
	secretTool, err := exec.LookPath("secret-tool")
	if err != nil {
		t.Logf("secret-tool unavailable; leaving opt-in test keyring entry for account %s: %v", account, err)
		return
	}
	command := exec.Command(secretTool, "clear", "service", "forge-runtime-cli", "username", "refresh-"+account)
	if output, err := command.CombinedOutput(); err != nil {
		t.Logf("could not clear opt-in test keyring entry for account %s: %v (%s)", account, err, strings.TrimSpace(string(output)))
	}
}
