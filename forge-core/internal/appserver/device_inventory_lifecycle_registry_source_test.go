package appserver

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"path/filepath"
	"testing"

	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"forgeos/forge-core/internal/statefs"
)

func TestPersistedLifecycleRegistryFileSetReadSourceProjectsAllInstancesThroughCandidates(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	first := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)
	second := lifecycleRegistrySourceState(t, owner, "device-b", "runner-b", 1)
	path := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{second, first})
	source := newPersistedLifecycleRegistryFileSetReadSource(path, 200_000)

	v1Handler := authenticator.Handler(newDeviceInventoryReadCandidateRoutes(&deviceInventoryReadCandidateConfig{
		Enabled: true, Source: source,
	}))
	v1Response := requestConversationAPI(t, v1Handler, identity, http.MethodGet, deviceInventoryReadCandidatePath,
		deviceInventoryReadCandidateScope, "", "", "")
	if v1Response.Code != http.StatusOK {
		t.Fatalf("registry v1 status=%d body=%q", v1Response.Code, v1Response.Body.String())
	}
	var v1 deviceplacement.SessionDeviceObservationInventory
	if err := json.Unmarshal(v1Response.Body.Bytes(), &v1); err != nil {
		t.Fatalf("decode registry v1 observation: %v", err)
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventory(v1); err != nil {
		t.Fatalf("validate registry v1 observation: %v", err)
	}
	if v1.Owner.Subject != owner.Subject || len(v1.Devices) != 2 ||
		v1.Devices[0].Device.DeviceID != "device-a" || v1.Devices[0].InstanceID != "runner-a" ||
		v1.Devices[1].Device.DeviceID != "device-b" || v1.Devices[1].InstanceID != "runner-b" ||
		v1.ExecutionAuthorized || v1.ReservationCreated || v1.DispatchPerformed {
		t.Fatalf("registry v1 observation=%#v", v1)
	}

	v2Handler := authenticator.Handler(newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
		Enabled: true, Source: source,
	}))
	v2Response := requestConversationAPI(t, v2Handler, identity, http.MethodGet, deviceInventoryReadCandidateV2Path,
		deviceInventoryReadCandidateScope, "", "", "")
	if v2Response.Code != http.StatusOK {
		t.Fatalf("registry v2 status=%d body=%q", v2Response.Code, v2Response.Body.String())
	}
	var v2 deviceplacement.SessionDeviceObservationInventoryV2
	if err := json.Unmarshal(v2Response.Body.Bytes(), &v2); err != nil {
		t.Fatalf("decode registry v2 observation: %v", err)
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventoryV2(v2); err != nil {
		t.Fatalf("validate registry v2 observation: %v", err)
	}
	if v2.Owner.Subject != owner.Subject || len(v2.Devices) != 2 ||
		v2.Devices[0].Revision != first.Revision || v2.Devices[0].HeartbeatSequence != 1 ||
		v2.Devices[1].Revision != second.Revision || v2.Devices[1].HeartbeatSequence != 1 ||
		v2.ExecutionAuthorized || v2.ReservationCreated || v2.DispatchPerformed {
		t.Fatalf("registry v2 observation=%#v", v2)
	}
}

func TestPersistedLifecycleRegistryFileSetReadSourceBindsOwnerAndContext(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "account-42", TenantID: "tenant-slate"}
	path := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{
		lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1),
	})
	source := newPersistedLifecycleRegistryFileSetReadSource(path, 200_000)

	_, err := source.ReadOwnedDeviceInventoryV2(context.Background(), model.Owner{
		Issuer: owner.Issuer, Subject: "account-foreign", TenantID: owner.TenantID,
	})
	if !errors.Is(err, deviceinventory.ErrPersistedLifecycleRegistryOwnerMismatch) {
		t.Fatalf("foreign owner error=%v, want %v", err, deviceinventory.ErrPersistedLifecycleRegistryOwnerMismatch)
	}

	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := source.ReadOwnedDeviceInventory(ctx, model.Owner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	}); !errors.Is(err, context.Canceled) {
		t.Fatalf("canceled context error=%v, want %v", err, context.Canceled)
	}
}

func writeLifecycleRegistrySourceFile(
	t *testing.T,
	owner deviceidentity.Owner,
	states []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "lifecycle-registry.json")
	writeLifecycleRegistrySourceFileAtPath(t, path, owner, states)
	return path
}

func writeLifecycleRegistrySourceFileAtPath(
	t *testing.T,
	path string,
	owner deviceidentity.Owner,
	states []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
) {
	t.Helper()
	envelope := struct {
		SchemaVersion string                                                       `json:"schema_version"`
		Owner         deviceidentity.Owner                                         `json:"owner"`
		States        []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState `json:"states"`
	}{
		SchemaVersion: "forge.device-enrollment-heartbeat-lifecycle-file-set/v1",
		Owner:         owner,
		States:        states,
	}
	data, err := json.Marshal(envelope)
	if err != nil {
		t.Fatal(err)
	}
	if err := statefs.AtomicWrite(path, append(data, '\n'), 0o600); err != nil {
		t.Fatal(err)
	}
}

func lifecycleRegistrySourceState(
	t *testing.T,
	owner deviceidentity.Owner,
	deviceID string,
	instanceID string,
	sequence uint64,
) deviceinventory.PersistedEnrollmentHeartbeatLifecycleState {
	t.Helper()
	keyID := "key-" + deviceID
	challengeID := "challenge-" + deviceID
	device := deviceidentity.DeviceBinding{
		DeviceID: deviceID, Owner: owner, KeyID: keyID,
		PublicKeySHA256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		ApprovalState:   "approved", CredentialState: "active",
	}
	challenge := deviceidentity.Challenge{
		ChallengeID:     challengeID,
		ChallengeSHA256: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
		IssuedAtMS:      100_000, ExpiresAtMS: 160_000,
	}
	proof := deviceidentity.Proof{
		DeviceID: deviceID, KeyID: keyID, PublicKeySHA256: device.PublicKeySHA256, Owner: owner,
		ChallengeID: challengeID, ChallengeSHA256: challenge.ChallengeSHA256,
		ProofSHA256: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
		IssuedAtMS:  110_000, ExpiresAtMS: 150_000,
	}
	capabilities, err := deviceheartbeat.NewCapabilitySnapshot(
		"linux", "amd64", 8, 7, 16<<30, 8<<30, 100<<30, 50<<30,
		nil, []string{"oci"},
	)
	if err != nil {
		t.Fatal(err)
	}
	input := deviceinventory.EnrollmentHeartbeatLifecycleInput{
		Owner: owner, Device: device, Challenge: challenge, Proof: proof,
		Heartbeat: deviceheartbeat.Heartbeat{
			DeviceID: deviceID, InstanceID: instanceID, Generation: 1,
			Sequence: sequence, Capabilities: capabilities,
		},
		CordonState: "clear", ReservationState: "none",
		ServerObservedAtMS: 100_000 + sequence*1_000, LeaseTTLMS: 60_000,
		IdentityNowMS: 120_000, EvaluatedAtMS: 120_000, StaleAfterMS: 60_000,
	}
	state, _, err := deviceinventory.CommitPersistedEnrollmentHeartbeatLifecycle(nil, 0, input)
	if err != nil {
		t.Fatalf("build lifecycle source state: %v", err)
	}
	return state
}
