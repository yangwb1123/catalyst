package appserver

import (
	"context"
	"encoding/json"
	"net/http"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceidentity"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"forgeos/forge-core/internal/statefs"
)

func TestLifecycleChallengeCandidateDefaultsClosedAndProductionRemainsUnregistered(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	body := lifecycleChallengeCandidateBody(t, "device-a", strings.Repeat("a", 64), 60_000, 1)
	request := func(handler http.Handler) int {
		return requestConversationAPI(t, handler, identity, http.MethodPost,
			lifecycleChallengeCandidatePath, lifecycleChallengeCandidateScope,
			"application/json", "", body).Code
	}
	if status := request(authenticator.Handler(newLifecycleRegistryCandidateRoutes(nil))); status != http.StatusNotFound {
		t.Fatalf("disabled candidate status=%d, want 404", status)
	}
	if status := request(authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))); status != http.StatusNotFound {
		t.Fatalf("production route status=%d, want 404", status)
	}
}

func TestLifecycleChallengeCandidateIssuesPersistsAndScopesOwner(t *testing.T) {
	identity, authenticator := newMultiPrincipalConversationTestIdentity(t)
	root := t.TempDir()
	if err := statefs.EnsurePrivateDir(root); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "lifecycle-registry.json")
	config := lifecycleChallengeCandidateTestConfig(path)
	handler := authenticator.Handler(newLifecycleRegistryCandidateRoutes(config))
	seed := requestConversationAPIAs(t, handler, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"account-42", "tenant-slate", "application/json", "", lifecycleHeartbeatCandidateBody(t, identity.issuer, "account-42", "tenant-slate", 1, 1, 0))
	if seed.Code != http.StatusOK {
		t.Fatalf("seed heartbeat status=%d body=%q", seed.Code, seed.Body.String())
	}
	body := lifecycleChallengeCandidateBody(t, "device-a", strings.Repeat("a", 64), 60_000, 1)
	issued := requestConversationAPIAs(t, handler, identity, http.MethodPost,
		lifecycleChallengeCandidatePath, lifecycleChallengeCandidateScope,
		"account-42", "tenant-slate", "application/json", "", body)
	if issued.Code != http.StatusOK {
		t.Fatalf("issue status=%d body=%q", issued.Code, issued.Body.String())
	}
	var response lifecycleChallengeCandidateResponse
	if err := json.Unmarshal(issued.Body.Bytes(), &response); err != nil {
		t.Fatal(err)
	}
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	if response.Owner != owner || response.DeviceID != "device-a" || response.Revision != 1 ||
		response.Challenge.ChallengeID == "" || response.Challenge.ChallengeSHA256 != strings.Repeat("a", 64) ||
		response.Challenge.Consumed || !response.PreviewOnly || !response.CandidatePublished ||
		response.Authority != (lifecycleChallengeCandidateResponse{}).Authority {
		t.Fatalf("unexpected challenge response=%#v", response)
	}
	stored, err := newPersistedLifecycleRegistryCandidateStore(path).ReadSnapshot(context.Background(), model.Owner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(stored.States()) != 1 || stored.States()[0].ChallengeCandidate == nil ||
		*stored.States()[0].ChallengeCandidate != response.Challenge {
		t.Fatalf("stored challenge=%#v", stored.States())
	}
	injected, err := json.Marshal(map[string]any{"states": stored.States()})
	if err != nil {
		t.Fatal(err)
	}
	registryPut := requestConversationAPIAs(t, handler, identity, http.MethodPut,
		lifecycleRegistryCandidatePath, lifecycleRegistryCandidateWriteScope,
		"account-42", "tenant-slate", "application/json", "", string(injected))
	if registryPut.Code != http.StatusBadRequest || !strings.Contains(registryPut.Body.String(), `"code":"invalid_request"`) {
		t.Fatalf("registry challenge injection status=%d body=%q", registryPut.Code, registryPut.Body.String())
	}
	active := requestConversationAPIAs(t, handler, identity, http.MethodPost,
		lifecycleChallengeCandidatePath, lifecycleChallengeCandidateScope,
		"account-42", "tenant-slate", "application/json", "", body)
	if active.Code != http.StatusConflict || !strings.Contains(active.Body.String(), `"code":"challenge_conflict"`) {
		t.Fatalf("active challenge status=%d body=%q", active.Code, active.Body.String())
	}
	foreign := requestConversationAPIAs(t, handler, identity, http.MethodPost,
		lifecycleChallengeCandidatePath, lifecycleChallengeCandidateScope,
		"account-foreign", "tenant-slate", "application/json", "", body)
	if foreign.Code == http.StatusOK {
		t.Fatalf("foreign owner unexpectedly issued challenge: %q", foreign.Body.String())
	}
	// Reconstructing the candidate handler from the same path observes the
	// persisted active challenge; it cannot issue a second live nonce.
	restarted := authenticator.Handler(newLifecycleRegistryCandidateRoutes(lifecycleChallengeCandidateTestConfig(path)))
	replayedIssue := requestConversationAPIAs(t, restarted, identity, http.MethodPost,
		lifecycleChallengeCandidatePath, lifecycleChallengeCandidateScope,
		"account-42", "tenant-slate", "application/json", "", body)
	if replayedIssue.Code != http.StatusConflict {
		t.Fatalf("restart active challenge status=%d body=%q", replayedIssue.Code, replayedIssue.Body.String())
	}
}

func lifecycleChallengeCandidateTestConfig(path string) *lifecycleRegistryCandidateConfig {
	return &lifecycleRegistryCandidateConfig{
		Enabled: true, Store: newPersistedLifecycleRegistryCandidateStore(path),
		Heartbeat: &lifecycleHeartbeatCandidateConfig{
			Enabled: true, AllowUnsignedProof: true,
			Now: func(context.Context) (uint64, error) { return 120_000, nil }, StaleAfterMS: 90_000,
		},
		Challenge: &lifecycleChallengeCandidateConfig{Enabled: true, Random: func(value []byte) error {
			for index := range value {
				value[index] = byte(index + 1)
			}
			return nil
		}},
	}
}

func lifecycleChallengeCandidateBody(t *testing.T, deviceID, digest string, ttlMS, revision uint64) string {
	t.Helper()
	body, err := json.Marshal(map[string]any{
		"device_id": deviceID, "heartbeat_sha256": digest,
		"ttl_ms": ttlMS, "expected_device_revision": revision,
	})
	if err != nil {
		t.Fatal(err)
	}
	return string(body)
}
