//go:build linux && !android

package main

import (
	"bufio"
	"bytes"
	"crypto/ed25519"
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	"encoding/pem"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/appserver"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

const (
	processSessionTenant   = "tenant-slate"
	processSessionSubject  = "account-42"
	processSessionAudience = "forge-api"
)

// TestForgeServerProcessAuthenticatedSharedSessionBoundary starts the real
// forge-server binary with the real Rust Runtime bridge. It proves the
// authenticated multi-client session boundary while keeping device authority
// disabled: a prompt is persisted, but no Run is started or dispatched.
func TestForgeServerProcessAuthenticatedSharedSessionBoundary(t *testing.T) {
	runtimeExecutable := os.Getenv("FORGE_RUNTIME_BIN")
	if runtimeExecutable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for forge-server process shared-session integration")
	}
	runtimeExecutable, err := filepath.Abs(runtimeExecutable)
	if err != nil {
		t.Fatal(err)
	}
	if resolved, resolveErr := filepath.EvalSymlinks(runtimeExecutable); resolveErr == nil {
		runtimeExecutable = resolved
	}
	if info, statErr := os.Stat(runtimeExecutable); statErr != nil || !info.Mode().IsRegular() {
		t.Skipf("FORGE_RUNTIME_BIN is not a regular executable: %s", runtimeExecutable)
	}

	issuer := newProcessSessionIssuer(t)
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	if err := os.Mkdir(runtimeState, 0o700); err != nil {
		t.Fatal(err)
	}
	initializeProcessRuntime(t, runtimeExecutable, runtimeState)

	serverBinary := buildServerCommand(t)
	serverParent := t.TempDir()
	if err := os.Chmod(serverParent, 0o700); err != nil {
		t.Fatal(err)
	}
	serverState := filepath.Join(serverParent, "server-state")
	command, stdout, stderr := startAuthenticatedServerCommand(t, serverBinary, runtimeExecutable, runtimeState, serverState, issuer.issuer, issuer.caFile)
	t.Cleanup(func() { stopUnwaited(command) })
	ready := readProcessReady(t, stdout, stderr.String)
	client := &http.Client{Transport: &http.Transport{Proxy: nil}, Timeout: 5 * time.Second}
	assertProcessHealth(t, ready.Listen)

	// The actual route authority is the listener address. A different Host is
	// rejected before authentication and before any Runtime call.
	wrongHost := doProcessSessionRequest(t, client, ready.Listen, "", http.MethodGet, appserver.HealthPath, "", "", "forge.invalid")
	if wrongHost.StatusCode != http.StatusMisdirectedRequest {
		fatalProcessResponse(t, wrongHost, "wrong Host", http.StatusMisdirectedRequest)
	}
	closeProcessResponse(t, wrongHost)

	tokenA := issuer.token(processSessionSubject, processSessionTenant, "forge:conversations:read forge:conversations:write", "client-a")
	tokenB := issuer.token(processSessionSubject, processSessionTenant, "forge:conversations:read forge:conversations:write", "client-b")
	createdResponse := processSessionRequest(t, client, ready.Listen, tokenA, http.MethodPost,
		"/api/v1/conversations", `{"scope":{"kind":"global"},"title":"process shared session"}`, "process-create")
	if createdResponse.StatusCode != http.StatusCreated {
		fatalProcessResponse(t, createdResponse, "create Conversation", http.StatusCreated)
	}
	var created model.Conversation
	if err := json.NewDecoder(createdResponse.Body).Decode(&created); err != nil || created.ID == "" {
		closeProcessResponse(t, createdResponse)
		t.Fatalf("created Conversation=%#v decode=%v", created, err)
	}
	closeProcessResponse(t, createdResponse)

	listResponse := processSessionRequest(t, client, ready.Listen, tokenB, http.MethodGet,
		"/api/v1/conversations?limit=50", "", "")
	if listResponse.StatusCode != http.StatusOK {
		fatalProcessResponse(t, listResponse, "client B list", http.StatusOK)
	}
	var page model.OwnedConversationPage
	if err := json.NewDecoder(listResponse.Body).Decode(&page); err != nil || len(page.Conversations) != 1 ||
		page.Conversations[0].Conversation.ID != created.ID {
		closeProcessResponse(t, listResponse)
		t.Fatalf("client B list=%#v decode=%v", page, err)
	}
	closeProcessResponse(t, listResponse)

	promptPath := "/api/v1/conversations/" + created.ID + "/prompts"
	appendResponse := processSessionRequest(t, client, ready.Listen, tokenB, http.MethodPost, promptPath,
		`{"content":"prompt from process client B","expected_version":1}`, "process-prompt")
	if appendResponse.StatusCode != http.StatusCreated {
		fatalProcessResponse(t, appendResponse, "client B prompt", http.StatusCreated)
	}
	var appendResult struct {
		Prompt           model.ConversationPrompt `json:"prompt"`
		AggregateVersion uint64                   `json:"aggregate_version"`
		Replayed         bool                     `json:"replayed"`
	}
	if err := json.NewDecoder(appendResponse.Body).Decode(&appendResult); err != nil ||
		appendResult.Prompt.Content != "prompt from process client B" || appendResult.AggregateVersion != 2 || appendResult.Replayed {
		closeProcessResponse(t, appendResponse)
		t.Fatalf("prompt append=%#v decode=%v", appendResult, err)
	}
	closeProcessResponse(t, appendResponse)

	historyResponse := processSessionRequest(t, client, ready.Listen, tokenA, http.MethodGet, promptPath+"?limit=50", "", "")
	if historyResponse.StatusCode != http.StatusOK {
		fatalProcessResponse(t, historyResponse, "client A prompt history", http.StatusOK)
	}
	var history model.ConversationPromptPage
	if err := json.NewDecoder(historyResponse.Body).Decode(&history); err != nil || len(history.Prompts) != 1 ||
		history.Prompts[0].Content != "prompt from process client B" {
		closeProcessResponse(t, historyResponse)
		t.Fatalf("client A history=%#v decode=%v", history, err)
	}
	closeProcessResponse(t, historyResponse)

	// Idempotent replay is accepted, while a stale aggregate version is a
	// conflict. Both checks exercise the process boundary and durable state.
	replay := processSessionRequest(t, client, ready.Listen, tokenA, http.MethodPost, promptPath,
		`{"content":"prompt from process client B","expected_version":1}`, "process-prompt")
	if replay.StatusCode != http.StatusOK {
		fatalProcessResponse(t, replay, "prompt replay", http.StatusOK)
	}
	closeProcessResponse(t, replay)
	stale := processSessionRequest(t, client, ready.Listen, tokenA, http.MethodPost, promptPath,
		`{"content":"stale prompt","expected_version":1}`, "process-stale")
	if stale.StatusCode != http.StatusConflict {
		fatalProcessResponse(t, stale, "stale prompt", http.StatusConflict)
	}
	closeProcessResponse(t, stale)

	readOnlyToken := issuer.token(processSessionSubject, processSessionTenant, "forge:conversations:read", "read-only")
	forbiddenCreate := processSessionRequest(t, client, ready.Listen, readOnlyToken, http.MethodPost,
		"/api/v1/conversations", `{"scope":{"kind":"global"},"title":"forbidden"}`, "")
	if forbiddenCreate.StatusCode != http.StatusForbidden {
		fatalProcessResponse(t, forbiddenCreate, "missing write scope", http.StatusForbidden)
	}
	closeProcessResponse(t, forbiddenCreate)
	wrongOwner := issuer.token("other-account", processSessionTenant, "forge:conversations:read", "wrong-owner")
	wrongOwnerResponse := processSessionRequest(t, client, ready.Listen, wrongOwner, http.MethodGet,
		"/api/v1/conversations?limit=50", "", "")
	if wrongOwnerResponse.StatusCode != http.StatusForbidden {
		fatalProcessResponse(t, wrongOwnerResponse, "wrong owner", http.StatusForbidden)
	}
	closeProcessResponse(t, wrongOwnerResponse)

	// Device registration/heartbeat and all observation/execution candidates
	// remain absent from the production router. Their 404s are part of this
	// boundary; candidate constructors are exercised by focused inert tests.
	for _, path := range []string{
		"/api/v1/device-placement/preview",
		"/api/v1/devices",
		"/api/v1/devices/enrollments",
		"/api/v1/devices/device-1/heartbeats",
		"/api/v1/conversations/" + created.ID + "/execution-consents",
		"/api/v1/conversations/" + created.ID + "/run-intents",
	} {
		response := processSessionRequest(t, client, ready.Listen, tokenA, http.MethodGet, path, "", "")
		if response.StatusCode != http.StatusNotFound {
			fatalProcessResponse(t, response, "disabled route "+path, http.StatusNotFound)
		}
		closeProcessResponse(t, response)
	}
}

