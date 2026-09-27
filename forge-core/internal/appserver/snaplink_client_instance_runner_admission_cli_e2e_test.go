package appserver

// This opt-in integration test proves that the Runtime CLI admission previews
// refresh the selected client-instance and owner-bound resource images before
// their metadata-only candidate POST. The candidate routes use a persisted
// lease and owned Run reference, but never contact a Runner.

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
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/executionlease"
	"forgeos/forge-core/internal/runnertransport"
)

func TestSnaplinkAuthenticatedClientInstanceRunnerAdmissionCLIConvergenceE2EWhenConfigured(t *testing.T) {
	executable := admissionCLIExecutable(t)
	fixture := newAdmissionCLIFixture(t)
	runAdmissionCLICase(t, executable, fixture.server.URL, fixture.token, fixture.recorder,
		"runner-dispatch-admission-preview", fixture.dispatchRequest, fixture.dispatchPath, true)
	runAdmissionCLICase(t, executable, fixture.server.URL, fixture.token, fixture.recorder,
		"runner-transport-admission-preview", fixture.transportRequest, fixture.transportPath, true)
	drifted := fixture.resourceSource.value
	drifted.Devices[0].DeviceID = "device-foreign"
	drifted.Devices[0].RunnerInstanceID = "runner-foreign"
	if err := drifted.Validate(); err != nil {
		t.Fatalf("validate drifted admission resource view: %v", err)
	}
	fixture.resourceSource.value = drifted
	fixture.inventorySource.value = preflightProjectionInventory(t, fixture.owner, drifted)
	runAdmissionCLICase(t, executable, fixture.server.URL, fixture.token, fixture.recorder,
		"runner-dispatch-admission-preview", fixture.dispatchRequest, fixture.dispatchPath, false)
	runAdmissionCLICase(t, executable, fixture.server.URL, fixture.token, fixture.recorder,
		"runner-transport-admission-preview", fixture.transportRequest, fixture.transportPath, false)
	assertAdmissionCLIProductionClosed(t, fixture.authenticator, fixture.token, fixture.dispatchPath, fixture.dispatchRequest)
}

type admissionCLIFixture struct {
	authenticator    *authn.Authenticator
	server           *httptest.Server
	token            string
	recorder         *conversationHTTPRecorder
	owner            deviceplacement.Owner
	entry            executionlease.RegistryEntry
	resourceSource   *fixtureClientInstanceResourceViewSource
	inventorySource  *fixtureDeviceInventoryReadV2Source
	dispatchPath     string
	transportPath    string
	dispatchRequest  deviceplacement.RunnerDispatchAdmissionRequest
	transportRequest deviceplacement.RunnerTransportAdmissionRequest
}

func admissionCLIExecutable(t *testing.T) string {
	t.Helper()
	if os.Getenv("FORGE_CLIENT_INSTANCE_RUNNER_ADMISSION_CLI_CONVERGENCE_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_RUNNER_ADMISSION_CLI_CONVERGENCE_E2E=1 for Runtime CLI Runner admission convergence E2E")
	}
	configured := os.Getenv("FORGE_RUNTIME_BIN")
	if configured == "" {
		t.Skip("set FORGE_RUNTIME_BIN for Runtime CLI Runner admission convergence E2E")
	}
	executable, err := exec.LookPath(configured)
	if err != nil {
		t.Fatalf("FORGE_RUNTIME_BIN must name a built forge-runtime executable: %v", err)
	}
	executable, err = filepath.Abs(executable)
	if err != nil {
		t.Fatal(err)
	}
	return executable
}

