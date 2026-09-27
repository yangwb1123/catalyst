package appserver

import (
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
	"forgeos/forge-core/internal/statefs"
)

func TestSchedulerSelectionLeaseRemainsClosedWithoutExplicitRegistry(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{
		lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1),
	})
	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{Status: "accepted", AcceptanceID: "p4-001", AcceptedAtUnixMS: 1}
	activation.Evidence.RunnerIsolation = true
	activation.Evidence.LeaseFencing = true
	activation.Evidence.CancellationAndUncertainWork = true
	activation.Evidence.VaultArtifactAuthorization = true
	activation.Evidence.AuditOutbox = true
	sessions, err := newAuthenticatedSessionRoutesWithDeviceFabricActivation(
		nil, nil, ptrDeviceFabricRequest(activation), registryPath, "",
	)
	if err != nil {
		t.Fatal(err)
	}
	response := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		schedulerSelectionLeasePath, schedulerSelectionLeaseScope, "application/json", "lease-key-00000001",
		`{"conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","requirements":{"os":"linux","architecture":"amd64","min_cpu_cores":1,"min_memory_bytes":1,"min_storage_bytes":1,"runtime":"oci","gpu":{"required":false,"min_memory_bytes":0,"runtime":""},"data_residency_zones":["us-west"],"minimum_trust_zone":"standard","sandbox_floor":"container","concurrency_slots":1},"ttl_ms":30000}`)
	if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
		t.Fatalf("lease route unexpectedly mounted without registry: status=%d body=%q", response.Code, response.Body.String())
	}
	renewal := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		schedulerSelectionLeaseRenewalPath, schedulerSelectionLeaseScope, "application/json", "renew-key-00000001",
		`{"conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","target_id":"runner-a","epoch":1,"fencing_token":"token-a","ttl_ms":30000}`)
	if renewal.Code != http.StatusNotFound || renewal.Body.String() != string(notFoundBody) {
		t.Fatalf("lease renewal route unexpectedly mounted without registry: status=%d body=%q", renewal.Code, renewal.Body.String())
	}
	release := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		schedulerSelectionLeaseReleasePath, schedulerSelectionLeaseScope, "application/json", "release-key-00000001",
		`{"conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","target_id":"runner-a","epoch":1,"fencing_token":"token-a"}`)
	if release.Code != http.StatusNotFound || release.Body.String() != string(notFoundBody) {
		t.Fatalf("lease release route unexpectedly mounted without registry: status=%d body=%q", release.Code, release.Body.String())
	}
	admission := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		"/api/v1/conversations/conversation-1/runs/run-1/runner-dispatch-admission/preview", schedulerSelectionLeaseScope, "application/json", "",
		`{"owner":{"issuer":"`+identity.issuer+`","subject":"account-42","tenant_id":"tenant-slate"},"conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","attempt_state":"accepted","command":{"v":1,"command_id":"command-1","lease_proof":{"attempt_id":"attempt-1","target_id":"runner-a","epoch":1,"fencing_token":"token-a"},"idempotency_key":"run-1:attempt-1:command-1","workspace_ref":"workspace-1","argv":["forge-task"],"timeout_ms":5000,"max_output_bytes":65536},"evaluated_at_ms":300}`)
	if admission.Code != http.StatusNotFound || admission.Body.String() != string(notFoundBody) {
		t.Fatalf("Runner dispatch admission route unexpectedly mounted without registry: status=%d body=%q", admission.Code, admission.Body.String())
	}
}

func TestSchedulerSelectionLeaseMountedByExecuteRegistryAndFailsClosedOnUnknownPolicy(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{
		lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1),
	})
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
	body := `{"conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","requirements":{"os":"linux","architecture":"amd64","min_cpu_cores":1,"min_memory_bytes":1,"min_storage_bytes":1,"runtime":"oci","gpu":{"required":false,"min_memory_bytes":0,"runtime":""},"data_residency_zones":["us-west"],"minimum_trust_zone":"standard","sandbox_floor":"container","concurrency_slots":1},"ttl_ms":30000}`
	response := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		schedulerSelectionLeasePath, schedulerSelectionLeaseScope, "application/json", "lease-key-00000001", body)
	if response.Code != http.StatusConflict || !strings.Contains(response.Body.String(), `"code":"no_eligible_target"`) {
		t.Fatalf("lease should fail closed for policy-unknown inventory: status=%d body=%q", response.Code, response.Body.String())
	}
}