type processSessionIssuer struct {
	server *httptest.Server
	key    ed25519.PrivateKey
	keyID  string
	issuer string
	caFile string
}

func newProcessSessionIssuer(t *testing.T) *processSessionIssuer {
	t.Helper()
	publicKey, privateKey, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	keyID := "forge-server-process-test-key"
	keys, err := json.Marshal(map[string]any{"keys": []any{map[string]string{
		"kty": "OKP", "crv": "Ed25519", "kid": keyID, "use": "sig", "alg": "EdDSA",
		"x": base64.RawURLEncoding.EncodeToString(publicKey),
	}}})
	if err != nil {
		t.Fatal(err)
	}
	server := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/jwks" {
			http.NotFound(w, r)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write(keys)
	}))
	caFile := filepath.Join(t.TempDir(), "issuer-ca.pem")
	if err := os.WriteFile(caFile, pem.EncodeToMemory(&pem.Block{
		Type: "CERTIFICATE", Bytes: server.TLS.Certificates[0].Certificate[0],
	}), 0o600); err != nil {
		server.Close()
		t.Fatal(err)
	}
	t.Cleanup(server.Close)
	return &processSessionIssuer{server: server, key: privateKey, keyID: keyID, issuer: server.URL, caFile: caFile}
}

func (issuer *processSessionIssuer) token(subject, tenant, scopes, clientID string) string {
	header, _ := json.Marshal(map[string]string{"typ": "at+jwt", "alg": "EdDSA", "kid": issuer.keyID})
	claims, _ := json.Marshal(map[string]any{
		"iss": issuer.issuer, "sub": subject, "aud": processSessionAudience,
		"exp": time.Now().Add(5 * time.Minute).Unix(), "iat": time.Now().Add(-time.Minute).Unix(),
		"tenant_id": tenant, "scope": scopes, "jti": clientID,
	})
	input := base64.RawURLEncoding.EncodeToString(header) + "." + base64.RawURLEncoding.EncodeToString(claims)
	signature := ed25519.Sign(issuer.key, []byte(input))
	return input + "." + base64.RawURLEncoding.EncodeToString(signature)
}