func newAdmissionCLIFixture(t *testing.T) admissionCLIFixture {
	t.Helper()
	identity, authenticator := newMultiPrincipalConversationTestIdentity(t)
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	instances := admissionCLIInstances(t, owner)
	resourceSource, inventorySource := admissionCLIResourceSources(t, owner, instances)
	registryPath, entry := admissionCLILease(t, owner)
	backend := preflightProjectionConversationBackend(entry.ConversationID, entry.RunID)
	dispatch, transport := admissionCLIRoutes(registryPath, backend)
	recorder := &conversationHTTPRecorder{}
	server := admissionCLIServer(authenticator, recorder, instances, resourceSource, inventorySource, entry, dispatch, transport)
	t.Cleanup(server.Close)
	t.Cleanup(authenticator.Close)
	token := tokenForIndependentClient(identity, "forge:conversations:read forge:devices:read forge:devices:placement:lease", "runner-admission-cli-convergence")
	return admissionCLIFixture{
		authenticator: authenticator, server: server, token: token, recorder: recorder, owner: owner,
		entry: entry, resourceSource: resourceSource, inventorySource: inventorySource,
		dispatchPath:     runnerDispatchAdmissionPathIDsURL(entry.ConversationID, entry.RunID),
		transportPath:    runnerTransportAdmissionPathIDsURL(entry.ConversationID, entry.RunID),
		dispatchRequest:  admissionCLIDispatchRequest(t, owner, entry),
		transportRequest: admissionCLITransportRequest(t, owner, entry),
	}
}

func admissionCLIResourceSources(
	t *testing.T, owner deviceplacement.Owner, instances deviceplacement.ClientInstanceSessionViewObservation,
) (*fixtureClientInstanceResourceViewSource, *fixtureDeviceInventoryReadV2Source) {
	t.Helper()
	resource := fixtureClientInstanceResourceView(owner)
	resource.Instances = append([]deviceplacement.ClientInstanceSessionViewInstance(nil), instances.Instances...)
	if err := resource.Validate(); err != nil {
		t.Fatalf("validate admission resource view: %v", err)
	}
	return &fixtureClientInstanceResourceViewSource{value: resource},
		&fixtureDeviceInventoryReadV2Source{value: preflightProjectionInventory(t, owner, resource)}
}

func admissionCLIRoutes(registryPath string, backend *fakeConversationBackend) (http.Handler, http.Handler) {
	now := int64(time.Now().UnixMilli())
	clock := func(context.Context) (int64, error) { return now, nil }
	return newRunnerDispatchAdmissionRoutes(&runnerDispatchAdmissionConfig{Enabled: true, RegistryPath: registryPath, Now: clock, Backend: backend}),
		newRunnerTransportAdmissionRoutes(&runnerTransportAdmissionConfig{Enabled: true, RegistryPath: registryPath, Now: clock, Backend: backend})
}

func admissionCLIServer(
	authenticator *authn.Authenticator, recorder *conversationHTTPRecorder,
	instances deviceplacement.ClientInstanceSessionViewObservation,
	resource *fixtureClientInstanceResourceViewSource, inventory *fixtureDeviceInventoryReadV2Source,
	entry executionlease.RegistryEntry, dispatch, transport http.Handler,
) *httptest.Server {
	testRoutes := http.NewServeMux()
	testRoutes.Handle(clientInstanceSessionViewCandidatePath,
		newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{Enabled: true, Source: &fixtureClientInstanceSessionViewSource{value: instances}}))
	testRoutes.Handle(clientInstanceResourceViewCandidatePath,
		newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{Enabled: true, Source: resource}))
	testRoutes.Handle(deviceInventoryReadCandidateV2Path,
		newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{Enabled: true, Source: inventory}))
	testRoutes.Handle(runnerDispatchAdmissionPathIDsURL(entry.ConversationID, entry.RunID), dispatch)
	testRoutes.Handle(runnerTransportAdmissionPathIDsURL(entry.ConversationID, entry.RunID), transport)
	server := httptest.NewServer(authenticator.Handler(recorder.wrap(testRoutes)))
	return server
}

func assertAdmissionCLIProductionClosed(
	t *testing.T, authenticator *authn.Authenticator, token, path string,
	request deviceplacement.RunnerDispatchAdmissionRequest,
) {
	t.Helper()
	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	req := httptest.NewRequest(http.MethodPost, path, strings.NewReader(string(body)))
	req.Header.Set("Authorization", "Bearer "+token)
	req.Header.Set("Content-Type", "application/json")
	response := httptest.NewRecorder()
	production.ServeHTTP(response, req)
	if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
		t.Fatalf("default production Runner admission status=%d body=%q", response.Code, response.Body.String())
	}
}

