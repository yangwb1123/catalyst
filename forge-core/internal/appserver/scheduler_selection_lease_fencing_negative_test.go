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
)

// TestSchedulerSelectionLeaseRejectsStaleReleaseAndIdempotencyConflict keeps
// the release boundary fenced after a renewal. A caller holding the previous
// epoch must not release the replacement lease, and reusing a release key for
// a different proof must fail closed instead of replaying a receipt.
func TestSchedulerSelectionLeaseRejectsStaleReleaseAndIdempotencyConflict(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	now := time.Now().UnixMilli()
	state := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)
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
	handler := authenticator.Handler(sessions)
	requirements := deviceplacement.Requirements{
		OS: "linux", Architecture: "amd64", MinCPUCores: 1, MinMemoryBytes: 1,
		MinStorageBytes: 1, Runtime: "oci", GPU: deviceplacement.GPURequirement{},
		DataResidencyZones: []string{"us-west"}, MinimumTrustZone: "standard",
		SandboxFloor: "container", ConcurrencySlots: 1,
	}
	claim := schedulerSelectionLeaseRequest{
		ConversationID: "conversation-fencing-negative", RunID: "run-fencing-negative",
		AttemptID: "attempt-fencing-negative", Requirements: requirements, TTLMS: 30_000,
	}
	claimBody, err := json.Marshal(claim)
	if err != nil {
		t.Fatal(err)
	}
	claimResponse := requestConversationAPI(t, handler, identity, http.MethodPost,
		schedulerSelectionLeasePath, schedulerSelectionLeaseScope, "application/json", "fencing-claim-key-000001", string(claimBody))
	if claimResponse.Code != http.StatusOK {
		t.Fatalf("claim status=%d body=%q", claimResponse.Code, claimResponse.Body.String())
	}
	var claimed schedulerSelectionLeaseResponse
	if err := json.Unmarshal(claimResponse.Body.Bytes(), &claimed); err != nil {
		t.Fatal(err)
	}
	renewal := schedulerSelectionLeaseRenewalRequest{
		ConversationID: claimed.ConversationID, RunID: claimed.RunID, AttemptID: claimed.AttemptID,
		TargetID: claimed.Grant.TargetID, Epoch: claimed.Grant.Epoch,
		FencingToken: claimed.Grant.FencingToken, TTLMS: 30_000,
	}
	renewalBody, err := json.Marshal(renewal)
	if err != nil {
		t.Fatal(err)
	}
	renewalResponse := requestConversationAPI(t, handler, identity, http.MethodPost,
		schedulerSelectionLeaseRenewalPath, schedulerSelectionLeaseScope, "application/json", "fencing-renew-key-000001", string(renewalBody))
	if renewalResponse.Code != http.StatusOK {
		t.Fatalf("renewal status=%d body=%q", renewalResponse.Code, renewalResponse.Body.String())
	}
	var renewed schedulerSelectionLeaseResponse
	if err := json.Unmarshal(renewalResponse.Body.Bytes(), &renewed); err != nil {
		t.Fatal(err)
	}
	if renewed.Grant.Epoch != claimed.Grant.Epoch+1 || renewed.Grant.FencingToken == claimed.Grant.FencingToken {
		t.Fatalf("renewal did not rotate proof: claimed=%#v renewed=%#v", claimed.Grant, renewed.Grant)
	}
	staleRelease := schedulerSelectionLeaseReleaseRequest{
		ConversationID: claimed.ConversationID, RunID: claimed.RunID, AttemptID: claimed.AttemptID,
		TargetID: claimed.Grant.TargetID, Epoch: claimed.Grant.Epoch, FencingToken: claimed.Grant.FencingToken,
	}
	staleBody, err := json.Marshal(staleRelease)
	if err != nil {
		t.Fatal(err)
	}
	staleResponse := requestConversationAPI(t, handler, identity, http.MethodPost,
		schedulerSelectionLeaseReleasePath, schedulerSelectionLeaseScope, "application/json", "fencing-stale-release-000001", string(staleBody))
	if staleResponse.Code != http.StatusConflict || !strings.Contains(staleResponse.Body.String(), `"code":"lease_stale"`) {
		t.Fatalf("stale release status=%d body=%q", staleResponse.Code, staleResponse.Body.String())
	}
	currentRelease := schedulerSelectionLeaseReleaseRequest{
		ConversationID: renewed.ConversationID, RunID: renewed.RunID, AttemptID: renewed.AttemptID,
		TargetID: renewed.Grant.TargetID, Epoch: renewed.Grant.Epoch, FencingToken: renewed.Grant.FencingToken,
	}
	currentBody, err := json.Marshal(currentRelease)
	if err != nil {
		t.Fatal(err)
	}
	const releaseKey = "fencing-release-key-000001"
	releaseResponse := requestConversationAPI(t, handler, identity, http.MethodPost,
		schedulerSelectionLeaseReleasePath, schedulerSelectionLeaseScope, "application/json", releaseKey, string(currentBody))
	if releaseResponse.Code != http.StatusOK {
		t.Fatalf("current release status=%d body=%q", releaseResponse.Code, releaseResponse.Body.String())
	}
	conflictingRelease := currentRelease
	conflictingRelease.TargetID = "runner-b"
	conflictingBody, err := json.Marshal(conflictingRelease)
	if err != nil {
		t.Fatal(err)
	}
	conflictResponse := requestConversationAPI(t, handler, identity, http.MethodPost,
		schedulerSelectionLeaseReleasePath, schedulerSelectionLeaseScope, "application/json", releaseKey, string(conflictingBody))
	if conflictResponse.Code != http.StatusConflict || !strings.Contains(conflictResponse.Body.String(), `"code":"idempotency_conflict"`) {
		t.Fatalf("release idempotency conflict status=%d body=%q", conflictResponse.Code, conflictResponse.Body.String())
	}
}
