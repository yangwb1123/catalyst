package appserver

import (
	"context"
	"encoding/json"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/executionlease"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
)

func TestSchedulerSelectionLeaseRenewalRequiresRunButReleaseSurvivesRunDeletion(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	root := t.TempDir()
	if err := os.Chmod(root, 0o700); err != nil {
		t.Fatal(err)
	}
	leasePath := filepath.Join(root, "leases.json")
	registry, err := deviceinventory.NewPersistedExecutionLeaseRegistryFileSetWriteAdapterWithTokenSource(
		leasePath, owner, func() (string, error) { return "claim-token", nil },
	)
	if err != nil {
		t.Fatal(err)
	}
	claimed, _, err := registry.Claim(context.Background(), executionlease.ClaimRequest{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		IdempotencyKey: "claim-key-000001", RequestSHA256: executionlease.RequestDigest([]byte("claim")),
		IssuedAtMS: 1_000, TTLMS: 30_000,
	}, []executionlease.ClaimCandidate{{
		DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1, HeartbeatSequence: 1,
	}})
	if err != nil {
		t.Fatal(err)
	}

	backend := &fakeConversationBackend{runPage: runmodel.OwnedRunPage{
		ConversationID: "conversation-1",
		Runs: []runmodel.OwnedRunSummary{{
			RunID: "run-1", PromptID: "prompt-1", CreatedAtMS: 1, LatestSequence: 1, Status: "completed",
		}},
	}}
	clockValue := int64(2_000)
	now := func(context.Context) (int64, error) {
		value := clockValue
		clockValue += 1_000
		return value, nil
	}
	renewalRoute := newSchedulerSelectionLeaseRenewalRoutes(&schedulerSelectionLeaseConfig{
		Enabled: true, Now: now, RegistryPath: leasePath, Backend: backend,
	})
	renewal := schedulerSelectionLeaseRenewalRequest{
		ConversationID: claimed.ConversationID, RunID: claimed.RunID, AttemptID: claimed.AttemptID,
		TargetID: claimed.Grant.TargetID, Epoch: claimed.Grant.Epoch,
		FencingToken: claimed.Grant.FencingToken, TTLMS: 30_000,
	}
	renewalBody, err := json.Marshal(renewal)
	if err != nil {
		t.Fatal(err)
	}
	response := requestConversationAPI(t, authenticator.Handler(renewalRoute), identity, http.MethodPost,
		schedulerSelectionLeaseRenewalPath, schedulerSelectionLeaseScope, "application/json",
		"renew-key-000001", string(renewalBody))
	if response.Code != http.StatusOK {
		t.Fatalf("renewal with durable Run status=%d body=%q", response.Code, response.Body.String())
	}
	var renewed schedulerSelectionLeaseResponse
	if err := json.Unmarshal(response.Body.Bytes(), &renewed); err != nil {
		t.Fatal(err)
	}
	if renewed.Grant.Epoch != claimed.Grant.Epoch+1 || renewed.Replayed {
		t.Fatalf("renewed=%#v claimed=%#v", renewed, claimed)
	}

	backend.runPage.Runs = []runmodel.OwnedRunSummary{}
	renewal.Epoch = renewed.Grant.Epoch
	renewal.FencingToken = renewed.Grant.FencingToken
	renewalBody, err = json.Marshal(renewal)
	if err != nil {
		t.Fatal(err)
	}
	deletedRunResponse := requestConversationAPI(t, authenticator.Handler(renewalRoute), identity, http.MethodPost,
		schedulerSelectionLeaseRenewalPath, schedulerSelectionLeaseScope, "application/json",
		"renew-key-000002", string(renewalBody))
	if deletedRunResponse.Code != http.StatusNotFound ||
		!strings.Contains(deletedRunResponse.Body.String(), `"code":"not_found"`) {
		t.Fatalf("renewal after Run deletion status=%d body=%q", deletedRunResponse.Code, deletedRunResponse.Body.String())
	}

	releaseRoute := newSchedulerSelectionLeaseReleaseRoutes(&schedulerSelectionLeaseConfig{
		Enabled: true, Now: now, RegistryPath: leasePath,
	})
	release := schedulerSelectionLeaseReleaseRequest{
		ConversationID: renewed.ConversationID, RunID: renewed.RunID, AttemptID: renewed.AttemptID,
		TargetID: renewed.Grant.TargetID, Epoch: renewed.Grant.Epoch, FencingToken: renewed.Grant.FencingToken,
	}
	releaseBody, err := json.Marshal(release)
	if err != nil {
		t.Fatal(err)
	}
	releaseResponse := requestConversationAPI(t, authenticator.Handler(releaseRoute), identity, http.MethodPost,
		schedulerSelectionLeaseReleasePath, schedulerSelectionLeaseScope, "application/json",
		"release-key-000001", string(releaseBody))
	if releaseResponse.Code != http.StatusOK {
		t.Fatalf("release after Run deletion status=%d body=%q", releaseResponse.Code, releaseResponse.Body.String())
	}
}
