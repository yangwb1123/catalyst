package appserver

// This test proves the lossless inventory v2 candidate across the real
// Snaplink JWT boundary and a separate Flutter process. The candidate is
// mounted only on an explicitly assembled test mux; production routes remain
// unregistered and therefore return 404.

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"testing"
	"time"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestSnaplinkAuthenticatedLosslessInventoryV2FlutterE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CONSOLE_E2E") != "1" {
		t.Skip("set FORGE_CONSOLE_E2E=1 for Snaplink JWT to Flutter v2 inventory E2E")
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
	source := &fixtureDeviceInventoryReadV2Source{
		value: fixtureDeviceInventoryReadV2ValueMultiple(owner),
	}
	testRoutes := http.NewServeMux()
	testRoutes.Handle(deviceInventoryReadCandidateV2Path,
		newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
			Enabled: true, Source: source,
		}))
	testRoutes.Handle("/", http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		// The Sessions widget performs its ordinary owner-scoped bootstrap
		// reads before loading the explicitly injected v2 candidate. Keep those
		// reads bounded and empty so this test exercises the same candidate
		// response without constructing a Runtime bridge.
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
	if os.Getenv("FORGE_BROWSER_E2E") == "1" {
		webBuildDir := os.Getenv("FORGE_WEB_BUILD_DIR")
		if webBuildDir == "" {
			t.Fatal("FORGE_WEB_BUILD_DIR is required when FORGE_BROWSER_E2E=1")
		}
		serverHandler = serveForgeConsoleWebAssets(webBuildDir, serverHandler)
	}
	server := httptest.NewServer(serverHandler)
	t.Cleanup(server.Close)

	response := snaplinkConversationRequest(t, server.Client(), server.URL, token,
		http.MethodGet, deviceInventoryReadCandidateV2Path, "", "")
	if response.StatusCode != http.StatusOK {
		t.Fatalf("Snaplink JWT v2 candidate status=%d body=%q", response.StatusCode, readConversationClientBody(t, response))
	}
	var decoded deviceplacement.SessionDeviceObservationInventoryV2
	if err := json.NewDecoder(response.Body).Decode(&decoded); err != nil {
		_ = response.Body.Close()
		t.Fatalf("decode Snaplink JWT v2 candidate: %v", err)
	}
	_ = response.Body.Close()
	if !reflect.DeepEqual(decoded, source.value) ||
		decoded.Owner != owner || len(decoded.Devices) != 2 ||
		decoded.Devices[0].Device.ReservationState != "reserved" ||
		len(decoded.Devices[0].Device.GPUs) != 2 ||
		decoded.Devices[1].Device.ReservationState != "none" ||
		len(decoded.Devices[1].Device.GPUs) != 0 {
		t.Fatalf("Snaplink JWT v2 candidate=%#v source=%#v", decoded, source.value)
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventoryV2(decoded); err != nil {
		t.Fatalf("Snaplink JWT v2 candidate validation: %v", err)
	}

	runForgeConsoleDeviceInventoryV2CandidateE2EWithToken(
		t, server.URL, token, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant,
	)
	runForgeRuntimeDeviceInventoryV2CandidateE2EWithToken(t, server.URL, token, source.value)
	if os.Getenv("FORGE_BROWSER_E2E") == "1" {
		runForgeConsoleBrowserDeviceInventoryV2E2EWithToken(t, server.URL, token, owner, source.value)
	}
	if os.Getenv("FORGE_RUNTIME_BIN") != "" {
		runForgeRuntimePersistedInventoryV2TUIE2EWithToken(t, server.URL, token)
	}
	expectedSourceCalls := 3
	if os.Getenv("FORGE_BROWSER_E2E") == "1" {
		expectedSourceCalls++
	}
	if os.Getenv("FORGE_RUNTIME_BIN") != "" {
		expectedSourceCalls += 2
	}
	if source.calls != expectedSourceCalls || source.owner != (model.Owner{
		Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant,
	}) {
		t.Fatalf("v2 candidate source=%#v; expected %d verified-owner reads", source, expectedSourceCalls)
	}

	production := authenticator.Handler(newConversationRoutes(nil))
	request, err := http.NewRequest(http.MethodGet, server.URL+deviceInventoryReadCandidateV2Path, nil)
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Authorization", "Bearer "+token)
	productionResponse := httptest.NewRecorder()
	production.ServeHTTP(productionResponse, request)
	if productionResponse.Code != http.StatusNotFound {
		t.Fatalf("production v2 inventory route status=%d body=%q", productionResponse.Code, productionResponse.Body.String())
	}
}

// When FORGE_RUNTIME_BIN is provided, extend the same Snaplink JWT fixture
// through the real Rust CLI. The candidate remains mounted only on this test
// mux; the production constructor check below still proves the route is 404.
func runForgeRuntimeDeviceInventoryV2CandidateE2EWithToken(
	t *testing.T, apiURL, token string, expected deviceplacement.SessionDeviceObservationInventoryV2,
) {
	t.Helper()
	executable := os.Getenv("FORGE_RUNTIME_BIN")
	if executable == "" {
		return
	}
	output, stderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, token, t.TempDir(), "--json", "remote", "inventory", "show-v2",
	)
	if err != nil {
		t.Fatalf("Snaplink JWT Rust CLI v2 inventory candidate failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var decoded deviceplacement.SessionDeviceObservationInventoryV2
	if err := json.Unmarshal([]byte(output), &decoded); err != nil {
		t.Fatalf("decode Snaplink JWT Rust CLI v2 candidate: %v stdout=%q", err, output)
	}
	// The accepted production assembly samples the Coordinator clock for each
	// read. A zero expected evaluation time opts this helper into comparing the
	// persisted observation rows and authority boundary while accepting that
	// per-read clock sample.
	if expected.EvaluatedAtMS == 0 {
		expected.EvaluatedAtMS = decoded.EvaluatedAtMS
	}
	if !reflect.DeepEqual(decoded, expected) {
		t.Fatalf("Snaplink JWT Rust CLI v2 candidate=%#v expected=%#v", decoded, expected)
	}
}

func runForgeConsoleDeviceInventoryV2CandidateE2EWithToken(
	t *testing.T, apiURL, token, issuer, subject, tenant string,
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
		t.Fatalf("Flutter is required for the v2 inventory E2E: %v", err)
	}
	input := struct {
		APIURL      string `json:"api_url"`
		AccessToken string `json:"access_token"`
		Issuer      string `json:"issuer"`
		Subject     string `json:"subject"`
		Tenant      string `json:"tenant_id"`
	}{APIURL: apiURL, AccessToken: token, Issuer: issuer, Subject: subject, Tenant: tenant}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Flutter v2 inventory input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "device-inventory-v2-e2e-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter v2 inventory input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub", "test/forge_device_inventory_v2_api_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_DEVICE_INVENTORY_V2_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter v2 inventory candidate E2E failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Flutter v2 inventory candidate E2E output exceeded the size limit")
	}

	// Feed the same owner-bound JWT and candidate URL through the shared
	// Sessions surface used by Web/App/Mobile. This is a second process and a
	// second authenticated read, so the source call count below proves the
	// widget did not use a local fixture or silently reuse the API result.
	sessionCommand := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub", "test/forge_device_inventory_v2_sessions_e2e_test.dart")
	sessionCommand.Dir = consoleRoot
	sessionCommand.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_DEVICE_INVENTORY_V2_SESSIONS_E2E_INPUT="+inputPath,
	)
	var sessionStdout, sessionStderr boundedCLIOutput
	sessionCommand.Stdout = &sessionStdout
	sessionCommand.Stderr = &sessionStderr
	if err := sessionCommand.Run(); err != nil {
		t.Fatalf("Flutter v2 inventory Sessions E2E failed: stdout=%q stderr=%q err=%v",
			sessionStdout.String(), sessionStderr.String(), err)
	}
	if sessionStdout.exceeded || sessionStderr.exceeded {
		t.Fatalf("Flutter v2 inventory Sessions E2E output exceeded the size limit")
	}
}