func admissionCLIInstances(t *testing.T, owner deviceplacement.Owner) deviceplacement.ClientInstanceSessionViewObservation {
	t.Helper()
	view, err := deviceplacement.ObserveClientInstanceSessionView(deviceplacement.ClientInstanceSessionViewRequest{
		Owner: owner,
		Instances: []deviceplacement.ClientInstanceSessionViewInstance{
			{InstanceID: "client-cli-001", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{"conversation-001"}, ObservedAtMS: 200_500, Status: "active"},
			{InstanceID: "client-tui-001", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{"conversation-002"}, ObservedAtMS: 200_500, Status: "active"},
		},
	})
	if err != nil {
		t.Fatalf("observe admission client instances: %v", err)
	}
	return view
}

func admissionCLILease(t *testing.T, owner deviceplacement.Owner) (string, executionlease.RegistryEntry) {
	t.Helper()
	lifecycleOwner := deviceidentity.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
	state := lifecycleRegistrySourceState(t, lifecycleOwner, "device-a", "runner-a", 1)
	registryPath := writeLifecycleRegistrySourceFile(t, lifecycleOwner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})
	leasePath := filepath.Join(filepath.Dir(registryPath), "leases.json")
	adapter, err := deviceinventory.NewPersistedExecutionLeaseRegistryFileSetWriteAdapterWithTokenSource(
		leasePath, lifecycleOwner, func() (string, error) { return "token-a", nil },
	)
	if err != nil {
		t.Fatal(err)
	}
	now := uint64(time.Now().UnixMilli())
	entry, replayed, err := adapter.Claim(context.Background(), executionlease.ClaimRequest{
		ConversationID: "conversation-001", RunID: "run-001", AttemptID: "attempt-001",
		IdempotencyKey: "admission-cli-lease-key-0001", RequestSHA256: executionlease.RequestDigest([]byte("admission-cli-lease")),
		IssuedAtMS: now - 1_000, TTLMS: 30_000,
	}, []executionlease.ClaimCandidate{{DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1, HeartbeatSequence: 1}})
	if err != nil || replayed {
		t.Fatalf("claim admission CLI lease entry=%#v replayed=%v err=%v", entry, replayed, err)
	}
	return leasePath, entry
}

func admissionCLIDispatchRequest(t *testing.T, owner deviceplacement.Owner, entry executionlease.RegistryEntry) deviceplacement.RunnerDispatchAdmissionRequest {
	t.Helper()
	return deviceplacement.RunnerDispatchAdmissionRequest{
		Owner: owner, ConversationID: entry.ConversationID, RunID: entry.RunID, AttemptID: entry.AttemptID,
		AttemptState: "accepted", EvaluatedAtMS: 1,
		Command: deviceplacement.RunnerExecutionCommand{
			V: 1, CommandID: "command-admission-cli-1",
			LeaseProof:     deviceplacement.RunnerExecutionLeaseProof{AttemptID: entry.AttemptID, TargetID: entry.Grant.TargetID, Epoch: entry.Grant.Epoch, FencingToken: entry.Grant.FencingToken},
			IdempotencyKey: entry.RunID + ":" + entry.AttemptID + ":command-admission-cli-1", WorkspaceRef: "workspace-admission-cli", Argv: []string{"forge-task", "--prompt-ref", "prompt-001"}, TimeoutMS: 5_000, MaxOutputBytes: 65_536,
		},
	}
}

func admissionCLITransportRequest(t *testing.T, owner deviceplacement.Owner, entry executionlease.RegistryEntry) deviceplacement.RunnerTransportAdmissionRequest {
	t.Helper()
	command := admissionCLIDispatchRequest(t, owner, entry).Command
	transport := admissionCLITransportObservation(t, command, entry.Grant.TargetID)
	return deviceplacement.RunnerTransportAdmissionRequest{
		Owner: owner, ConversationID: entry.ConversationID, RunID: entry.RunID, AttemptID: entry.AttemptID,
		AttemptState: "accepted", Command: command,
		Lease:     deviceplacement.RunnerDispatchAdmissionLease{TargetID: entry.InstanceID, Epoch: entry.Grant.Epoch, IssuedAtMS: entry.Grant.IssuedAtMS, ExpiresAtMS: entry.Grant.ExpiresAtMS, Current: true, Active: true},
		Transport: transport, ExpectedPayloadSHA256: transport.PayloadSHA256, EvaluatedAtMS: 1,
	}
}

