package appserver

// This opt-in test crosses the real Snaplink JWT boundary for the read-only
// execution-consent preview candidate. It deliberately stops at the server
// resolved project/profile metadata: no consent grant, Run, device, lease, or
// Runner operation is involved.

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/executionprofile"
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestSnaplinkAuthenticatedExecutionConsentPreviewE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_EXECUTION_CONSENT_PREVIEW_E2E") != "1" {
		t.Skip("set FORGE_EXECUTION_CONSENT_PREVIEW_E2E=1 for the execution-consent preview E2E")
	}

	const conversationID = "conversation-consent-preview"
	const projectID = "project-consent-preview"
	const profileID = "profile-consent-preview-v1"
	profileDigest := sha256.Sum256([]byte("snaplink-execution-consent-preview-profile-v1"))
	profileDigestText := hex.EncodeToString(profileDigest[:])
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)
	authenticator, err := authn.New(authn.Config{
		Issuer: issuer, Audience: snaplinkForgeTestAudience,
		JWKSURL:          issuer + "/.well-known/jwks.json",
		ExpectedTenantID: snaplinkForgeTestTenant, ExpectedSubjectID: snaplinkForgeTestUser,
		JWKSHTTPClient: ssoClient, JWKSRefreshInterval: 24 * time.Hour,
	})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(authenticator.Close)
	profiles, err := executionprofile.New([]executionprofile.Binding{{
		ProjectID: projectID,
		Profile:   intentmodel.ServerExecutionProfile{ID: profileID, SHA256: profileDigest},
	}})
	if err != nil {
		t.Fatal(err)
	}
	backend := &fakeConversationBackend{
		listPage: model.OwnedConversationPage{
			Conversations: []model.OwnedConversationEntry{{
				Conversation: model.Conversation{
					ID: conversationID, Scope: model.ConversationScope{Kind: "project", ID: projectID},
					Title: "Execution consent preview fixture", CreatedAtMS: 1, UpdatedAtMS: 1,
				},
				AggregateVersion: 1,
			}},
			HasMore: false,
		},
		projectIdentity: model.OwnedProjectConversationIdentity{
			ConversationID: conversationID, ProjectID: projectID,
		},
	}
	candidate := authenticator.Handler(newConversationRoutesWithInertExecutionAPI(backend, profiles))
	server := httptest.NewServer(candidate)
	t.Cleanup(server.Close)
	path := conversationCollectionPath + "/" + conversationID + "/execution-consents"
	response := snaplinkConversationRequest(t, server.Client(), server.URL, token, http.MethodGet, path, "", "")
	if response.StatusCode != http.StatusOK {
		t.Fatalf("Snaplink execution-consent preview status=%d body=%q", response.StatusCode, readConversationClientBody(t, response))
	}
	body := readConversationClientBody(t, response)
	var preview executionConsentPreviewResponse
	if err := json.Unmarshal([]byte(body), &preview); err != nil {
		t.Fatalf("decode Snaplink execution-consent preview: %v body=%q", err, body)
	}
	if preview.ConversationID != conversationID || preview.ProjectID != projectID ||
		preview.ProfileID != profileID || preview.ProfileSHA256 != profileDigestText ||
		preview.MaximumTTLMS != maxExecutionConsentTTLMS {
		t.Fatalf("Snaplink execution-consent preview=%#v", preview)
	}
	var fields map[string]json.RawMessage
	if err := json.Unmarshal([]byte(body), &fields); err != nil || len(fields) != 5 {
		t.Fatalf("execution-consent preview fields=%d err=%v body=%q", len(fields), err, body)
	}

	inputPath := filepath.Join(t.TempDir(), "execution-consent-preview.json")
	if err := os.WriteFile(inputPath, []byte(body), 0o600); err != nil {
		t.Fatal(err)
	}
	if executable := os.Getenv("FORGE_RUNTIME_BIN"); executable != "" {
		runForgeRuntimeExecutionConsentPreviewCLI(t, executable, server.URL, token, conversationID, preview)
		runForgeRuntimeExecutionConsentPreviewTUI(t, executable, server.URL, token, conversationID, preview)
	}
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		runForgeConsoleExecutionConsentPreviewE2EWithToken(t, server.URL, token, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant, conversationID, projectID, profileID, profileDigestText)
	}

	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, profiles))
	request, err := http.NewRequest(http.MethodGet, server.URL+path, nil)
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Authorization", "Bearer "+token)
	productionResponse := httptest.NewRecorder()
	production.ServeHTTP(productionResponse, request)
	if productionResponse.Code != http.StatusNotFound || productionResponse.Body.String() != string(notFoundBody) {
		t.Fatalf("production execution-consent preview status=%d body=%q", productionResponse.Code, productionResponse.Body.String())
	}
}

