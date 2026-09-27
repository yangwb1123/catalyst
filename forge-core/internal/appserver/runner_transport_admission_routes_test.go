package appserver

import (
	"context"
	"encoding/json"
	"net/http"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/executionlease"
	"forgeos/forge-core/internal/runnertransport"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
)

func TestRunnerTransportAdmissionRebindsVerifiedObservationToDurableLease(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	state := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})
	leasePath := filepath.Join(filepath.Dir(registryPath), "leases.json")
	leaseAdapter, err := deviceinventory.NewPersistedExecutionLeaseRegistryFileSetWriteAdapterWithTokenSource(
		leasePath, owner, func() (string, error) { return "token-a", nil },
	)
	if err != nil {
		t.Fatal(err)
	}
	now := uint64(time.Now().UnixMilli())
	entry, replayed, err := leaseAdapter.Claim(context.Background(), executionlease.ClaimRequest{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		IdempotencyKey: "lease-key-00000001", RequestSHA256: executionlease.RequestDigest([]byte("claim")),
		IssuedAtMS: now - 1_000, TTLMS: 30_000,
	}, []executionlease.ClaimCandidate{{DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1, HeartbeatSequence: 1}})
	if err != nil || replayed {
		t.Fatalf("claim entry=%#v replayed=%v err=%v", entry, replayed, err)
	}
	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{Status: "accepted", AcceptanceID: "p4-001", AcceptedAtUnixMS: 1}
	activation.Evidence.RunnerIsolation = true
	activation.Evidence.LeaseFencing = true
	activation.Evidence.CancellationAndUncertainWork = true
	activation.Evidence.VaultArtifactAuthorization = true
	activation.Evidence.AuditOutbox = true
	backend := &fakeConversationBackend{runPage: runmodel.OwnedRunPage{
		ConversationID: "conversation-1",
		Runs: []runmodel.OwnedRunSummary{{
			RunID: "run-1", PromptID: "prompt-1", CreatedAtMS: 1,
			LatestSequence: 1, Status: "completed",
		}},
	}}
	sessions, err := newAuthenticatedSessionRoutesWithDeviceFabricActivationAndRunnerAuthorityWithBackend(
		nil, nil, ptrDeviceFabricRequest(activation), registryPath, "", nil, backend, leasePath,
	)
	if err != nil {
		t.Fatal(err)
	}

	transport := runnerTransportAdmissionObservation(t)
	command := runnerDispatchAdmissionIntent(t)
	request := deviceplacement.RunnerTransportAdmissionRequest{
		Owner: ownerToPlacement(owner), ConversationID: "conversation-1", RunID: "run-1",
		AttemptID: "attempt-1", AttemptState: "accepted", Command: command,
		Lease: deviceplacement.RunnerDispatchAdmissionLease{
			TargetID: entry.InstanceID, Epoch: entry.Grant.Epoch,
			IssuedAtMS: entry.Grant.IssuedAtMS, ExpiresAtMS: entry.Grant.ExpiresAtMS,
			Current: true, Active: true,
		},
		Transport: transport, ExpectedPayloadSHA256: transport.PayloadSHA256, EvaluatedAtMS: 1,
	}
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-1/runs/run-1/runner-transport-admission/preview"
	response := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		path, schedulerSelectionLeaseScope, "application/json", "", string(body))
	if response.Code != http.StatusOK {
		t.Fatalf("transport admission status=%d body=%q", response.Code, response.Body.String())
	}
	var observation deviceplacement.RunnerTransportAdmissionObservation
	if err := json.Unmarshal(response.Body.Bytes(), &observation); err != nil {
		t.Fatal(err)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("transport admission observation invalid: %v", err)
	}
	if !observation.AdmissionReady || !observation.TransportBindingValid ||
		observation.LeaseEpoch != entry.Grant.Epoch || observation.TargetID != entry.InstanceID ||
		observation.Authority != (deviceplacement.RunnerTransportAdmissionAuthority{}) {
		t.Fatalf("transport admission observation=%#v", observation)
	}
	if strings.Contains(response.Body.String(), "fencing_token") ||
		strings.Contains(response.Body.String(), "argv") ||
		strings.Contains(response.Body.String(), "workspace_ref") {
		t.Fatalf("transport admission response leaked proof material: %q", response.Body.String())
	}
	backend.runPage.Runs = []runmodel.OwnedRunSummary{}
	response = requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		path, schedulerSelectionLeaseScope, "application/json", "", string(body))
	if response.Code != http.StatusNotFound || !strings.Contains(response.Body.String(), `"code":"not_found"`) {
		t.Fatalf("missing durable Run transport admission status=%d body=%q", response.Code, response.Body.String())
	}
}

func runnerTransportAdmissionObservation(t *testing.T) runnertransport.Observation {
	t.Helper()
	path := deviceplacement.TransportPayloadBindingPath("runner-a")
	payload := []byte(`{"attempt_id":"attempt-1","command_id":"command-1","target_id":"runner-a"}`)
	const timestamp = int64(1_700_000_000)
	const nonce = "transport-admission-route-1"
	// secret-scan:ignore — deterministic HMAC fixture, never used outside this test.
	const secret = "runner-transport-admission-secret"
	signature, err := runnertransport.Sign(secret, "POST", path, timestamp, nonce, payload)
	if err != nil {
		t.Fatal(err)
	}
	observation, err := runnertransport.Verify(secret, "POST", path, runnertransport.Envelope{
		TS: timestamp, Nonce: nonce, Sig: signature, Payload: json.RawMessage(payload),
	}, timestamp, runnertransport.NewReplayCache(4))
	if err != nil {
		t.Fatal(err)
	}
	return observation
}
