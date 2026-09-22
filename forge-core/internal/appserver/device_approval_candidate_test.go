package appserver

import (
	"context"
	"encoding/json"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceapproval"
	"forgeos/forge-core/internal/deviceidentity"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"forgeos/forge-core/internal/statefs"
)

func TestLifecycleApprovalCandidateDefaultsClosedAndProductionRemainsUnregistered(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	body := lifecycleApprovalCandidateBody(t, "approve", 1, "", "")
	request := func(handler http.Handler) int {
		return requestConversationAPI(t, handler, identity, http.MethodPost,
			lifecycleApprovalCandidatePath, lifecycleApprovalCandidateScope,
			"application/json", "", body).Code
	}
	if status := request(authenticator.Handler(newLifecycleRegistryCandidateRoutes(nil))); status != http.StatusNotFound {
		t.Fatalf("disabled candidate status=%d, want 404", status)
	}
	sessions := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	if status := request(sessions); status != http.StatusNotFound {
		t.Fatalf("production route status=%d, want 404", status)
	}
}

func TestLifecycleApprovalCandidateBindsOwnerAndPublishesOnlyCandidatePlan(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	root := t.TempDir()
	if err := statefs.EnsurePrivateDir(root); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "lifecycle-registry.json")
	config := &lifecycleRegistryCandidateConfig{
		Enabled:  true,
		Store:    newPersistedLifecycleRegistryCandidateStore(path),
		Approval: &lifecycleApprovalCandidateConfig{Enabled: true},
	}
	// Seed one approved live image, then replace the initial image with a
	// pending value before exercising the owner approval transition.
	seedConfig := *config
	seedConfig.Heartbeat = &lifecycleHeartbeatCandidateConfig{
		Enabled:      true,
		Now:          func(context.Context) (uint64, error) { return 120_000, nil },
		StaleAfterMS: 90_000,
	}
	handler := authenticator.Handler(newLifecycleRegistryCandidateRoutes(&seedConfig))
	seed := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", lifecycleHeartbeatCandidateBody(t, identity.issuer, "account-42", "tenant-slate", 1, 1, 0))
	if seed.Code != http.StatusOK {
		t.Fatalf("seed heartbeat status=%d body=%q", seed.Code, seed.Body.String())
	}
	adapter := newPersistedLifecycleRegistryCandidateStore(path)
	snapshot, err := adapter.ReadSnapshot(context.Background(), modelOwnerForApprovalTest(identity))
	if err != nil {
		t.Fatalf("read seed: %v", err)
	}
	states := snapshot.States()
	if len(states) != 1 {
		t.Fatalf("seed states=%d, want one", len(states))
	}
	states[0].Device.ApprovalState = deviceapproval.ApprovalPending
	states[0].Inventory.Device.ApprovalState = deviceapproval.ApprovalPending
	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
	empty, err := adapter.ReadSnapshot(context.Background(), modelOwnerForApprovalTest(identity))
	if err != nil {
		t.Fatalf("read empty seed image: %v", err)
	}
	if _, err := adapter.ReplaceStatesIfUnchanged(context.Background(), modelOwnerForApprovalTest(identity), empty, states); err != nil {
		t.Fatalf("write pending image: %v", err)
	}

	wrongScope := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleApprovalCandidatePath, lifecycleRegistryCandidateWriteScope,
		"application/json", "", lifecycleApprovalCandidateBody(t, "approve", 1, "", ""))
	if wrongScope.Code != http.StatusForbidden {
		t.Fatalf("wrong scope status=%d body=%q", wrongScope.Code, wrongScope.Body.String())
	}
	unknown := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleApprovalCandidatePath, lifecycleApprovalCandidateScope,
		"application/json", "", strings.TrimSuffix(lifecycleApprovalCandidateBody(t, "approve", 1, "", ""), "}")+`,"authority":true}`)
	if unknown.Code != http.StatusBadRequest {
		t.Fatalf("unknown field status=%d body=%q", unknown.Code, unknown.Body.String())
	}

	approve := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleApprovalCandidatePath, lifecycleApprovalCandidateScope,
		"application/json", "", lifecycleApprovalCandidateBody(t, "approve", 1, "", ""))
	if approve.Code != http.StatusOK {
		t.Fatalf("approve status=%d body=%q", approve.Code, approve.Body.String())
	}
	var approved lifecycleApprovalCandidateResponse
	if err := json.Unmarshal(approve.Body.Bytes(), &approved); err != nil {
		t.Fatalf("decode approval response: %v", err)
	}
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	if approved.Owner != owner || approved.Previous.ApprovalState != deviceapproval.ApprovalPending ||
		approved.Next.ApprovalState != deviceapproval.ApprovalApproved || !approved.PreviewOnly ||
		!approved.CandidatePublished || approved.Authority != (deviceapproval.TransitionAuthority{}) {
		t.Fatalf("unexpected approval response=%#v", approved)
	}
	statesResponse := requestConversationAPI(t, handler, identity, http.MethodGet,
		lifecycleRegistryCandidatePath, lifecycleRegistryCandidateReadScope, "", "", "")
	if statesResponse.Code != http.StatusOK || strings.Contains(statesResponse.Body.String(), `"approval_candidate"`) {
		t.Fatalf("candidate registry response status=%d body=%q", statesResponse.Code, statesResponse.Body.String())
	}
	// The live lifecycle wire remains closed; the candidate plan is retained in
	// the injected image and is visible only through the approval response.
	stored, err := adapter.ReadSnapshot(context.Background(), modelOwnerForApprovalTest(identity))
	if err != nil {
		t.Fatalf("read published candidate: %v", err)
	}
	if len(stored.States()) != 1 || stored.States()[0].Device.ApprovalState != deviceapproval.ApprovalPending ||
		stored.States()[0].ApprovalCandidate == nil || stored.States()[0].ApprovalCandidate.ApprovalState != deviceapproval.ApprovalApproved {
		t.Fatalf("unexpected persisted candidate=%#v", stored.States())
	}
}