func admissionCLITransportObservation(
	t *testing.T, command deviceplacement.RunnerExecutionCommand, targetID string,
) runnertransport.Observation {
	t.Helper()
	path := deviceplacement.TransportPayloadBindingPath(targetID)
	payload, err := json.Marshal(struct {
		AttemptID string `json:"attempt_id"`
		CommandID string `json:"command_id"`
		TargetID  string `json:"target_id"`
	}{AttemptID: command.LeaseProof.AttemptID, CommandID: command.CommandID, TargetID: targetID})
	if err != nil {
		t.Fatal(err)
	}
	const timestamp = int64(1_700_000_000)
	const nonce = "admission-cli-transport-nonce"
	// secret-scan:ignore — deterministic HMAC fixture, never used outside this test.
	const secret = "admission-cli-transport-secret"
	signature, err := runnertransport.Sign(secret, http.MethodPost, path, timestamp, nonce, payload)
	if err != nil {
		t.Fatal(err)
	}
	return mustVerifyAdmissionCLITransport(t, secret, path, timestamp, nonce, signature, payload)
}

func mustVerifyAdmissionCLITransport(
	t *testing.T, secret, path string, timestamp int64, nonce, signature string, payload []byte,
) runnertransport.Observation {
	t.Helper()
	observation, err := runnertransport.Verify(secret, http.MethodPost, path, runnertransport.Envelope{
		TS: timestamp, Nonce: nonce, Sig: signature, Payload: json.RawMessage(payload),
	}, timestamp, runnertransport.NewReplayCache(4))
	if err != nil {
		t.Fatalf("verify admission CLI transport fixture: %v", err)
	}
	return observation
}

func runAdmissionCLICase(
	t *testing.T, executable, apiURL, token string, recorder *conversationHTTPRecorder,
	command string, request any, previewPath string, wantPost bool,
) {
	t.Helper()
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	inputPath := filepath.Join(t.TempDir(), "runner-admission-cli-request.json")
	if err := os.WriteFile(inputPath, body, 0o600); err != nil {
		t.Fatal(err)
	}
	start := len(recorder.snapshot())
	output, stderr, runErr := runForgeRuntimeCLI(t, executable, apiURL, token, t.TempDir(), "--json", "remote", "placement", command, "--input", inputPath, "--instance", "client-cli-001")
	requests := recorder.snapshot()[start:]
	if wantPost {
		if runErr != nil {
			t.Fatalf("instance-filtered %s failed: stderr=%q stdout=%q err=%v", command, stderr, output, runErr)
		}
		if !strings.Contains(output, "preview_only") {
			t.Fatalf("instance-filtered %s omitted preview metadata: %q", command, output)
		}
		assertAdmissionCLIRequestOrder(t, requests, previewPath, true)
		return
	}
	if runErr == nil || !strings.Contains(stderr, "no Runner") || strings.Contains(output, "preview_only") {
		t.Fatalf("drifted instance-filtered %s was not blocked: stderr=%q stdout=%q err=%v", command, stderr, output, runErr)
	}
	assertAdmissionCLIRequestOrder(t, requests, previewPath, false)
}

func assertAdmissionCLIRequestOrder(t *testing.T, requests []recordedConversationRequest, previewPath string, wantPost bool) {
	t.Helper()
	want := []recordedConversationRequest{
		{method: http.MethodGet, path: clientInstanceSessionViewCandidatePath},
		{method: http.MethodGet, path: clientInstanceResourceViewCandidatePath},
		{method: http.MethodGet, path: deviceInventoryReadCandidateV2Path},
		{method: http.MethodGet, path: clientInstanceResourceViewCandidatePath},
	}
	if wantPost {
		want = append(want, recordedConversationRequest{method: http.MethodPost, path: previewPath})
	}
	if len(requests) != len(want) {
		t.Fatalf("Runner admission CLI requests=%#v want=%#v", requests, want)
	}
	for index := range want {
		if requests[index] != want[index] {
			t.Fatalf("Runner admission CLI request[%d]=%#v want=%#v", index, requests[index], want[index])
		}
	}
}
