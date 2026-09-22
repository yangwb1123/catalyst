package appserver

// This opt-in acceptance test crosses the real Snaplink JWT boundary for the
// pure execution-reconciliation candidate. Each Web/App/Mobile client reads
// the shared resource view before posting the same owner/Run-bound restart
// image; the response is metadata-only and carries no execution authority.

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/deviceplacement"
)

func TestSnaplinkAuthenticatedClientInstanceExecutionReconciliationProjectionE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_EXECUTION_RECONCILIATION_PROJECTION_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_EXECUTION_RECONCILIATION_PROJECTION_E2E=1 for client-instance execution-reconciliation projection E2E")
	}
	identity, authenticator := newMultiPrincipalConversationTestIdentity(t)
	owner := deviceplacement.Owner{
		Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate",
	}
	localCandidate := newConversationRoutesWithInertExecutionAPIAndLocalRunnerPreview(nil, nil, nil)
	resourceSource := &fixtureClientInstanceResourceViewSource{
		value: clientInstanceDispatchPlanResourceView(owner),
	}
	resourceCandidate := newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
		Enabled: true,
		Source:  resourceSource,
	})
	handler := authenticator.Handler(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method == http.MethodGet && r.URL.Path == clientInstanceResourceViewCandidatePath {
			resourceCandidate.ServeHTTP(w, r)
			return
		}
		localCandidate.ServeHTTP(w, r)
	}))
	server := httptest.NewServer(handler)
	t.Cleanup(server.Close)

	for _, client := range []struct {
		name       string
		instanceID string
	}{
		{name: "web", instanceID: "client-web-001"},
		{name: "app", instanceID: "client-app-001"},
		{name: "mobile", instanceID: "client-mobile-001"},
	} {
		client := client
		t.Run(client.name, func(t *testing.T) {
			token := tokenForIndependentClient(
				identity,
				"forge:conversations:read forge:devices:read",
				"execution-reconciliation-projection-"+client.name,
			)
			runForgeConsoleClientInstanceExecutionReconciliationProjectionE2E(
				t, server.URL, token, owner, client.instanceID,
			)
		})
	}

	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	resourceRequest := httptest.NewRequest(http.MethodGet, clientInstanceResourceViewCandidatePath, nil)
	resourceRequest.Header.Set("Authorization", "Bearer "+tokenForIndependentClient(
		identity, "forge:devices:read", "execution-reconciliation-production-resource",
	))
	resourceResponse := httptest.NewRecorder()
	production.ServeHTTP(resourceResponse, resourceRequest)
	if resourceResponse.Code != http.StatusNotFound {
		t.Fatalf("production client-instance resource route status=%d body=%q", resourceResponse.Code, resourceResponse.Body.String())
	}

	input := executionReconciliationInput(t, owner, "conversation-001", "run-001")
	body, err := json.Marshal(input)
	if err != nil {
		t.Fatal(err)
	}
	previewPath := "/api/v1/conversations/conversation-001/runs/run-001/execution-reconciliation/preview"
	previewRequest := httptest.NewRequest(http.MethodPost, previewPath, strings.NewReader(string(body)))
	previewRequest.Header.Set("Authorization", "Bearer "+tokenForIndependentClient(
		identity, "forge:conversations:read", "execution-reconciliation-production-preview",
	))
	previewRequest.Header.Set("Content-Type", "application/json")
	previewResponse := httptest.NewRecorder()
	production.ServeHTTP(previewResponse, previewRequest)
	if previewResponse.Code != http.StatusNotFound {
		t.Fatalf("production execution-reconciliation route status=%d body=%q", previewResponse.Code, previewResponse.Body.String())
	}
}

func runForgeConsoleClientInstanceExecutionReconciliationProjectionE2E(
	t *testing.T,
	apiURL, token string,
	owner deviceplacement.Owner,
	instanceID string,
) {
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
		t.Fatalf("Flutter is required for execution-reconciliation projection E2E: %v", err)
	}
	inputJSON, err := json.Marshal(struct {
		APIURL         string                `json:"api_url"`
		AccessToken    string                `json:"access_token"`
		Owner          deviceplacement.Owner `json:"owner"`
		InstanceID     string                `json:"instance_id"`
		ConversationID string                `json:"conversation_id"`
	}{
		APIURL: apiURL, AccessToken: token, Owner: owner, InstanceID: instanceID,
		ConversationID: "conversation-001",
	})
	if err != nil {
		t.Fatalf("encode Flutter execution-reconciliation projection input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "client-instance-execution-reconciliation-projection-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter execution-reconciliation projection input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub", "test/forge_client_instance_execution_reconciliation_projection_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_CLIENT_INSTANCE_EXECUTION_RECONCILIATION_PROJECTION_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter execution-reconciliation projection E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("Flutter execution-reconciliation projection output exceeded the size limit")
	}
}