func TestSchedulerSelectionLeaseClaimsAndReplaysWithPolicyCompleteSource(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	state := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)
	now := time.Now().UnixMilli()
	state.Heartbeat.Instance.ServerObservedAtMS = uint64(now - 1_000)
	state.Heartbeat.Instance.CapabilityLeaseExpiresAtMS = uint64(now + 59_000)
	state.Inventory.Runner.ServerObservedAtMS = uint64(now - 1_000)
	state.Inventory.Runner.CapabilityLeaseExpiresAtMS = uint64(now + 59_000)
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})
	policyPath := writePlacementPolicySourceFile(t, owner, []deviceplacement.PlacementPolicy{{
		DeviceID: "device-a", InstanceID: "runner-a", Revision: state.Revision,
		Generation: state.Heartbeat.Instance.Generation, HeartbeatSequence: state.Heartbeat.Instance.HeartbeatSequence,
		DataResidencyZones: []string{"us-west"}, TrustZone: "standard", SandboxLevels: []string{"container"},
		ConcurrencyLimit: 4, ActiveConcurrency: 0,
	}})
	leasePath := filepath.Join(filepath.Dir(registryPath), "leases.json")
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
		nil, nil, ptrDeviceFabricRequest(activation), registryPath, "", nil, backend, leasePath, policyPath,
	)
	if err != nil {
		t.Fatal(err)
	}
	body := `{"conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","requirements":{"os":"linux","architecture":"amd64","min_cpu_cores":1,"min_memory_bytes":1,"min_storage_bytes":1,"runtime":"oci","gpu":{"required":false,"min_memory_bytes":0,"runtime":""},"data_residency_zones":["us-west"],"minimum_trust_zone":"standard","sandbox_floor":"container","concurrency_slots":1},"ttl_ms":30000}`
	previewBody := `{"conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","requirements":{"os":"linux","architecture":"amd64","min_cpu_cores":1,"min_memory_bytes":1,"min_storage_bytes":1,"runtime":"oci","gpu":{"required":false,"min_memory_bytes":0,"runtime":""},"data_residency_zones":["us-west"],"minimum_trust_zone":"standard","sandbox_floor":"container","concurrency_slots":1}}`
	previewResponse := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		schedulerSelectionPreviewPath, schedulerSelectionPreviewScope, "application/json", "", previewBody)
	if previewResponse.Code != http.StatusOK {
		t.Fatalf("policy-complete scheduler preview status=%d body=%q", previewResponse.Code, previewResponse.Body.String())
	}
	var preview deviceplacement.SchedulerSelectionPreviewObservation
	if err := json.Unmarshal(previewResponse.Body.Bytes(), &preview); err != nil {
		t.Fatalf("decode policy-complete scheduler preview: %v", err)
	}
	if err := preview.Validate(); err != nil || !preview.SelectionAvailable || preview.SelectionReason != "first_sorted_eligible_candidate" ||
		preview.SelectedDeviceID == nil || *preview.SelectedDeviceID != "device-a" ||
		preview.SelectedInstanceID == nil || *preview.SelectedInstanceID != "runner-a" ||
		preview.Authority != (deviceplacement.SchedulerSelectionPreviewAuthority{}) {
		t.Fatalf("policy-complete scheduler preview=%#v err=%v", preview, err)
	}
	reorderedBody := `{"ttl_ms":30000,"requirements":{"concurrency_slots":1,"sandbox_floor":"container","minimum_trust_zone":"standard","data_residency_zones":["us-west"],"gpu":{"runtime":"","min_memory_bytes":0,"required":false},"runtime":"oci","min_storage_bytes":1,"min_memory_bytes":1,"min_cpu_cores":1,"architecture":"amd64","os":"linux"},"attempt_id":"attempt-1","run_id":"run-1","conversation_id":"conversation-1"}`
	response := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		schedulerSelectionLeasePath, schedulerSelectionLeaseScope, "application/json", "lease-key-00000001", body)
	if response.Code != http.StatusOK {
		t.Fatalf("policy-complete lease status=%d body=%q", response.Code, response.Body.String())
	}
	var first schedulerSelectionLeaseResponse
	if err := json.Unmarshal(response.Body.Bytes(), &first); err != nil {
		t.Fatalf("decode first lease: %v", err)
	}
	if first.DeviceID != "device-a" || first.InstanceID != "runner-a" || first.Replayed ||
		!first.Authority.PlacementSelected || !first.Authority.ReservationCreated || !first.Authority.LeaseIssued ||
		first.Authority.ExecutionAuthorized || first.Authority.DispatchPerformed || first.Authority.AuditPublished {
		t.Fatalf("first lease=%#v", first)
	}
	response = requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		schedulerSelectionLeasePath, schedulerSelectionLeaseScope, "application/json", "lease-key-00000001", reorderedBody)
	if response.Code != http.StatusOK {
		t.Fatalf("lease replay status=%d body=%q", response.Code, response.Body.String())
	}
	var replay schedulerSelectionLeaseResponse
	if err := json.Unmarshal(response.Body.Bytes(), &replay); err != nil {
		t.Fatalf("decode replay lease: %v", err)
	}
	if !replay.Replayed || replay.Grant != first.Grant || replay.DeviceID != first.DeviceID {
		t.Fatalf("replay=%#v first=%#v", replay, first)
	}
	backend.runPage.Runs = []runmodel.OwnedRunSummary{}
	missingRunResponse := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		schedulerSelectionLeasePath, schedulerSelectionLeaseScope, "application/json", "lease-key-00000002", body)
	if missingRunResponse.Code != http.StatusNotFound || !strings.Contains(missingRunResponse.Body.String(), `"code":"not_found"`) {
		t.Fatalf("missing durable Run claim status=%d body=%q", missingRunResponse.Code, missingRunResponse.Body.String())
	}
}

