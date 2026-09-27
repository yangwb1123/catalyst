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
	"forgeos/forge-core/internal/statefs"
)

func TestLifecycleHeartbeatCandidateDefaultsClosedAndProductionRemainsUnregistered(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	body := lifecycleHeartbeatCandidateBody(t, identity.issuer, "account-42", "tenant-slate", 1, 1, 0)
	request := func(handler http.Handler) int {
		return requestConversationAPI(t, handler, identity, http.MethodPost,
			lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
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

func TestLifecycleHeartbeatCandidateRequiresExplicitUnsignedFixtureOptIn(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	root := t.TempDir()
	if err := statefs.EnsurePrivateDir(root); err != nil {
		t.Fatal(err)
	}
	handler := authenticator.Handler(newLifecycleRegistryCandidateRoutes(&lifecycleRegistryCandidateConfig{
		Enabled: true,
		Store:   newPersistedLifecycleRegistryCandidateStore(filepath.Join(root, "lifecycle-registry.json")),
		Heartbeat: &lifecycleHeartbeatCandidateConfig{
			Enabled:      true,
			Now:          func(context.Context) (uint64, error) { return 120_000, nil },
			StaleAfterMS: 90_000,
		},
	}))
	response := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", lifecycleHeartbeatCandidateBody(t, identity.issuer, "account-42", "tenant-slate", 1, 1, 0))
	if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
		t.Fatalf("unsigned fixture without explicit opt-in status=%d body=%q", response.Code, response.Body.String())
	}
}

func TestLifecycleHeartbeatCandidateUsesServerClockAndPersistsCandidateImage(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	root := t.TempDir()
	if err := statefs.EnsurePrivateDir(root); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "lifecycle-registry.json")
	clockCalls := 0
	config := &lifecycleRegistryCandidateConfig{
		Enabled: true,
		Store:   newPersistedLifecycleRegistryCandidateStore(path),
		Heartbeat: &lifecycleHeartbeatCandidateConfig{
			Enabled:            true,
			AllowUnsignedProof: true,
			Now: func(_ context.Context) (uint64, error) {
				clockCalls++
				return 120_000, nil
			},
			StaleAfterMS: 90_000,
		},
	}
	handler := authenticator.Handler(newLifecycleRegistryCandidateRoutes(config))
	body := lifecycleHeartbeatCandidateBody(t, identity.issuer, "account-42", "tenant-slate", 1, 1, 0)
	response := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", body)
	if response.Code != http.StatusOK {
		t.Fatalf("heartbeat status=%d body=%q", response.Code, response.Body.String())
	}
	if clockCalls != 1 {
		t.Fatalf("clock calls=%d, want one server-owned observation", clockCalls)
	}
	var published lifecycleHeartbeatCandidateResponse
	if err := json.Unmarshal(response.Body.Bytes(), &published); err != nil {
		t.Fatalf("decode heartbeat response: %v", err)
	}
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	if published.SchemaVersion != deviceinventory.EnrollmentHeartbeatLifecycleSchemaVersion ||
		published.Owner != owner || published.DeviceID != "device-a" || published.Revision != 1 ||
		published.Heartbeat.Revision != 1 || published.Inventory.Revision != 1 ||
		published.Projection.Status != "online" || !published.PreviewOnly ||
		!published.CandidatePublished || published.Authority != (deviceinventory.LifecycleAuthority{}) {
		t.Fatalf("unexpected heartbeat response=%#v", published)
	}
	if info, err := os.Stat(path); err != nil || info.Mode().Perm() != 0o600 {
		t.Fatalf("candidate file info=%v, want mode 0600", err)
	}

	get := requestConversationAPI(t, handler, identity, http.MethodGet,
		lifecycleRegistryCandidatePath, lifecycleRegistryCandidateReadScope, "", "", "")
	if get.Code != http.StatusOK || !strings.Contains(get.Body.String(), `"device_id":"device-a"`) {
		t.Fatalf("lifecycle GET status=%d body=%q", get.Code, get.Body.String())
	}

	// Device supplied observation time is not part of the request contract; the
	// server-owned clock above is the only accepted time source.
	withCallerTime := body[:len(body)-1] + `,"server_observed_at_ms":120000}`
	invalid := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", withCallerTime)
	if invalid.Code != http.StatusBadRequest {
		t.Fatalf("caller clock field status=%d body=%q", invalid.Code, invalid.Body.String())
	}
	withCallerState := body[:len(body)-1] + `,"cordon_state":"cordoned"}`
	stateMutation := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", withCallerState)
	if stateMutation.Code != http.StatusBadRequest {
		t.Fatalf("caller server-state field status=%d body=%q", stateMutation.Code, stateMutation.Body.String())
	}
}

func TestLifecycleHeartbeatCandidateRejectsApprovalProofReplayAndScopeViolations(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	root := t.TempDir()
	if err := statefs.EnsurePrivateDir(root); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "lifecycle-registry.json")
	config := &lifecycleRegistryCandidateConfig{
		Enabled: true,
		Store:   newPersistedLifecycleRegistryCandidateStore(path),
		Heartbeat: &lifecycleHeartbeatCandidateConfig{
			Enabled:            true,
			AllowUnsignedProof: true,
			Now:                func(context.Context) (uint64, error) { return 120_000, nil },
			StaleAfterMS:       90_000,
		},
	}
	handler := authenticator.Handler(newLifecycleRegistryCandidateRoutes(config))

	wrongScope := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleRegistryCandidateWriteScope,
		"application/json", "", lifecycleHeartbeatCandidateBody(t, identity.issuer, "account-42", "tenant-slate", 1, 1, 0))
	if wrongScope.Code != http.StatusForbidden {
		t.Fatalf("wrong scope status=%d body=%q", wrongScope.Code, wrongScope.Body.String())
	}

	pending := lifecycleHeartbeatCandidateBody(t, identity.issuer, "account-42", "tenant-slate", 1, 1, 0)
	pending = strings.Replace(pending, `"approval_state":"approved"`, `"approval_state":"pending"`, 1)
	approval := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", pending)
	if approval.Code != http.StatusConflict || !strings.Contains(approval.Body.String(), `"code":"device_approval_required"`) {
		t.Fatalf("approval status=%d body=%q", approval.Code, approval.Body.String())
	}

	accepted := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", lifecycleHeartbeatCandidateBody(t, identity.issuer, "account-42", "tenant-slate", 1, 1, 0))
	if accepted.Code != http.StatusOK {
		t.Fatalf("initial heartbeat status=%d body=%q", accepted.Code, accepted.Body.String())
	}
	replay := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", lifecycleHeartbeatCandidateBody(t, identity.issuer, "account-42", "tenant-slate", 1, 1, 1))
	if replay.Code != http.StatusConflict || !strings.Contains(replay.Body.String(), `"code":"heartbeat_conflict"`) {
		t.Fatalf("replay status=%d body=%q", replay.Code, replay.Body.String())
	}
	foreign := lifecycleHeartbeatCandidateBody(t, identity.issuer, "account-42", "tenant-slate", 1, 2, 1)
	var foreignValue map[string]any
	if err := json.Unmarshal([]byte(foreign), &foreignValue); err != nil {
		t.Fatal(err)
	}
	foreignValue["proof"].(map[string]any)["owner"].(map[string]any)["subject"] = "account-foreign"
	foreignBytes, err := json.Marshal(foreignValue)
	if err != nil {
		t.Fatal(err)
	}
	foreign = string(foreignBytes)
	foreignResponse := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", foreign)
	if foreignResponse.Code != http.StatusForbidden || !strings.Contains(foreignResponse.Body.String(), `"code":"device_identity_rejected"`) {
		t.Fatalf("foreign proof status=%d body=%q", foreignResponse.Code, foreignResponse.Body.String())
	}
}

