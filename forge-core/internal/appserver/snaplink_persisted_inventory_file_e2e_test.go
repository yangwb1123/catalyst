package appserver

// This opt-in test crosses the real Snaplink JWT boundary, the explicitly
// mounted private inventory candidate, and a separate Rust CLI process. The
// source is a private file-backed owner-scoped read adapter; production route
// constructors remain untouched and are checked for 404 below.

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
)

func TestSnaplinkAuthenticatedPersistedInventoryFileRustCLIE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_INVENTORY_FILE_E2E") != "1" {
		t.Skip("set FORGE_INVENTORY_FILE_E2E=1 for persisted inventory file to Rust CLI E2E")
	}
	if os.Getenv("FORGE_RUNTIME_BIN") == "" {
		t.Skip("set FORGE_RUNTIME_BIN with FORGE_INVENTORY_FILE_E2E=1 for persisted inventory file E2E")
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
	second.Runner.ServerObservedAtMS = 100_000
	second.Runner.CapabilityLeaseExpiresAtMS = 200_000
	second.Device.ApprovalState = "pending"
	second.Device.CordonState = "cordoned"
	second.Device.ReservationState = "none"
	second.Runner.Liveness = "offline"
	path := writePersistedInventoryFileSetForCandidate(t, owner, []deviceinventory.PersistedInventoryState{second, first})
	source := newPersistedInventoryFileSetReadSource(path, 200_000)
	testRoutes := http.NewServeMux()
	testRoutes.Handle(deviceInventoryReadCandidateV2Path,
		newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
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
		http.MethodGet, deviceInventoryReadCandidateV2Path, "", "")
	if response.StatusCode != http.StatusOK {
		t.Fatalf("file-backed JWT v2 candidate status=%d body=%q", response.StatusCode, readConversationClientBody(t, response))
	}
	var expected deviceplacement.SessionDeviceObservationInventoryV2
	if err := json.NewDecoder(response.Body).Decode(&expected); err != nil {
		_ = response.Body.Close()
		t.Fatalf("decode file-backed JWT v2 candidate: %v", err)
	}
	_ = response.Body.Close()
	if err := deviceplacement.ValidateSessionDeviceObservationInventoryV2(expected); err != nil {
		t.Fatalf("validate file-backed JWT v2 candidate: %v", err)
	}
	if expected.Owner.Subject != snaplinkForgeTestUser || len(expected.Devices) != 2 ||
		expected.Devices[0].Device.DeviceID != "device-a" || expected.Devices[0].InstanceID != "runner-a" ||
		expected.Devices[0].Revision != first.Revision ||
		expected.Devices[0].HeartbeatSequence != first.Runner.HeartbeatSequence ||
		len(expected.Devices[0].Device.GPUs) != 2 ||
		expected.Devices[1].Device.DeviceID != "device-b" || expected.Devices[1].InstanceID != "runner-b" ||
		expected.Devices[1].Revision != second.Revision ||
		expected.Devices[1].HeartbeatSequence != second.Runner.HeartbeatSequence ||
		expected.Devices[0].Device.ReservationState != "reserved" ||
		expected.ExecutionAuthorized || expected.ReservationCreated || expected.DispatchPerformed {
		t.Fatalf("file-backed JWT v2 candidate=%#v", expected)
	}

	runForgeRuntimeDeviceInventoryV2CandidateE2EWithToken(t, server.URL, token, expected)
	runForgeRuntimePersistedInventoryV2TUIE2EWithToken(t, server.URL, token)
	if os.Getenv("FORGE_CONSOLE_E2E") == "1" {
		runForgeConsoleDeviceInventoryV2CandidateE2EWithToken(
			t, server.URL, token, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant,
		)
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

func runForgeRuntimePersistedInventoryV2TUIE2EWithToken(t *testing.T, apiURL, accessToken string) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("the persisted inventory TUI E2E requires a PTY launcher named script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", os.Getenv("FORGE_RUNTIME_BIN"), "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader("inventory read-v2\nquit\n")
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("persisted inventory Rust TUI failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("persisted inventory Rust TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"remote device inventory [forge.device-inventory-observation/v2]",
		"device-a / runner-a",
		"revision=1 generation=1 heartbeat=1",
		"device-b / runner-b",
		"revision=2 generation=2 heartbeat=4",
		"reservation=reserved",
		"authority: identity_verified=false",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("persisted inventory Rust TUI output omitted %q: %q", want, output)
		}
	}
}
