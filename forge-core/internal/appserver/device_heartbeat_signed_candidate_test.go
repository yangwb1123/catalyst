package appserver

import (
	"context"
	"crypto/ed25519"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"net/http"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"forgeos/forge-core/internal/statefs"
)

func TestLifecycleSignedHeartbeatCandidateVerifiesProofAndBindsHeartbeat(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	publicKey, privateKey, err := ed25519.GenerateKey(nil)
	if err != nil {
		t.Fatal(err)
	}
	keyDigest := sha256.Sum256(publicKey)
	keyDigestText := hex.EncodeToString(keyDigest[:])
	registryPath := filepath.Join(t.TempDir(), "lifecycle-registry.json")
	if err := statefs.EnsurePrivateDir(filepath.Dir(registryPath)); err != nil {
		t.Fatal(err)
	}
	config := &lifecycleRegistryCandidateConfig{
		Enabled: true,
		Store:   newPersistedLifecycleRegistryCandidateStore(registryPath),
		Heartbeat: &lifecycleHeartbeatCandidateConfig{
			Enabled:            true,
			AllowUnsignedProof: true,
			Now:                func(context.Context) (uint64, error) { return 120_000, nil },
			StaleAfterMS:       90_000,
		},
		Challenge: &lifecycleChallengeCandidateConfig{Enabled: true},
	}
	handler := authenticator.Handler(newLifecycleRegistryCandidateRoutes(config))
	seedBody := lifecycleHeartbeatCandidateBody(t, identity.issuer, owner.Subject, owner.TenantID, 1, 1, 0)
	seedBody = replaceHeartbeatSeedKeyDigest(t, seedBody, keyDigestText)
	seed := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", seedBody)
	if seed.Code != http.StatusOK {
		t.Fatalf("seed heartbeat status=%d body=%q", seed.Code, seed.Body.String())
	}
	secondSeed := replaceHeartbeatSeedIdentity(t, seedBody, "device-b", "runner-b")
	second := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", secondSeed)
	if second.Code != http.StatusOK {
		t.Fatalf("second seed heartbeat status=%d body=%q", second.Code, second.Body.String())
	}

	var seedValue map[string]any
	if err := json.Unmarshal([]byte(seedBody), &seedValue); err != nil {
		t.Fatal(err)
	}
	var heartbeat deviceheartbeat.Heartbeat
	heartbeatBytes, err := json.Marshal(seedValue["heartbeat"])
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(heartbeatBytes, &heartbeat); err != nil {
		t.Fatal(err)
	}
	heartbeat.Sequence = 2
	heartbeatDigest, err := heartbeat.Digest()
	if err != nil {
		t.Fatalf("heartbeat digest: %v", err)
	}
	challengeRequest, err := json.Marshal(map[string]any{
		"device_id": heartbeat.DeviceID, "heartbeat_sha256": heartbeatDigest,
		"ttl_ms": 60_000, "expected_device_revision": 1,
	})
	if err != nil {
		t.Fatal(err)
	}
	challengeResponse := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleChallengeCandidatePath, lifecycleChallengeCandidateScope,
		"application/json", "", string(challengeRequest))
	if challengeResponse.Code != http.StatusOK {
		t.Fatalf("challenge status=%d body=%q", challengeResponse.Code, challengeResponse.Body.String())
	}
	var issued lifecycleChallengeCandidateResponse
	if err := json.Unmarshal(challengeResponse.Body.Bytes(), &issued); err != nil {
		t.Fatal(err)
	}
	challenge := issued.Challenge
	proof := deviceidentity.SignedProof{
		DeviceID: heartbeat.DeviceID, KeyID: "key-a",
		PublicKeyBase64URL: base64.RawURLEncoding.EncodeToString(publicKey),
		Owner:              owner, ChallengeID: challenge.ChallengeID,
		ChallengeSHA256: challenge.ChallengeSHA256, IssuedAtMS: 110_000, ExpiresAtMS: 150_000,
	}
	signingBytes, err := proof.SigningBytes()
	if err != nil {
		t.Fatal(err)
	}
	proof.SignatureBase64URL = base64.RawURLEncoding.EncodeToString(ed25519.Sign(privateKey, signingBytes))
	body := signedHeartbeatCandidateBody(t, proof, challenge, heartbeat, heartbeatDigest, 1)

	badProof := proof
	badProof.SignatureBase64URL = base64.RawURLEncoding.EncodeToString(make([]byte, ed25519.SignatureSize))
	bad := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleSignedHeartbeatCandidatePath, lifecycleSignedHeartbeatCandidateScope,
		"application/json", "", signedHeartbeatCandidateBody(t, badProof, challenge, heartbeat, heartbeatDigest, 1))
	if bad.Code != http.StatusForbidden || !strings.Contains(bad.Body.String(), `"code":"device_identity_rejected"`) {
		t.Fatalf("bad signature status=%d body=%q", bad.Code, bad.Body.String())
	}

	accepted := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleSignedHeartbeatCandidatePath, lifecycleSignedHeartbeatCandidateScope,
		"application/json", "", body)
	if accepted.Code != http.StatusOK {
		t.Fatalf("signed heartbeat status=%d body=%q", accepted.Code, accepted.Body.String())
	}
	var response lifecycleSignedHeartbeatCandidateResponse
	if err := json.Unmarshal(accepted.Body.Bytes(), &response); err != nil {
		t.Fatal(err)
	}
	if response.Revision != 2 || response.Heartbeat.Instance.HeartbeatSequence != 2 ||
		!response.ProofVerified || !response.PreviewOnly || !response.CandidatePublished ||
		response.Authority != (deviceinventory.LifecycleAuthority{}) {
		t.Fatalf("unexpected signed heartbeat response=%#v", response)
	}
	registry := requestConversationAPI(t, handler, identity, http.MethodGet,
		lifecycleRegistryCandidatePath, lifecycleRegistryCandidateReadScope, "", "", "")
	if registry.Code != http.StatusOK || !strings.Contains(registry.Body.String(), `"device_id":"device-b"`) {
		t.Fatalf("signed heartbeat dropped another device status=%d body=%q", registry.Code, registry.Body.String())
	}

	replay := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleSignedHeartbeatCandidatePath, lifecycleSignedHeartbeatCandidateScope,
		"application/json", "", body)
	if replay.Code != http.StatusConflict || !strings.Contains(replay.Body.String(), `"code":"heartbeat_conflict"`) {
		t.Fatalf("signed heartbeat replay status=%d body=%q", replay.Code, replay.Body.String())
	}
	consumedBody := signedHeartbeatCandidateBody(t, proof, challenge, heartbeat, heartbeatDigest, 2)
	consumedReplay := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleSignedHeartbeatCandidatePath, lifecycleSignedHeartbeatCandidateScope,
		"application/json", "", consumedBody)
	if consumedReplay.Code != http.StatusForbidden || !strings.Contains(consumedReplay.Body.String(), `"code":"device_identity_rejected"`) {
		t.Fatalf("consumed challenge replay status=%d body=%q", consumedReplay.Code, consumedReplay.Body.String())
	}
	stored, err := newPersistedLifecycleRegistryCandidateStore(registryPath).ReadSnapshot(context.Background(), model.Owner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	})
	if err != nil {
		t.Fatal(err)
	}
	for _, state := range stored.States() {
		if state.Device.DeviceID == "device-a" && (state.ChallengeCandidate == nil || !state.ChallengeCandidate.Consumed) {
			t.Fatalf("signed heartbeat did not persist consumed challenge: %#v", state.ChallengeCandidate)
		}
	}

	var tampered map[string]any
	if err := json.Unmarshal([]byte(body), &tampered); err != nil {
		t.Fatal(err)
	}
	tamperedHeartbeat := tampered["heartbeat"].(map[string]any)
	tamperedHeartbeat["sequence"] = float64(3)
	tampered["expected_device_revision"] = float64(2)
	tamperedBody, err := json.Marshal(tampered)
	if err != nil {
		t.Fatal(err)
	}
	tamperedResponse := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleSignedHeartbeatCandidatePath, lifecycleSignedHeartbeatCandidateScope,
		"application/json", "", string(tamperedBody))
	if tamperedResponse.Code != http.StatusForbidden || !strings.Contains(tamperedResponse.Body.String(), `"code":"device_identity_rejected"`) {
		t.Fatalf("tampered heartbeat status=%d body=%q", tamperedResponse.Code, tamperedResponse.Body.String())
	}
}