func runForgeRuntimeExecutionConsentPreviewCLI(t *testing.T, executable, apiURL, accessToken, conversationID string, expected executionConsentPreviewResponse) {
	t.Helper()
	output, stderr, err := runForgeRuntimeCLI(t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "remote", "execution-consent", "preview", conversationID)
	if err != nil {
		t.Fatalf("authenticated execution-consent preview Rust CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var preview executionConsentPreviewResponse
	if err := json.Unmarshal([]byte(output), &preview); err != nil {
		t.Fatalf("decode authenticated execution-consent preview CLI: %v stdout=%q", err, output)
	}
	if preview != expected {
		t.Fatalf("authenticated execution-consent preview CLI=%#v want=%#v", preview, expected)
	}
}

func runForgeRuntimeExecutionConsentPreviewTUI(t *testing.T, executable, apiURL, accessToken, conversationID string, expected executionConsentPreviewResponse) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("execution-consent preview TUI E2E requires script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("execution-consent-preview\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated execution-consent preview Rust TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("execution-consent preview TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"execution-consent preview [read-only candidate]",
		"conversation=" + expected.ConversationID,
		"project=" + expected.ProjectID,
		"profile=" + expected.ProfileID,
		"profile_sha256=" + expected.ProfileSHA256,
		"maximum_ttl_ms=",
		"No consent was granted",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("authenticated execution-consent preview TUI omitted %q: %q", want, output)
		}
	}
}

func runForgeConsoleExecutionConsentPreviewE2EWithToken(t *testing.T, apiURL, token, issuer, subject, tenant, conversationID, projectID, profileID, profileDigest string) {
	t.Helper()
	consoleRoot := os.Getenv("SNAPLINK_CONSOLE_ROOT")
	if consoleRoot == "" {
		workingDirectory, err := os.Getwd()
		if err != nil {
			t.Fatalf("resolve Console repository: %v", err)
		}
		repoRoot := filepath.Clean(filepath.Join(workingDirectory, "..", "..", ".."))
		consoleRoot = filepath.Join(filepath.Dir(repoRoot), "workspace", "demo", "snaplink-console")
	}
	if _, err := os.Stat(filepath.Join(consoleRoot, "pubspec.yaml")); err != nil {
		t.Fatalf("SNAPLINK_CONSOLE_ROOT must name the Flutter Console repository: %v", err)
	}
	flutterBinary := os.Getenv("FLUTTER_BIN")
	if flutterBinary == "" {
		flutterBinary = "flutter"
	}
	flutterExecutable, err := exec.LookPath(flutterBinary)
	if err != nil {
		t.Fatalf("Flutter is required for execution-consent preview E2E: %v", err)
	}
	input := struct {
		APIURL                string `json:"api_url"`
		AccessToken           string `json:"access_token"`
		Issuer                string `json:"issuer"`
		Subject               string `json:"subject"`
		TenantID              string `json:"tenant_id"`
		ConversationID        string `json:"conversation_id"`
		ExpectedProjectID     string `json:"expected_project_id"`
		ExpectedProfileID     string `json:"expected_profile_id"`
		ExpectedProfileSHA256 string `json:"expected_profile_sha256"`
		ExpectedMaximumTTLMS  uint64 `json:"expected_maximum_ttl_ms"`
	}{
		APIURL: apiURL, AccessToken: token, Issuer: issuer, Subject: subject, TenantID: tenant,
		ConversationID: conversationID, ExpectedProjectID: projectID, ExpectedProfileID: profileID,
		ExpectedProfileSHA256: profileDigest, ExpectedMaximumTTLMS: maxExecutionConsentTTLMS,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatal(err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "execution-consent-preview-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatal(err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub", "test/forge_execution_consent_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_EXECUTION_CONSENT_E2E_INPUT="+inputPath)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter execution-consent preview E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("execution-consent preview Flutter output exceeded the size limit")
	}
}
