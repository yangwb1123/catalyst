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
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
)

func TestRunnerDispatchAdmissionRechecksDurableLeaseAndStaysReadOnly(t *testing.T) {
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
	// Reuse the package's bounded command constructor without carrying its
	// prompt/run wrapper into this metadata-only admission request.
	localIntent := runnerDispatchAdmissionIntent(t)
	request := deviceplacement.RunnerDispatchAdmissionRequest{
		Owner: ownerToPlacement(owner), ConversationID: "conversation-1", RunID: "run-1",
		AttemptID: "attempt-1", AttemptState: "accepted", Command: localIntent,
		EvaluatedAtMS: 1,
	}
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-1/runs/run-1/runner-dispatch-admission/preview"
	response := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		path, schedulerSelectionLeaseScope, "application/json", "", string(body))
	if response.Code != http.StatusOK {
		t.Fatalf("admission status=%d body=%q", response.Code, response.Body.String())
	}
	var observation deviceplacement.RunnerDispatchAdmissionObservation
	if err := json.Unmarshal(response.Body.Bytes(), &observation); err != nil {
		t.Fatal(err)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("admission observation invalid: %v", err)
	}
	if !observation.AdmissionReady || observation.LeaseEpoch != entry.Grant.Epoch ||
		observation.TargetID != entry.InstanceID || observation.Authority != (deviceplacement.RunnerDispatchAdmissionAuthority{}) {
		t.Fatalf("admission observation=%#v", observation)
	}
	if strings.Contains(response.Body.String(), "fencing_token") || strings.Contains(response.Body.String(), "argv") {
		t.Fatalf("admission response leaked fencing or command material: %q", response.Body.String())
	}

	releaseBody, err := json.Marshal(schedulerSelectionLeaseReleaseRequest{
		ConversationID: entry.ConversationID, RunID: entry.RunID, AttemptID: entry.AttemptID,
		TargetID: entry.Grant.TargetID, Epoch: entry.Grant.Epoch, FencingToken: entry.Grant.FencingToken,
	})
	if err != nil {
		t.Fatal(err)
	}
	release := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		schedulerSelectionLeaseReleasePath, schedulerSelectionLeaseScope, "application/json", "release-key-00000001", string(releaseBody))
	if release.Code != http.StatusOK {
		t.Fatalf("release status=%d body=%q", release.Code, release.Body.String())
	}
	response = requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		path, schedulerSelectionLeaseScope, "application/json", "", string(body))
	if response.Code != http.StatusOK {
		t.Fatalf("released admission status=%d body=%q", response.Code, response.Body.String())
	}
	if err := json.Unmarshal(response.Body.Bytes(), &observation); err != nil {
		t.Fatal(err)
	}
	if observation.AdmissionReady || !containsAdmissionReason(observation.RejectionReasons, "lease_inactive_at_evaluated_time") {
		t.Fatalf("released lease was admitted: %#v", observation)
	}
	backend.runPage.Runs = []runmodel.OwnedRunSummary{}
	response = requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		path, schedulerSelectionLeaseScope, "application/json", "", string(body))
	if response.Code != http.StatusNotFound || !strings.Contains(response.Body.String(), `"code":"not_found"`) {
		t.Fatalf("missing durable Run dispatch admission status=%d body=%q", response.Code, response.Body.String())
	}
}

func ownerToPlacement(owner deviceidentity.Owner) deviceplacement.Owner {
	return deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
}

func runnerDispatchAdmissionIntent(t *testing.T) deviceplacement.RunnerExecutionCommand {
	t.Helper()
	return deviceplacement.RunnerExecutionCommand{
		V:         1,
		CommandID: "command-1",
		LeaseProof: deviceplacement.RunnerExecutionLeaseProof{
			AttemptID: "attempt-1", TargetID: "runner-a", Epoch: 1, FencingToken: "token-a",
		},
		IdempotencyKey: "run-1:attempt-1:command-1", WorkspaceRef: "workspace-1",
		Argv: []string{"forge-task", "--prompt-ref", "prompt-1"}, TimeoutMS: 5_000, MaxOutputBytes: 65_536,
	}
}

func containsAdmissionReason(values []string, want string) bool {
	for _, value := range values {
		if value == want {
			return true
		}
	}
	return false
}
