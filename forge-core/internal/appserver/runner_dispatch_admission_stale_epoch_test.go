package appserver

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/executionlease"
)

// TestRunnerDispatchAdmissionRejectsStaleEpochAfterSchedulerRenewal proves
// that the metadata-only dispatch boundary consumes the same current lease
// proof as the scheduler. A proof from the previous epoch is rejected before
// admission, while the replacement proof can produce only a redacted preview.
func TestRunnerDispatchAdmissionRejectsStaleEpochAfterSchedulerRenewal(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	state := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})
	leasePath := filepath.Join(filepath.Dir(registryPath), "leases.json")
	leaseAdapter, err := deviceinventory.NewPersistedExecutionLeaseRegistryFileSetWriteAdapterWithTokenSource(
		leasePath, owner, func() (string, error) { return "token-b", nil },
	)
	if err != nil {
		t.Fatal(err)
	}
	now := uint64(time.Now().UnixMilli())
	entry, replayed, err := leaseAdapter.Claim(context.Background(), executionlease.ClaimRequest{
		ConversationID: "conversation-dispatch-fence", RunID: "run-dispatch-fence", AttemptID: "attempt-dispatch-fence",
		IdempotencyKey: "dispatch-fence-claim-000001", RequestSHA256: executionlease.RequestDigest([]byte("claim")),
		IssuedAtMS: now - 1_000, TTLMS: 30_000,
	}, []executionlease.ClaimCandidate{{
		DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1, HeartbeatSequence: 1,
	}})
	if err != nil || replayed {
		t.Fatalf("claim entry=%#v replayed=%v err=%v", entry, replayed, err)
	}
	// The token source deliberately returns the next epoch's token. The first
	// claim therefore receives token-b as well; the proof rotation is still
	// demonstrated by the epoch and the changed token source below.
	leaseAdapter, err = deviceinventory.NewPersistedExecutionLeaseRegistryFileSetWriteAdapterWithTokenSource(
		leasePath, owner, func() (string, error) { return "token-c", nil },
	)
	if err != nil {
		t.Fatal(err)
	}
	renewed, replayed, err := leaseAdapter.Renew(context.Background(), executionlease.RenewRequest{
		ConversationID: entry.ConversationID, RunID: entry.RunID, AttemptID: entry.AttemptID,
		Proof: entry.Grant.Proof(), IdempotencyKey: "dispatch-fence-renew-000001",
		RequestSHA256: executionlease.RequestDigest([]byte("renew")), IssuedAtMS: now, TTLMS: 30_000,
	})
	if err != nil || replayed || renewed.Grant.Epoch != entry.Grant.Epoch+1 || renewed.Grant.FencingToken == entry.Grant.FencingToken {
		t.Fatalf("renewed=%#v replayed=%v entry=%#v err=%v", renewed, replayed, entry, err)
	}

	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{Status: "accepted", AcceptanceID: "p4-dispatch-fence-001", AcceptedAtUnixMS: 1}
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
	handler := authenticator.Handler(sessions)
	path := "/api/v1/conversations/" + entry.ConversationID + "/runs/" + entry.RunID + "/runner-dispatch-admission/preview"
	post := func(request deviceplacement.RunnerDispatchAdmissionRequest) *httptest.ResponseRecorder {
		t.Helper()
		body, marshalErr := json.Marshal(request)
		if marshalErr != nil {
			t.Fatal(marshalErr)
		}
		return requestConversationAPI(t, handler, identity, http.MethodPost,
			path, schedulerSelectionLeaseScope, "application/json", "", string(body))
	}
	request := deviceplacement.RunnerDispatchAdmissionRequest{
		Owner: ownerToPlacement(owner), ConversationID: entry.ConversationID, RunID: entry.RunID,
		AttemptID: entry.AttemptID, AttemptState: "accepted", EvaluatedAtMS: 1,
		Command: deviceplacement.RunnerExecutionCommand{
			V: 1, CommandID: "command-dispatch-fence-1",
			LeaseProof: deviceplacement.RunnerExecutionLeaseProof{
				AttemptID: entry.AttemptID, TargetID: entry.Grant.TargetID,
				Epoch: entry.Grant.Epoch, FencingToken: entry.Grant.FencingToken,
			},
			IdempotencyKey: "run-dispatch-fence:attempt-dispatch-fence:command-dispatch-fence-1",
			WorkspaceRef:   "workspace-dispatch-fence", Argv: []string{"forge-task", "--prompt-ref", "prompt-dispatch-fence"},
			TimeoutMS: 5_000, MaxOutputBytes: 65_536,
		},
	}
	staleResponse := post(request)
	if staleResponse.Code != http.StatusConflict || !strings.Contains(staleResponse.Body.String(), `"code":"lease_stale"`) {
		t.Fatalf("stale dispatch admission status=%d body=%q", staleResponse.Code, staleResponse.Body.String())
	}

	request.Command.LeaseProof.Epoch = renewed.Grant.Epoch
	request.Command.LeaseProof.FencingToken = renewed.Grant.FencingToken
	currentResponse := post(request)
	if currentResponse.Code != http.StatusOK {
		t.Fatalf("current dispatch admission status=%d body=%q", currentResponse.Code, currentResponse.Body.String())
	}
	var observation deviceplacement.RunnerDispatchAdmissionObservation
	if err := json.Unmarshal(currentResponse.Body.Bytes(), &observation); err != nil {
		t.Fatal(err)
	}
	if err := observation.Validate(); err != nil || !observation.AdmissionReady || !observation.LeaseProofCurrent ||
		!observation.LeaseActive || observation.LeaseEpoch != renewed.Grant.Epoch ||
		observation.Authority != (deviceplacement.RunnerDispatchAdmissionAuthority{}) {
		t.Fatalf("current dispatch admission=%#v err=%v", observation, err)
	}
	if strings.Contains(currentResponse.Body.String(), renewed.Grant.FencingToken) || strings.Contains(currentResponse.Body.String(), "argv") {
		t.Fatalf("dispatch admission leaked fencing or command material: %q", currentResponse.Body.String())
	}
}