// runForgeConsoleBrowserDeviceInventoryV2E2EWithToken exercises the same
// owner-bound v2 candidate from Chromium. The ordinary Web Gate remains
// request-free; this journey explicitly issues one authenticated GET from the
// browser page and compares the response with the canonical source value.
func runForgeConsoleBrowserDeviceInventoryV2E2EWithToken(
	t *testing.T,
	apiURL, token string,
	owner deviceplacement.Owner,
	expected deviceplacement.SessionDeviceObservationInventoryV2,
) {
	t.Helper()
	pythonBinary := os.Getenv("FORGE_BROWSER_PYTHON")
	if pythonBinary == "" {
		pythonBinary = "python3"
	}
	pythonExecutable, err := exec.LookPath(pythonBinary)
	if err != nil {
		t.Fatalf("Python is required when FORGE_BROWSER_E2E=1: %v", err)
	}
	workingDirectory, err := os.Getwd()
	if err != nil {
		t.Fatalf("resolve Forge Core repository: %v", err)
	}
	repoRoot := filepath.Clean(filepath.Join(workingDirectory, "..", "..", ".."))
	runnerPath := filepath.Join(repoRoot, "scripts", "forge_console_browser_inventory_v2_e2e.py")
	if _, err := os.Stat(runnerPath); err != nil {
		t.Fatalf("Forge Web v2 inventory browser runner not found at %s: %v", runnerPath, err)
	}
	input := struct {
		PageURL     string                                              `json:"page_url"`
		AccessToken string                                              `json:"access_token"`
		Owner       deviceplacement.Owner                               `json:"owner"`
		Inventory   deviceplacement.SessionDeviceObservationInventoryV2 `json:"inventory"`
	}{
		PageURL: apiURL + "/forge/", AccessToken: token,
		Owner: owner, Inventory: expected,
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode Forge Web v2 inventory input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "forge-browser-inventory-v2-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Forge Web v2 inventory input: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, pythonExecutable, runnerPath, inputPath)
	// Keep the caller's Python user-site environment intact. Unlike the
	// isolated Flutter process above, Playwright is commonly installed in the
	// invoking user's site-packages; replacing HOME would hide that module.
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
		t.Fatalf("Forge Web v2 inventory browser E2E failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Forge Web v2 inventory browser E2E output exceeded the size limit")
	}
}

func fixtureDeviceInventoryReadV2ValueMultiple(owner deviceplacement.Owner) deviceplacement.SessionDeviceObservationInventoryV2 {
	value := fixtureDeviceInventoryReadV2Value(owner)
	value.Devices = append(value.Devices, deviceplacement.SessionPlacementCandidateV2{
		InstanceID: "runner-b", Revision: 2, Generation: 2, HeartbeatSequence: 4,
		Device: deviceplacement.DeviceV2{
			DeviceID: "device-b", Owner: owner, ApprovalState: "pending", CordonState: "cordoned",
			ReservationState: "none", Liveness: "offline", SnapshotObservedAtMS: 100_000,
			LeaseExpiresAtMS: 200_000, OS: "linux", Architecture: "amd64", AvailableCPUCores: 7,
			AvailableMemoryBytes: 8 << 30, AvailableStorage: 50 << 30,
			Runtimes: []string{"go", "rust"}, GPUs: []deviceplacement.GPUDeclarationV2{},
			DataResidencyZones: []string{}, TrustZone: "unknown", SandboxLevels: []string{},
			ConcurrencyLimit: 0, ActiveConcurrency: 0,
		},
	})
	return value
}