func lifecycleHeartbeatCandidateBody(t *testing.T, issuer, subject, tenant string, generation, sequence, expectedRevision uint64) string {
	t.Helper()
	value := map[string]any{
		"device": map[string]any{
			"device_id":         "device-a",
			"owner":             map[string]string{"issuer": issuer, "subject": subject, "tenant_id": tenant},
			"key_id":            "key-a",
			"public_key_sha256": strings.Repeat("a", 64),
			"approval_state":    "approved",
			"credential_state":  "active",
		},
		"challenge": map[string]any{
			"challenge_id": "challenge-a", "challenge_sha256": strings.Repeat("b", 64),
			"issued_at_ms": 100_000, "expires_at_ms": 160_000, "consumed": false,
		},
		"proof": map[string]any{
			"device_id": "device-a", "key_id": "key-a", "public_key_sha256": strings.Repeat("a", 64),
			"owner":        map[string]string{"issuer": issuer, "subject": subject, "tenant_id": tenant},
			"challenge_id": "challenge-a", "challenge_sha256": strings.Repeat("b", 64),
			"proof_sha256": strings.Repeat("c", 64), "issued_at_ms": 110_000, "expires_at_ms": 150_000,
		},
		"heartbeat": map[string]any{
			"device_id": "device-a", "instance_id": "runner-a", "generation": generation, "sequence": sequence,
			"capabilities": map[string]any{
				"os": "linux", "architecture": "amd64", "cpu_cores": 8, "available_cpu_cores": 7,
				"memory_bytes": 17_179_869_184, "available_memory_bytes": 8_589_934_592,
				"storage_bytes": 107_374_182_400, "available_storage_bytes": 53_687_091_200,
				"gpus":     []any{map[string]any{"id": "gpu-a", "vendor": "NVIDIA", "memory_bytes": 8_589_934_592, "available_memory_bytes": 4_294_967_296}},
				"runtimes": []string{"oci", "python"},
			},
		},
		"lease_ttl_ms": 60_000, "expected_device_revision": expectedRevision,
	}
	body, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return string(body)
}
