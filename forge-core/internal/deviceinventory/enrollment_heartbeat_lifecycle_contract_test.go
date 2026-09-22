package deviceinventory

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"reflect"
	"testing"

	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceidentity"
)

type enrollmentHeartbeatLifecycleFixture struct {
	SchemaVersion  string                             `json:"schema_version"`
	EvaluationMode string                             `json:"evaluation_mode"`
	Notice         string                             `json:"notice"`
	Owner          lifecycleOwnerFixture              `json:"owner"`
	Device         lifecycleDeviceFixture             `json:"device"`
	Challenge      lifecycleChallengeFixture          `json:"challenge"`
	Proof          lifecycleProofFixture              `json:"proof"`
	Capabilities   lifecycleCapabilitiesFixture       `json:"capabilities"`
	Authority      lifecycleAuthorityFixture          `json:"authority"`
	Cases          []enrollmentHeartbeatLifecycleCase `json:"cases"`
}

type lifecycleOwnerFixture struct {
	Issuer   string `json:"issuer"`
	Subject  string `json:"subject"`
	TenantID string `json:"tenant_id"`
}

type lifecycleDeviceFixture struct {
	DeviceID         string `json:"device_id"`
	KeyID            string `json:"key_id"`
	PublicKeySHA256  string `json:"public_key_sha256"`
	ApprovalState    string `json:"approval_state"`
	CredentialState  string `json:"credential_state"`
	CordonState      string `json:"cordon_state"`
	ReservationState string `json:"reservation_state"`
}

type lifecycleChallengeFixture struct {
	ChallengeID     string `json:"challenge_id"`
	ChallengeSHA256 string `json:"challenge_sha256"`
	IssuedAtMS      uint64 `json:"issued_at_ms"`
	ExpiresAtMS     uint64 `json:"expires_at_ms"`
}

type lifecycleProofFixture struct {
	DeviceID        string                `json:"device_id"`
	KeyID           string                `json:"key_id"`
	PublicKeySHA256 string                `json:"public_key_sha256"`
	Owner           lifecycleOwnerFixture `json:"owner"`
	ChallengeID     string                `json:"challenge_id"`
	ChallengeSHA256 string                `json:"challenge_sha256"`
	ProofSHA256     string                `json:"proof_sha256"`
	IssuedAtMS      uint64                `json:"issued_at_ms"`
	ExpiresAtMS     uint64                `json:"expires_at_ms"`
}

type lifecycleCapabilitiesFixture struct {
	OS                    string                `json:"os"`
	Architecture          string                `json:"architecture"`
	CPUCores              uint32                `json:"cpu_cores"`
	AvailableCPUCores     uint32                `json:"available_cpu_cores"`
	MemoryBytes           uint64                `json:"memory_bytes"`
	AvailableMemoryBytes  uint64                `json:"available_memory_bytes"`
	StorageBytes          uint64                `json:"storage_bytes"`
	AvailableStorageBytes uint64                `json:"available_storage_bytes"`
	GPUs                  []lifecycleGPUFixture `json:"gpus"`
	Runtimes              []string              `json:"runtimes"`
}

type lifecycleGPUFixture struct {
	ID                   string `json:"id"`
	Vendor               string `json:"vendor"`
	MemoryBytes          uint64 `json:"memory_bytes"`
	AvailableMemoryBytes uint64 `json:"available_memory_bytes"`
}

