package appserver

// This opt-in test crosses the real Snaplink JWT boundary, the explicitly
// mounted persisted-inventory/resource-view candidate, Rust CLI/TUI readers,
// and the Flutter API reader. The candidate is mounted only on a test mux;
// production route wiring remains closed and is asserted as 404.

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

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
)

func TestSnaplinkAuthenticatedClientInstanceResourceViewE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_RESOURCE_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_RESOURCE_E2E=1 for client-instance/resource-view E2E")
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

	owner := deviceinventory.SnapshotOwner{
		Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant,
	}
	first := persistedInventoryReadSourceState(t, owner)
	first.Device.ReservationState = "reserved"
	first.Runner.Capabilities.GPUs = []deviceheartbeat.GPUCapability{
		{ID: "gpu-a", Vendor: "NVIDIA", MemoryBytes: 16 << 30, AvailableMemoryBytes: 12 << 30},
		{ID: "gpu-b", Vendor: "NVIDIA", MemoryBytes: 8 << 30, AvailableMemoryBytes: 4 << 30},
	}
	second := persistedInventoryReadSourceState(t, owner)
	second.Revision = 2
	second.Device.DeviceID = "device-b"
	second.Runner.DeviceID = "device-b"
	second.Runner.InstanceID = "runner-b"
	second.Runner.Generation = 2
	second.Runner.HeartbeatSequence = 4
	second.Runner.Liveness = "offline"
	second.Device.ApprovalState = "pending"
	second.Device.CordonState = "cordoned"
	instances := []deviceplacement.ClientInstanceSessionViewInstance{
		{InstanceID: "client-mobile", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{"conversation-a"}, ObservedAtMS: 200_500, Status: "offline"},
		{InstanceID: "client-app", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{"conversation-a"}, ObservedAtMS: 200_500, Status: "idle"},
		{InstanceID: "client-web", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{"conversation-a", "conversation-b"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-tui", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{"conversation-b"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-cli", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{"conversation-a", "conversation-b"}, ObservedAtMS: 200_500, Status: "active"},
	}
	path := writePersistedInventoryFileSetForCandidate(t, owner, []deviceinventory.PersistedInventoryState{second, first})
	source := newPersistedInventoryFileSetClientInstanceResourceViewSource(path, instances)
	testRoutes := http.NewServeMux()
	testRoutes.Handle(clientInstanceResourceViewCandidatePath,
		newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
			Enabled: true, Source: source,
		}))
	testRoutes.Handle("/", http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		if request.URL.Path == "/api/v1/conversations" && request.Method == http.MethodGet {
			writer.Header().Set("Content-Type", "application/json")
			_, _ = writer.Write([]byte(`{"conversations":[],"next_after_id":null,"has_more":false}`))
			return
		}
		newConversationRoutes(nil).ServeHTTP(writer, request)
	}))
	server := httptest.NewServer(authenticator.Handler(testRoutes))
	t.Cleanup(server.Close)

	response := snaplinkConversationRequest(t, server.Client(), server.URL, token,
		http.MethodGet, clientInstanceResourceViewCandidatePath, "", "")
	if response.StatusCode != http.StatusOK {
		t.Fatalf("resource view candidate status=%d body=%q", response.StatusCode, readConversationClientBody(t, response))
	}
	var expected deviceplacement.ClientInstanceResourceViewObservation
	body := readConversationClientBody(t, response)
	if err := json.Unmarshal([]byte(body), &expected); err != nil {
		t.Fatalf("decode resource view candidate: %v body=%q", err, body)
	}
	if err := expected.Validate(); err != nil {
		t.Fatalf("validate resource view candidate: %v", err)
	}
	if expected.Owner.Subject != snaplinkForgeTestUser || len(expected.Instances) != 5 ||
		expected.Instances[0].InstanceID != "client-app" || expected.Instances[4].InstanceID != "client-web" ||
		len(expected.Devices) != 2 || expected.Devices[0].DeviceID != "device-a" ||
		expected.Devices[0].RunnerInstanceID != "runner-a" || expected.Devices[0].GPUCount != 2 ||
		expected.Devices[0].AvailableGPUMemoryBytes != 16<<30 || expected.Devices[0].ReservationState != "reserved" ||
		expected.Devices[1].DeviceID != "device-b" || expected.Devices[1].RunnerInstanceID != "runner-b" ||
		expected.Devices[1].Liveness != "offline" || expected.Authority != (deviceplacement.ClientInstanceResourceViewAuthority{}) {
		t.Fatalf("resource view candidate=%#v", expected)
	}

	resourcePath := filepath.Join(t.TempDir(), "client-instance-resource-view.json")
	if err := os.WriteFile(resourcePath, []byte(body), 0o600); err != nil {
		t.Fatal(err)
	}
	if executable := os.Getenv("FORGE_RUNTIME_BIN"); executable != "" {
		runForgeRuntimeClientInstanceResourceViewCLI(t, executable, resourcePath)
		runForgeRuntimeClientInstanceResourceViewTUI(t, executable, server.URL, token, resourcePath)
		runForgeRuntimeClientInstanceResourceViewRemoteCLI(t, executable, server.URL, token)
		runForgeRuntimeClientInstanceResourceViewRemoteTUI(t, executable, server.URL, token)
	}
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		runForgeConsoleClientInstanceResourceViewE2EWithToken(t, server.URL, token, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant)
	}

	production := authenticator.Handler(newConversationRoutes(nil))
	request, err := http.NewRequest(http.MethodGet, server.URL+clientInstanceResourceViewCandidatePath, nil)
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Authorization", "Bearer "+token)
	productionResponse := httptest.NewRecorder()
	production.ServeHTTP(productionResponse, request)
	if productionResponse.Code != http.StatusNotFound {
		t.Fatalf("production resource view route status=%d body=%q", productionResponse.Code, productionResponse.Body.String())
	}
}