func replaceHeartbeatSeedKeyDigest(t *testing.T, body, digest string) string {
	t.Helper()
	var value map[string]any
	if err := json.Unmarshal([]byte(body), &value); err != nil {
		t.Fatal(err)
	}
	value["device"].(map[string]any)["public_key_sha256"] = digest
	value["proof"].(map[string]any)["public_key_sha256"] = digest
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return string(encoded)
}

func replaceHeartbeatSeedIdentity(t *testing.T, body, deviceID, instanceID string) string {
	t.Helper()
	var value map[string]any
	if err := json.Unmarshal([]byte(body), &value); err != nil {
		t.Fatal(err)
	}
	value["device"].(map[string]any)["device_id"] = deviceID
	value["proof"].(map[string]any)["device_id"] = deviceID
	heartbeat := value["heartbeat"].(map[string]any)
	heartbeat["device_id"] = deviceID
	heartbeat["instance_id"] = instanceID
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return string(encoded)
}

func signedHeartbeatCandidateBody(
	t *testing.T,
	proof deviceidentity.SignedProof,
	challenge deviceidentity.Challenge,
	heartbeat deviceheartbeat.Heartbeat,
	heartbeatDigest string,
	expectedRevision uint64,
) string {
	t.Helper()
	body, err := json.Marshal(map[string]any{
		"device_id": heartbeat.DeviceID, "challenge": challenge, "signed_proof": proof,
		"heartbeat": heartbeat, "heartbeat_sha256": heartbeatDigest,
		"lease_ttl_ms": 60_000, "expected_device_revision": expectedRevision,
	})
	if err != nil {
		t.Fatal(err)
	}
	return string(body)
}
