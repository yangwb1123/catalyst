package appserver

// This opt-in test crosses the real Snaplink JWT boundary and Chromium for
// both owner-bound client-instance observations. The candidate routes are
// mounted only on this test mux; production route wiring remains closed.

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
)

func TestSnaplinkAuthenticatedClientInstanceViewsBrowserE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_BROWSER_E2E") != "1" || os.Getenv("FORGE_CLIENT_INSTANCE_BROWSER_E2E") != "1" {
		t.Skip("set FORGE_BROWSER_E2E=1 and FORGE_CLIENT_INSTANCE_BROWSER_E2E=1 for client-instance Chromium E2E")
	}

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

	owner := deviceplacement.Owner{
		Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant,
	}
	sessionView := fixtureClientInstanceSessionView(owner)
	resourceView := fixtureClientInstanceResourceView(owner)
	testRoutes := http.NewServeMux()
	testRoutes.Handle(clientInstanceSessionViewCandidatePath,
		newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
			Enabled: true,
			Source:  &fixtureClientInstanceSessionViewSource{value: sessionView},
		}))
	testRoutes.Handle(clientInstanceResourceViewCandidatePath,
		newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
			Enabled: true,
			Source:  &fixtureClientInstanceResourceViewSource{value: resourceView},
		}))
	testRoutes.Handle("/", http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		if request.URL.Path == "/api/v1/conversations" && request.Method == http.MethodGet {
			writer.Header().Set("Content-Type", "application/json")
			_, _ = writer.Write([]byte(`{"conversations":[],"next_after_id":null,"has_more":false}`))
			return
		}
		if request.URL.Path == "/api/v1/conversation-changes" && request.Method == http.MethodGet {
			writer.Header().Set("Content-Type", "application/json")
			_, _ = writer.Write([]byte(`{"after_cursor":0,"scanned_through_cursor":0,"has_more":false,"changes":[]}`))
			return
		}
		newConversationRoutes(nil).ServeHTTP(writer, request)
	}))
	serverHandler := http.Handler(authenticator.Handler(testRoutes))
	webBuildDir := os.Getenv("FORGE_WEB_BUILD_DIR")
	if webBuildDir == "" {
		t.Fatal("FORGE_WEB_BUILD_DIR is required when FORGE_BROWSER_E2E=1")
	}
	serverHandler = serveForgeConsoleWebAssets(webBuildDir, serverHandler)
	server := httptest.NewServer(serverHandler)
	t.Cleanup(server.Close)

	for path := range map[string]struct{}{
		clientInstanceSessionViewCandidatePath:  {},
		clientInstanceResourceViewCandidatePath: {},
	} {
		response := snaplinkConversationRequest(t, server.Client(), server.URL, token,
			http.MethodGet, path, "", "")
		if response.StatusCode != http.StatusOK {
			t.Fatalf("candidate %s status=%d body=%q", path, response.StatusCode, readConversationClientBody(t, response))
		}
		_ = response.Body.Close()
	}

	runForgeConsoleBrowserClientInstanceViewsE2EWithToken(
		t, server.URL, token, owner, sessionView, resourceView,
	)

	production := authenticator.Handler(newConversationRoutes(nil))
	for path := range map[string]struct{}{
		clientInstanceSessionViewCandidatePath:  {},
		clientInstanceResourceViewCandidatePath: {},
	} {
		request, err := http.NewRequest(http.MethodGet, server.URL+path, nil)
		if err != nil {
			t.Fatal(err)
		}
		request.Header.Set("Authorization", "Bearer "+token)
		productionResponse := httptest.NewRecorder()
		production.ServeHTTP(productionResponse, request)
		if productionResponse.Code != http.StatusNotFound {
			t.Fatalf("production %s route status=%d body=%q", path, productionResponse.Code, productionResponse.Body.String())
		}
	}
}

func runForgeConsoleBrowserClientInstanceViewsE2EWithToken(
	t *testing.T,
	apiURL, token string,
	owner deviceplacement.Owner,
	sessionView deviceplacement.ClientInstanceSessionViewObservation,
	resourceView deviceplacement.ClientInstanceResourceViewObservation,
) {
	t.Helper()
	pythonBinary := os.Getenv("FORGE_BROWSER_PYTHON")
	if pythonBinary == "" {
		pythonBinary = "python3"
	}
	pythonExecutable, err := exec.LookPath(pythonBinary)
	if err != nil {
		t.Fatalf("Python is required for client-instance Chromium E2E: %v", err)
	}
	workingDirectory, err := os.Getwd()
	if err != nil {
		t.Fatalf("resolve Forge Core repository: %v", err)
	}
	repoRoot := filepath.Clean(filepath.Join(workingDirectory, "..", "..", ".."))
	runnerPath := filepath.Join(repoRoot, "scripts", "forge_console_browser_client_instance_views_e2e.py")
	if _, err := os.Stat(runnerPath); err != nil {
		t.Fatalf("Forge Web client-instance browser runner not found at %s: %v", runnerPath, err)
	}
	input := struct {
		PageURL      string                                                `json:"page_url"`
		AccessToken  string                                                `json:"access_token"`
		Owner        deviceplacement.Owner                                 `json:"owner"`
		SessionView  deviceplacement.ClientInstanceSessionViewObservation  `json:"session_view"`
		ResourceView deviceplacement.ClientInstanceResourceViewObservation `json:"resource_view"`
	}{
		PageURL: apiURL + "/forge/", AccessToken: token, Owner: owner,
		SessionView: sessionView, ResourceView: resourceView,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Forge Web client-instance input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "forge-browser-client-instance-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Forge Web client-instance input: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, pythonExecutable, runnerPath, inputPath)
	env := os.Environ()
	env = append(env, "FORGE_BROWSER_PYTHON="+pythonExecutable)
	if browserExecutable := os.Getenv("FORGE_BROWSER_EXECUTABLE"); browserExecutable != "" {
		env = append(env, "FORGE_BROWSER_EXECUTABLE="+browserExecutable)
	}
	command.Env = env
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Forge Web client-instance browser E2E failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Forge Web client-instance browser E2E output exceeded the size limit")
	}
}