func runForgeRuntimeClientInstanceResourceViewCLI(t *testing.T, executable, inputPath string) {
	t.Helper()
	output, stderr, err := runForgeRuntimeCLI(t, executable, "https://127.0.0.1:1", "offline", t.TempDir(),
		"--json", "device", "client-instance-resource-view-preview", "--input", inputPath)
	if err != nil {
		t.Fatalf("client-instance/resource-view Rust CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var decoded deviceplacement.ClientInstanceResourceViewObservation
	if err := json.Unmarshal([]byte(output), &decoded); err != nil {
		t.Fatalf("decode Rust CLI resource view: %v stdout=%q", err, output)
	}
	if err := decoded.Validate(); err != nil || len(decoded.Instances) != 5 || len(decoded.Devices) != 2 ||
		decoded.Devices[0].RunnerInstanceID != "runner-a" || decoded.Devices[1].RunnerInstanceID != "runner-b" {
		t.Fatalf("Rust CLI resource view=%#v err=%v", decoded, err)
	}
}

func runForgeRuntimeClientInstanceResourceViewRemoteCLI(t *testing.T, executable, apiURL, accessToken string) {
	t.Helper()
	output, stderr, err := runForgeRuntimeCLI(t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "remote", "client-instances", "resource-view")
	if err != nil {
		t.Fatalf("authenticated client-instance/resource-view Rust CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var decoded deviceplacement.ClientInstanceResourceViewObservation
	if err := json.Unmarshal([]byte(output), &decoded); err != nil {
		t.Fatalf("decode authenticated Rust CLI resource view: %v stdout=%q", err, output)
	}
	if err := decoded.Validate(); err != nil || len(decoded.Instances) != 5 || len(decoded.Devices) != 2 ||
		decoded.Devices[0].RunnerInstanceID != "runner-a" || decoded.Devices[1].RunnerInstanceID != "runner-b" {
		t.Fatalf("authenticated Rust CLI resource view=%#v err=%v", decoded, err)
	}
}

func runForgeRuntimeClientInstanceResourceViewTUI(t *testing.T, executable, apiURL, accessToken, inputPath string) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("client-instance/resource-view TUI E2E requires script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("client-instance-resource-view-preview --input " + inputPath + "\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("client-instance/resource-view Rust TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("client-instance/resource-view Rust TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"offline client-instance/resource-view [forge.client-instance-resource-view/v1]",
		"instance client-app:", "instance client-cli:", "instance client-mobile:",
		"device device-a: runner=runner-a", "device device-b: runner=runner-b",
		"authority: owner_authenticated=false",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("client-instance/resource-view TUI output omitted %q: %q", want, output)
		}
	}
}

func runForgeRuntimeClientInstanceResourceViewRemoteTUI(t *testing.T, executable, apiURL, accessToken string) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("authenticated client-instance/resource-view TUI E2E requires script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("client-instances resource-view\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated client-instance/resource-view Rust TUI failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("authenticated client-instance/resource-view Rust TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"remote client-instance/resource-view [forge.client-instance-resource-view/v1]",
		"instance client-app:", "instance client-cli:", "instance client-mobile:",
		"device device-a: runner=runner-a", "device device-b: runner=runner-b",
		"authority: owner_authenticated=false",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("authenticated client-instance/resource-view TUI output omitted %q: %q", want, output)
		}
	}
}

func runForgeConsoleClientInstanceResourceViewE2EWithToken(
	t *testing.T, apiURL, token, issuer, subject, tenant string,
) {
	runForgeConsoleClientInstanceResourceViewE2EWithExpectation(
		t, apiURL, token, issuer, subject, tenant,
		forgeConsoleClientInstanceResourceViewExpectation{
			GPUCount:                2,
			AvailableGPUMemoryBytes: 16 << 30,
			ReservationState:        "reserved",
		},
	)
}

func runForgeConsoleClientInstanceResourceViewE2EWithGenericResources(
	t *testing.T, apiURL, token, issuer, subject, tenant string,
) {
	runForgeConsoleClientInstanceResourceViewE2EWithExpectation(
		t, apiURL, token, issuer, subject, tenant,
		forgeConsoleClientInstanceResourceViewExpectation{
			GPUCount:                0,
			AvailableGPUMemoryBytes: 0,
			ReservationState:        "none",
		},
	)
}

type forgeConsoleClientInstanceResourceViewExpectation struct {
	GPUCount                int
	AvailableGPUMemoryBytes int64
	ReservationState        string
}

func runForgeConsoleClientInstanceResourceViewE2EWithExpectation(
	t *testing.T, apiURL, token, issuer, subject, tenant string,
	expectation forgeConsoleClientInstanceResourceViewExpectation,
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
		t.Fatalf("Flutter is required for the client-instance/resource-view E2E: %v", err)
	}
	input := struct {
		APIURL      string `json:"api_url"`
		AccessToken string `json:"access_token"`
		Issuer      string `json:"issuer"`
		Subject     string `json:"subject"`
		Tenant      string `json:"tenant_id"`
		GPUCount    int    `json:"expected_gpu_count"`
		GPUMemory   int64  `json:"expected_available_gpu_memory_bytes"`
		Reservation string `json:"expected_reservation_state"`
	}{
		APIURL: apiURL, AccessToken: token, Issuer: issuer, Subject: subject, Tenant: tenant,
		GPUCount: expectation.GPUCount, GPUMemory: expectation.AvailableGPUMemoryBytes,
		Reservation: expectation.ReservationState,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter resource view input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "client-instance-resource-view-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter resource view input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub", "test/forge_client_instance_resource_view_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter client-instance/resource-view E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Flutter client-instance/resource-view E2E output exceeded the size limit")
	}
}