func TestLifecycleApprovalCandidateRotateAndRevokeAreStrictAndOwnerScoped(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	root := t.TempDir()
	if err := statefs.EnsurePrivateDir(root); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "lifecycle-registry.json")
	config := &lifecycleRegistryCandidateConfig{
		Enabled:  true,
		Store:    newPersistedLifecycleRegistryCandidateStore(path),
		Approval: &lifecycleApprovalCandidateConfig{Enabled: true},
		Heartbeat: &lifecycleHeartbeatCandidateConfig{
			Enabled:      true,
			Now:          func(context.Context) (uint64, error) { return 120_000, nil },
			StaleAfterMS: 90_000,
		},
	}
	handler := authenticator.Handler(newLifecycleRegistryCandidateRoutes(config))
	seed := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", lifecycleHeartbeatCandidateBody(t, identity.issuer, "account-42", "tenant-slate", 1, 1, 0))
	if seed.Code != http.StatusOK {
		t.Fatalf("seed heartbeat status=%d body=%q", seed.Code, seed.Body.String())
	}
	rotate := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleApprovalCandidatePath, lifecycleApprovalCandidateScope,
		"application/json", "", lifecycleApprovalCandidateBody(t, "rotate_key", 1, "key-b", strings.Repeat("d", 64)))
	if rotate.Code != http.StatusOK {
		t.Fatalf("rotate status=%d body=%q", rotate.Code, rotate.Body.String())
	}
	var rotated lifecycleApprovalCandidateResponse
	if err := json.Unmarshal(rotate.Body.Bytes(), &rotated); err != nil {
		t.Fatal(err)
	}
	if rotated.Next.KeyID != "key-b" || rotated.Next.KeyGeneration != 2 || rotated.Authority != (deviceapproval.TransitionAuthority{}) {
		t.Fatalf("unexpected rotation=%#v", rotated)
	}
	revoke := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleApprovalCandidatePath, lifecycleApprovalCandidateScope,
		"application/json", "", lifecycleApprovalCandidateBody(t, "revoke", 1, "", ""))
	if revoke.Code != http.StatusOK {
		t.Fatalf("revoke status=%d body=%q", revoke.Code, revoke.Body.String())
	}
	terminal := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleApprovalCandidatePath, lifecycleApprovalCandidateScope,
		"application/json", "", lifecycleApprovalCandidateBody(t, "rotate_key", 1, "key-c", strings.Repeat("e", 64)))
	if terminal.Code != http.StatusConflict || !strings.Contains(terminal.Body.String(), `"code":"approval_conflict"`) {
		t.Fatalf("terminal rotation status=%d body=%q", terminal.Code, terminal.Body.String())
	}
	stale := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleApprovalCandidatePath, lifecycleApprovalCandidateScope,
		"application/json", "", lifecycleApprovalCandidateBody(t, "revoke", 2, "", ""))
	if stale.Code != http.StatusConflict {
		t.Fatalf("stale revision status=%d body=%q", stale.Code, stale.Body.String())
	}
}

func lifecycleApprovalCandidateBody(t *testing.T, action string, revision uint64, keyID, digest string) string {
	t.Helper()
	value := map[string]any{
		"device_id":                "device-a",
		"action":                   action,
		"next_key_id":              keyID,
		"next_public_key_sha256":   digest,
		"expected_device_revision": revision,
	}
	body, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return string(body)
}

func modelOwnerForApprovalTest(identity *conversationTestIdentity) model.Owner {
	return model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
}