func initializeProcessRuntime(t *testing.T, executable, stateDir string) {
	t.Helper()
	command := exec.Command(executable, "--state-dir", stateDir, "session", "list")
	command.Env = []string{}
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("initialize Runtime state: %v output=%q", err, output)
	}
}

func startAuthenticatedServerCommand(t *testing.T, binary, runtimeExecutable, runtimeState, serverState, issuerURL, caFile string) (*exec.Cmd, *bufio.Reader, *bytes.Buffer) {
	t.Helper()
	args := []string{
		"--state-dir", serverState, "--listen", "127.0.0.1:0",
		"--runtime-executable", runtimeExecutable, "--runtime-state-dir", runtimeState,
		"--snaplink-issuer", issuerURL, "--snaplink-audience", processSessionAudience,
		"--snaplink-jwks-url", issuerURL + "/jwks", "--snaplink-tenant", processSessionTenant,
		"--snaplink-subject", processSessionSubject,
	}
	command := exec.Command(binary, args...)
	env := make([]string, 0, len(os.Environ())+1)
	for _, value := range os.Environ() {
		if !strings.HasPrefix(value, "SSL_CERT_FILE=") {
			env = append(env, value)
		}
	}
	command.Env = append(env, "SSL_CERT_FILE="+caFile)
	stdout, err := command.StdoutPipe()
	if err != nil {
		t.Fatal(err)
	}
	stderr := &bytes.Buffer{}
	command.Stderr = stderr
	if err := command.Start(); err != nil {
		t.Fatal(err)
	}
	return command, bufio.NewReader(stdout), stderr
}

func processSessionRequest(t *testing.T, client *http.Client, baseURL, token, method, path, body, idempotencyKey string) *http.Response {
	return doProcessSessionRequest(t, client, baseURL, token, method, path, body, idempotencyKey, "")
}

func doProcessSessionRequest(t *testing.T, client *http.Client, baseURL, token, method, path, body, idempotencyKey, host string) *http.Response {
	t.Helper()
	request, err := http.NewRequest(method, baseURL+path, strings.NewReader(body))
	if err != nil {
		t.Fatal(err)
	}
	if token != "" {
		request.Header.Set("Authorization", "Bearer "+token)
	}
	if body != "" {
		request.Header.Set("Content-Type", "application/json")
	}
	if idempotencyKey != "" {
		request.Header.Set("Idempotency-Key", idempotencyKey)
	}
	if host != "" {
		request.Host = host
	}
	response, err := client.Do(request)
	if err != nil {
		t.Fatalf("%s %s: %v", method, path, err)
	}
	return response
}

func fatalProcessResponse(t *testing.T, response *http.Response, operation string, want int) {
	t.Helper()
	body, _ := io.ReadAll(response.Body)
	t.Fatalf("%s status=%d want=%d body=%q", operation, response.StatusCode, want, body)
}

func closeProcessResponse(t *testing.T, response *http.Response) {
	t.Helper()
	if err := response.Body.Close(); err != nil {
		t.Fatal(err)
	}
}