func TestSchedulerSelectionLeaseRenewalClaimsAndReplaysWithCurrentProof(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	state := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)
	now := time.Now().UnixMilli()
	state.Heartbeat.Instance.ServerObservedAtMS = uint64(now - 1_000)
	state.Heartbeat.Instance.CapabilityLeaseExpiresAtMS = uint64(now + 59_000)
	state.Inventory.Runner.ServerObservedAtMS = uint64(now - 1_000)
	state.Inventory.Runner.CapabilityLeaseExpiresAtMS = uint64(now + 59_000)
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})
	policyPath := writePlacementPolicySourceFile(t, owner, []deviceplacement.PlacementPolicy{{
		DeviceID: "device-a", InstanceID: "runner-a", Revision: state.Revision,
		Generation: state.Heartbeat.Instance.Generation, HeartbeatSequence: state.Heartbeat.Instance.HeartbeatSequence,
		DataResidencyZones: []string{"us-west"}, TrustZone: "standard", SandboxLevels: []string{"container"},
		ConcurrencyLimit: 4, ActiveConcurrency: 0,
	}})
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
		nil, nil, ptrDeviceFabricRequest(activation), registryPath, "", leasePath, policyPath,
	)
	if err != nil {
		t.Fatal(err)
	}
	claimBody := `{"conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","requirements":{"os":"linux","architecture":"amd64","min_cpu_cores":1,"min_memory_bytes":1,"min_storage_bytes":1,"runtime":"oci","gpu":{"required":false,"min_memory_bytes":0,"runtime":""},"data_residency_zones":["us-west"],"minimum_trust_zone":"standard","sandbox_floor":"container","concurrency_slots":1},"ttl_ms":30000}`
	claimResponse := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		schedulerSelectionLeasePath, schedulerSelectionLeaseScope, "application/json", "lease-key-00000001", claimBody)
	if claimResponse.Code != http.StatusOK {
		t.Fatalf("claim status=%d body=%q", claimResponse.Code, claimResponse.Body.String())
	}
	var claim schedulerSelectionLeaseResponse
	if err := json.Unmarshal(claimResponse.Body.Bytes(), &claim); err != nil {
		t.Fatal(err)
	}
	renewal := schedulerSelectionLeaseRenewalRequest{
		ConversationID: claim.ConversationID, RunID: claim.RunID, AttemptID: claim.AttemptID,
		TargetID: claim.Grant.TargetID, Epoch: claim.Grant.Epoch, FencingToken: claim.Grant.FencingToken, TTLMS: 30_000,
	}
	renewalBody, err := json.Marshal(renewal)
	if err != nil {
		t.Fatal(err)
	}
	renewalResponse := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		schedulerSelectionLeaseRenewalPath, schedulerSelectionLeaseScope, "application/json", "renew-key-00000001", string(renewalBody))
	if renewalResponse.Code != http.StatusOK {
		t.Fatalf("renew status=%d body=%q", renewalResponse.Code, renewalResponse.Body.String())
	}
	var renewed schedulerSelectionLeaseResponse
	if err := json.Unmarshal(renewalResponse.Body.Bytes(), &renewed); err != nil {
		t.Fatal(err)
	}
	if renewed.DeviceID != claim.DeviceID || renewed.InstanceID != claim.InstanceID || renewed.Grant.Epoch != claim.Grant.Epoch+1 ||
		renewed.Grant.FencingToken == claim.Grant.FencingToken || renewed.Replayed || !renewed.Authority.LeaseIssued {
		t.Fatalf("renewed=%#v claim=%#v", renewed, claim)
	}
	replayResponse := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		schedulerSelectionLeaseRenewalPath, schedulerSelectionLeaseScope, "application/json", "renew-key-00000001", string(renewalBody))
	if replayResponse.Code != http.StatusOK {
		t.Fatalf("renew replay status=%d body=%q", replayResponse.Code, replayResponse.Body.String())
	}
	var replay schedulerSelectionLeaseResponse
	if err := json.Unmarshal(replayResponse.Body.Bytes(), &replay); err != nil {
		t.Fatal(err)
	}
	if !replay.Replayed || replay.Grant != renewed.Grant {
		t.Fatalf("renew replay=%#v renewed=%#v", replay, renewed)
	}
	staleBody := renewal
	staleBody.Epoch = claim.Grant.Epoch
	staleJSON, err := json.Marshal(staleBody)
	if err != nil {
		t.Fatal(err)
	}
	staleResponse := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		schedulerSelectionLeaseRenewalPath, schedulerSelectionLeaseScope, "application/json", "renew-key-00000002", string(staleJSON))
	if staleResponse.Code != http.StatusConflict || !strings.Contains(staleResponse.Body.String(), `"code":"lease_stale"`) {
		t.Fatalf("stale renewal status=%d body=%q", staleResponse.Code, staleResponse.Body.String())
	}
	release := schedulerSelectionLeaseReleaseRequest{
		ConversationID: renewed.ConversationID, RunID: renewed.RunID, AttemptID: renewed.AttemptID,
		TargetID: renewed.Grant.TargetID, Epoch: renewed.Grant.Epoch, FencingToken: renewed.Grant.FencingToken,
	}
	releaseBody, err := json.Marshal(release)
	if err != nil {
		t.Fatal(err)
	}
	releaseResponse := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		schedulerSelectionLeaseReleasePath, schedulerSelectionLeaseScope, "application/json", "release-key-00000001", string(releaseBody))
	if releaseResponse.Code != http.StatusOK {
		t.Fatalf("release status=%d body=%q", releaseResponse.Code, releaseResponse.Body.String())
	}
	var released schedulerSelectionLeaseReleaseResponse
	if err := json.Unmarshal(releaseResponse.Body.Bytes(), &released); err != nil {
		t.Fatal(err)
	}
	if released.EvaluationMode != executionlease.RegistryReleaseEvaluationMode || released.Replayed ||
		released.DeviceID != renewed.DeviceID || released.InstanceID != renewed.InstanceID || released.Epoch != renewed.Grant.Epoch ||
		released.ReleasedAtMS == 0 || released.Authority != (schedulerSelectionLeaseAuthority{}) {
		t.Fatalf("released=%#v", released)
	}
	replayRelease := requestConversationAPI(t, authenticator.Handler(sessions), identity, http.MethodPost,
		schedulerSelectionLeaseReleasePath, schedulerSelectionLeaseScope, "application/json", "release-key-00000001", string(releaseBody))
	if replayRelease.Code != http.StatusOK {
		t.Fatalf("release replay status=%d body=%q", replayRelease.Code, replayRelease.Body.String())
	}
	var releaseReplay schedulerSelectionLeaseReleaseResponse
	if err := json.Unmarshal(replayRelease.Body.Bytes(), &releaseReplay); err != nil {
		t.Fatal(err)
	}
	if !releaseReplay.Replayed || releaseReplay.ReleasedAtMS != released.ReleasedAtMS {
		t.Fatalf("release replay=%#v released=%#v", releaseReplay, released)
	}
}

func writePlacementPolicySourceFile(
	t *testing.T,
	owner deviceidentity.Owner,
	policies []deviceplacement.PlacementPolicy,
) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "placement-policy-registry.json")
	value := deviceplacement.PlacementPolicyRegistry{
		SchemaVersion:  deviceplacement.PlacementPolicyRegistrySchemaVersion,
		EvaluationMode: deviceplacement.PlacementPolicyRegistryEvaluationMode,
		Owner:          deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		Policies:       policies,
	}
	if err := deviceplacement.ValidatePlacementPolicyRegistry(value); err != nil {
		t.Fatal(err)
	}
	data, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	if err := statefs.AtomicWrite(path, append(data, '\n'), 0o600); err != nil {
		t.Fatal(err)
	}
	return path
}
