package appserver

import (
	"context"
	"encoding/json"
	"fmt"
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

func TestRunnerExecutionBoundaryRequiresServerOwnedAuthorityAndRechecksLease(t *testing.T) {
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
	authority := devicefabricgate.RunnerAuthorityConfig{
		Enabled: true, AuthorityID: "runner-authority-1",
		Decision: devicefabricgate.Decision{Status: "accepted", AcceptanceID: "runner-authority-001", AcceptedAtUnixMS: 1},
	}
	sessions, err := newAuthenticatedSessionRoutesWithDeviceFabricActivationAndRunnerAuthority(
		nil, nil, ptrDeviceFabricRequest(activation), registryPath, "", &authority, leasePath,
	)
	if err != nil {
		t.Fatal(err)
	}
	transport := runnerTransportAdmissionObservation(t)
	command := runnerDispatchAdmissionIntent(t)
	request := deviceplacement.RunnerExecutionBoundaryPreviewRequest{
		Owner: ownerToPlacement(owner), ConversationID: "conversation-1", RunID: "run-1",
		AttemptID: "attempt-1", AttemptState: "accepted", Command: command,
		Transport: transport, ExpectedPayloadSHA256: transport.PayloadSHA256,
		Controls: deviceplacement.RunnerExecutionBoundaryControls{EffectState: "not_started"},
	}
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	path := fmt.Sprintf(runnerExecutionBoundaryPath, "conversation-1", "run-1")
	response := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		path, schedulerSelectionLeaseScope, "application/json", "", string(body))
	if response.Code != http.StatusOK {
		t.Fatalf("execution boundary status=%d body=%q", response.Code, response.Body.String())
	}
	var observation deviceplacement.RunnerExecutionBoundaryObservation
	if err := json.Unmarshal(response.Body.Bytes(), &observation); err != nil {
		t.Fatal(err)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("execution boundary observation invalid: %v", err)
	}
	if !observation.ExecutionBoundaryReady || !observation.ActivationAllowed ||
		!observation.RunnerAuthorityAccepted || observation.LeaseEpoch != entry.Grant.Epoch ||
		observation.Authority != (deviceplacement.RunnerExecutionBoundaryAuthority{}) {
		t.Fatalf("execution boundary observation=%#v", observation)
	}
	if strings.Contains(response.Body.String(), "fencing_token") || strings.Contains(response.Body.String(), "argv") ||
		strings.Contains(response.Body.String(), "workspace_ref") || strings.Contains(response.Body.String(), "payload_body") {
		t.Fatalf("execution boundary response leaked proof material: %q", response.Body.String())
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
		t.Fatalf("released execution boundary status=%d body=%q", response.Code, response.Body.String())
	}
	if err := json.Unmarshal(response.Body.Bytes(), &observation); err != nil {
		t.Fatal(err)
	}
	if observation.ExecutionBoundaryReady || !containsAdmissionReason(observation.RejectionReasons, "lease_inactive_at_evaluated_time") {
		t.Fatalf("released lease was admitted by execution boundary: %#v", observation)
	}
}

func TestRunnerExecutionBoundaryRouteRemainsClosedWithoutAuthorityConfig(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	state := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})
	leasePath := filepath.Join(filepath.Dir(registryPath), "leases.json")
	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{Status: "accepted", AcceptanceID: "p4-001", AcceptedAtUnixMS: 1}
	activation.Evidence.RunnerIsolation = true
	activation.Evidence.LeaseFencing = true
	activation.Evidence.CancellationAndUncertainWork = true
	activation.Evidence.VaultArtifactAuthorization = true
	activation.Evidence.AuditOutbox = true
	sessions, err := newAuthenticatedSessionRoutesWithDeviceFabricActivation(
		nil, nil, ptrDeviceFabricRequest(activation), registryPath, "", leasePath,
	)
	if err != nil {
		t.Fatal(err)
	}
	path := fmt.Sprintf(runnerExecutionBoundaryPath, "conversation-1", "run-1")
	response := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		path, schedulerSelectionLeaseScope, "application/json", "", `{}`)
	if response.Code != http.StatusNotFound {
		t.Fatalf("authority-free execution boundary status=%d body=%q", response.Code, response.Body.String())
	}
}

func TestRunnerExecutionBoundaryPreviewRequiresDurableOwnerRunWhenBackendBound(t *testing.T) {
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
	if _, replayed, err := leaseAdapter.Claim(context.Background(), executionlease.ClaimRequest{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		IdempotencyKey: "durable-run-boundary-claim-000001", RequestSHA256: executionlease.RequestDigest([]byte("claim")),
		IssuedAtMS: now - 1_000, TTLMS: 30_000,
	}, []executionlease.ClaimCandidate{{DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1, HeartbeatSequence: 1}}); err != nil || replayed {
		t.Fatalf("claim replayed=%v err=%v", replayed, err)
	}
	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{Status: "accepted", AcceptanceID: "p4-durable-run-boundary-001", AcceptedAtUnixMS: 1}
	activation.Evidence.RunnerIsolation = true
	activation.Evidence.LeaseFencing = true
	activation.Evidence.CancellationAndUncertainWork = true
	activation.Evidence.VaultArtifactAuthorization = true
	activation.Evidence.AuditOutbox = true
	authority := devicefabricgate.RunnerAuthorityConfig{
		Enabled: true, AuthorityID: "runner-authority-durable-run-boundary",
		Decision: devicefabricgate.Decision{Status: "accepted", AcceptanceID: "runner-authority-durable-run-boundary-001", AcceptedAtUnixMS: 1},
	}
	backend := &fakeConversationBackend{runPage: runmodel.OwnedRunPage{
		ConversationID: "conversation-1",
		Runs:           []runmodel.OwnedRunSummary{{RunID: "run-1", PromptID: "prompt-1", CreatedAtMS: 1, LatestSequence: 1, Status: "completed"}},
	}}
	route := newRunnerExecutionBoundaryRoutes(&runnerExecutionBoundaryConfig{
		Enabled: true, RegistryPath: leasePath,
		Now:        func(context.Context) (int64, error) { return int64(now), nil },
		Activation: activation, Authority: authority, Backend: backend,
	})
	transport := runnerTransportAdmissionObservation(t)
	request := deviceplacement.RunnerExecutionBoundaryPreviewRequest{
		Owner: ownerToPlacement(owner), ConversationID: "conversation-1", RunID: "run-1",
		AttemptID: "attempt-1", AttemptState: "accepted", Command: runnerDispatchAdmissionIntent(t),
		Transport: transport, ExpectedPayloadSHA256: transport.PayloadSHA256,
		Controls: deviceplacement.RunnerExecutionBoundaryControls{EffectState: "not_started"},
	}
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	path := fmt.Sprintf(runnerExecutionBoundaryPath, "conversation-1", "run-1")
	response := requestConversationAPI(t, authenticator.Handler(route), identity, http.MethodPost,
		path, schedulerSelectionLeaseScope, "application/json", "", string(body))
	if response.Code != http.StatusOK || backend.runCalls != 1 {
		t.Fatalf("durable Run boundary status=%d calls=%d body=%q", response.Code, backend.runCalls, response.Body.String())
	}

	backend.runPage.Runs = []runmodel.OwnedRunSummary{}
	response = requestConversationAPI(t, authenticator.Handler(route), identity, http.MethodPost,
		path, schedulerSelectionLeaseScope, "application/json", "", string(body))
	if response.Code != http.StatusNotFound || !strings.Contains(response.Body.String(), `"code":"not_found"`) {
		t.Fatalf("missing durable Run boundary status=%d body=%q", response.Code, response.Body.String())
	}
}