type lifecycleAuthorityFixture struct {
	IdentityVerified       bool `json:"identity_verified"`
	ChallengeConsumed      bool `json:"challenge_consumed"`
	HeartbeatPersisted     bool `json:"heartbeat_persisted"`
	InventoryAuthoritative bool `json:"inventory_authoritative"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
}

type enrollmentHeartbeatLifecycleCase struct {
	Name                      string                    `json:"name"`
	ApprovalState             string                    `json:"approval_state"`
	CredentialState           string                    `json:"credential_state"`
	ChallengeConsumed         bool                      `json:"challenge_consumed"`
	ProofOwner                string                    `json:"proof_owner"`
	Heartbeat                 lifecycleHeartbeatFixture `json:"heartbeat"`
	ExpectedHeartbeatRevision uint64                    `json:"expected_heartbeat_revision"`
	ExpectedInventoryRevision uint64                    `json:"expected_inventory_revision"`
	ServerObservedAtMS        uint64                    `json:"server_observed_at_ms"`
	LeaseTTLMS                uint64                    `json:"lease_ttl_ms"`
	IdentityNowMS             uint64                    `json:"identity_now_ms"`
	EvaluatedAtMS             uint64                    `json:"evaluated_at_ms"`
	StaleAfterMS              uint64                    `json:"stale_after_ms"`
	Expected                  lifecycleExpectedFixture  `json:"expected"`
}

type lifecycleHeartbeatFixture struct {
	DeviceID   string `json:"device_id"`
	InstanceID string `json:"instance_id"`
	Generation uint64 `json:"generation"`
	Sequence   uint64 `json:"sequence"`
}

type lifecycleExpectedFixture struct {
	Accepted          bool   `json:"accepted"`
	Error             string `json:"error"`
	HeartbeatRevision uint64 `json:"heartbeat_revision"`
	InventoryRevision uint64 `json:"inventory_revision"`
	Generation        uint64 `json:"generation"`
	HeartbeatSequence uint64 `json:"heartbeat_sequence"`
	Status            string `json:"status"`
	Fresh             bool   `json:"fresh"`
	DeclaredEligible  bool   `json:"declared_eligible"`
}

func TestEnrollmentHeartbeatLifecycleContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_FIXTURE")
	if path == "" {
		t.Skip("FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture enrollmentHeartbeatLifecycleFixture
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode lifecycle fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("lifecycle fixture has trailing JSON: %v", err)
	}
	assertLifecycleFixtureEnvelope(t, fixture)

	owner := deviceidentity.Owner{Issuer: fixture.Owner.Issuer, Subject: fixture.Owner.Subject, TenantID: fixture.Owner.TenantID}
	var currentHeartbeat *deviceheartbeat.PersistedInstance
	var currentInventory *PersistedInventoryState
	for _, testCase := range fixture.Cases {
		t.Run(testCase.Name, func(t *testing.T) {
			proofOwner := owner
			if testCase.ProofOwner == "foreign" {
				proofOwner.Subject = "user-foreign"
			}
			proof := deviceidentity.Proof{
				DeviceID: fixture.Proof.DeviceID, KeyID: fixture.Proof.KeyID,
				PublicKeySHA256: fixture.Proof.PublicKeySHA256, Owner: proofOwner,
				ChallengeID: fixture.Proof.ChallengeID, ChallengeSHA256: fixture.Proof.ChallengeSHA256,
				ProofSHA256: fixture.Proof.ProofSHA256, IssuedAtMS: fixture.Proof.IssuedAtMS,
				ExpiresAtMS: fixture.Proof.ExpiresAtMS,
			}
			input := EnrollmentHeartbeatLifecycleInput{
				Owner: owner,
				Device: deviceidentity.DeviceBinding{
					DeviceID: fixture.Device.DeviceID, Owner: owner,
					KeyID: fixture.Device.KeyID, PublicKeySHA256: fixture.Device.PublicKeySHA256,
					ApprovalState: testCase.ApprovalState, CredentialState: testCase.CredentialState,
				},
				Challenge: deviceidentity.Challenge{
					ChallengeID: fixture.Challenge.ChallengeID, ChallengeSHA256: fixture.Challenge.ChallengeSHA256,
					IssuedAtMS: fixture.Challenge.IssuedAtMS, ExpiresAtMS: fixture.Challenge.ExpiresAtMS,
					Consumed: testCase.ChallengeConsumed,
				},
				Proof: proof,
				Heartbeat: deviceheartbeat.Heartbeat{
					DeviceID: testCase.Heartbeat.DeviceID, InstanceID: testCase.Heartbeat.InstanceID,
					Generation: testCase.Heartbeat.Generation, Sequence: testCase.Heartbeat.Sequence,
					Capabilities: lifecycleCapabilities(fixture.Capabilities),
				},
				CurrentHeartbeat: currentHeartbeat, ExpectedHeartbeatRevision: testCase.ExpectedHeartbeatRevision,
				CurrentInventory: currentInventory, ExpectedInventoryRevision: testCase.ExpectedInventoryRevision,
				CordonState: fixture.Device.CordonState, ReservationState: fixture.Device.ReservationState,
				ServerObservedAtMS: testCase.ServerObservedAtMS, LeaseTTLMS: testCase.LeaseTTLMS,
				IdentityNowMS: testCase.IdentityNowMS, EvaluatedAtMS: testCase.EvaluatedAtMS,
				StaleAfterMS: testCase.StaleAfterMS,
			}
			beforeHeartbeat, beforeInventory := currentHeartbeat, currentInventory
			result, err := ObserveEnrollmentHeartbeatLifecycle(input)
			if !testCase.Expected.Accepted {
				if err == nil || err.Error() != testCase.Expected.Error {
					t.Fatalf("error=%v want=%q", err, testCase.Expected.Error)
				}
				if !reflect.DeepEqual(result, EnrollmentHeartbeatLifecycleResult{}) {
					t.Fatalf("rejected lifecycle exposed partial result: %#v", result)
				}
				if currentHeartbeat != beforeHeartbeat || currentInventory != beforeInventory {
					t.Fatal("rejected lifecycle changed caller state")
				}
				return
			}
			if err != nil {
				t.Fatalf("lifecycle rejected: %v", err)
			}
			if result.Heartbeat.Revision != testCase.Expected.HeartbeatRevision ||
				result.Inventory.Revision != testCase.Expected.InventoryRevision ||
				result.Projection.Status != testCase.Expected.Status ||
				result.Projection.Fresh != testCase.Expected.Fresh ||
				result.Projection.DeclaredEligible != testCase.Expected.DeclaredEligible ||
				result.Heartbeat.Instance.Generation != testCase.Expected.Generation ||
				result.Heartbeat.Instance.HeartbeatSequence != testCase.Expected.HeartbeatSequence {
				t.Fatalf("lifecycle result=%#v expected=%#v", result, testCase.Expected)
			}
			if !result.PreviewOnly || result.Authority != (LifecycleAuthority{}) || !result.IdentityBound || result.IdentityReason != "bound_approved" {
				t.Fatalf("lifecycle authority/binding=%#v", result)
			}
			currentHeartbeat = &result.Heartbeat
			currentInventory = &result.Inventory
		})
	}
	if currentInventory == nil || currentHeartbeat == nil {
		t.Fatal("fixture did not produce a successful replacement")
	}
	if _, err := RestorePersistedInventory(currentInventory.Revision, currentInventory.Device, currentInventory.Runner); err != nil {
		t.Fatalf("restart restore rejected last value: %v", err)
	}
	if !reflect.DeepEqual(currentInventory.Device.Owner, SnapshotOwner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}) {
		t.Fatal("restart restore lost exact owner tuple")
	}
}

func assertLifecycleFixtureEnvelope(t *testing.T, fixture enrollmentHeartbeatLifecycleFixture) {
	t.Helper()
	if fixture.SchemaVersion != EnrollmentHeartbeatLifecycleSchemaVersion ||
		fixture.EvaluationMode != EnrollmentHeartbeatLifecycleEvaluationMode ||
		fixture.Notice == "" || fixture.Owner.TenantID != "tenant-1" ||
		fixture.Device.DeviceID != "device-a" || fixture.Device.KeyID != "key-a" ||
		fixture.Device.ApprovalState != "approved" || fixture.Device.CredentialState != "active" ||
		fixture.Device.CordonState != "clear" || fixture.Device.ReservationState != "none" ||
		fixture.Challenge.ExpiresAtMS <= fixture.Challenge.IssuedAtMS ||
		fixture.Proof.ExpiresAtMS <= fixture.Proof.IssuedAtMS ||
		fixture.Authority.IdentityVerified || fixture.Authority.ChallengeConsumed ||
		fixture.Authority.HeartbeatPersisted || fixture.Authority.InventoryAuthoritative ||
		fixture.Authority.ReservationCreated || fixture.Authority.ExecutionAuthorized ||
		fixture.Authority.DispatchPerformed || len(fixture.Cases) != 10 {
		t.Fatalf("invalid lifecycle fixture envelope: %#v", fixture)
	}
}

func lifecycleCapabilities(value lifecycleCapabilitiesFixture) deviceheartbeat.CapabilitySnapshot {
	gpus := make([]deviceheartbeat.GPUCapability, len(value.GPUs))
	for index, gpu := range value.GPUs {
		gpus[index] = deviceheartbeat.GPUCapability{ID: gpu.ID, Vendor: gpu.Vendor, MemoryBytes: gpu.MemoryBytes, AvailableMemoryBytes: gpu.AvailableMemoryBytes}
	}
	capabilities, err := deviceheartbeat.NewCapabilitySnapshot(
		value.OS, value.Architecture, value.CPUCores, value.AvailableCPUCores,
		value.MemoryBytes, value.AvailableMemoryBytes, value.StorageBytes, value.AvailableStorageBytes,
		gpus, value.Runtimes,
	)
	if err != nil {
		panic(err)
	}
	return capabilities
}
