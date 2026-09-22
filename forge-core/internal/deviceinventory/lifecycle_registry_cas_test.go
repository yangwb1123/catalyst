package deviceinventory

import (
	"errors"
	"math"
	"reflect"
	"testing"

	"forgeos/forge-core/internal/devicecredential"
	"forgeos/forge-core/internal/deviceidentity"
)

func TestCommitPersistedEnrollmentHeartbeatLifecycleRegistryAddsUpdatesAndSorts(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	first := lifecycleRegistryState(t, "device-z", "runner-z")
	current := &PersistedEnrollmentHeartbeatLifecycleRegistryState{Owner: owner, States: []PersistedEnrollmentHeartbeatLifecycleState{first}}

	input := lifecyclePersistenceInput(t, 1, 2)
	input.Device.DeviceID = "device-z"
	input.Device.KeyID = "key-device-z"
	input.Proof.DeviceID = "device-z"
	input.Proof.KeyID = input.Device.KeyID
	input.Heartbeat.DeviceID = "device-z"
	input.Heartbeat.InstanceID = "runner-z"

	updated, result, err := CommitPersistedEnrollmentHeartbeatLifecycleRegistry(current, owner, 1, input)
	if err != nil {
		t.Fatalf("update registry: %v", err)
	}
	if result.Heartbeat.Instance.HeartbeatSequence != 2 || len(updated.States) != 1 || updated.States[0].Revision != 2 {
		t.Fatalf("updated=%#v result=%#v", updated, result)
	}
	if current.States[0].Revision != 1 {
		t.Fatal("registry CAS mutated caller state")
	}

	newInput := lifecyclePersistenceInput(t, 1, 1)
	newInput.Device.DeviceID = "device-a"
	newInput.Device.KeyID = "key-device-a"
	newInput.Proof.DeviceID = "device-a"
	newInput.Proof.KeyID = newInput.Device.KeyID
	newInput.Heartbeat.DeviceID = "device-a"
	newInput.Heartbeat.InstanceID = "runner-a"
	withNew, _, err := CommitPersistedEnrollmentHeartbeatLifecycleRegistry(&updated, owner, 0, newInput)
	if err != nil {
		t.Fatalf("add registry member: %v", err)
	}
	if len(withNew.States) != 2 || withNew.States[0].Device.DeviceID != "device-a" || withNew.States[1].Device.DeviceID != "device-z" {
		t.Fatalf("registry is not deterministic: %#v", withNew.States)
	}
}

func TestCommitPersistedEnrollmentHeartbeatLifecycleRegistryRejectsStaleAndPreservesImage(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	state := lifecycleRegistryState(t, "device-a", "runner-a")
	current := &PersistedEnrollmentHeartbeatLifecycleRegistryState{Owner: owner, States: []PersistedEnrollmentHeartbeatLifecycleState{state}}
	before := *current
	before.States = append([]PersistedEnrollmentHeartbeatLifecycleState(nil), current.States...)

	input := lifecyclePersistenceInput(t, 1, 2)
	input.Device.DeviceID = "device-a"
	input.Device.KeyID = "key-device-a"
	input.Proof.DeviceID = "device-a"
	input.Proof.KeyID = input.Device.KeyID
	input.Heartbeat.DeviceID = "device-a"
	input.Heartbeat.InstanceID = "runner-a"
	if _, _, err := CommitPersistedEnrollmentHeartbeatLifecycleRegistry(current, owner, 0, input); !errors.Is(err, ErrLifecycleRegistryRevisionConflict) {
		t.Fatalf("stale CAS error=%v, want %v", err, ErrLifecycleRegistryRevisionConflict)
	}
	if !reflect.DeepEqual(*current, before) {
		t.Fatal("stale CAS changed current image")
	}

	foreign := owner
	foreign.Subject = "other-user"
	if _, _, err := CommitPersistedEnrollmentHeartbeatLifecycleRegistry(current, foreign, 1, input); !errors.Is(err, ErrLifecycleRegistryOwnerMismatch) {
		t.Fatalf("foreign owner error=%v, want %v", err, ErrLifecycleRegistryOwnerMismatch)
	}
	if !reflect.DeepEqual(*current, before) {
		t.Fatal("owner rejection changed current image")
	}
}

func TestCommitPersistedEnrollmentHeartbeatLifecycleRegistryRejectsRunnerCollisionAndInvalidCurrent(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	first := lifecycleRegistryState(t, "device-a", "runner-a")
	second := lifecycleRegistryState(t, "device-b", "runner-b")
	current := &PersistedEnrollmentHeartbeatLifecycleRegistryState{Owner: owner, States: []PersistedEnrollmentHeartbeatLifecycleState{first, second}}

	input := lifecyclePersistenceInput(t, 1, 1)
	input.Device.DeviceID = "device-c"
	input.Device.KeyID = "key-device-c"
	input.Proof.DeviceID = "device-c"
	input.Proof.KeyID = input.Device.KeyID
	input.Heartbeat.DeviceID = "device-c"
	input.Heartbeat.InstanceID = "runner-a"
	if _, _, err := CommitPersistedEnrollmentHeartbeatLifecycleRegistry(current, owner, 0, input); !errors.Is(err, ErrLifecycleRegistryDuplicateRunner) {
		t.Fatalf("Runner collision error=%v, want %v", err, ErrLifecycleRegistryDuplicateRunner)
	}

	invalid := *current
	invalid.States = append([]PersistedEnrollmentHeartbeatLifecycleState(nil), current.States...)
	invalid.States[1].Device.DeviceID = invalid.States[0].Device.DeviceID
	if _, _, err := CommitPersistedEnrollmentHeartbeatLifecycleRegistry(&invalid, owner, 0, input); !errors.Is(err, ErrLifecycleRegistryInvalidState) && !errors.Is(err, ErrLifecycleRegistryDuplicateDevice) {
		t.Fatalf("invalid current error=%v", err)
	}
}

func TestCommitPersistedEnrollmentHeartbeatLifecycleRegistryRejectsCapacityAndOverflow(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	states := make([]PersistedEnrollmentHeartbeatLifecycleState, 0, maxPersistedLifecycleRegistryStates)
	for index := 0; index < maxPersistedLifecycleRegistryStates; index++ {
		state := lifecycleRegistryState(t, "device-"+string(rune('a'+index/26))+string(rune('a'+index%26)), "runner-"+string(rune('a'+index/26))+string(rune('a'+index%26)))
		states = append(states, state)
	}
	current := &PersistedEnrollmentHeartbeatLifecycleRegistryState{Owner: owner, States: states}
	input := lifecyclePersistenceInput(t, 1, 1)
	input.Device.DeviceID = "device-overflow"
	input.Device.KeyID = "key-device-overflow"
	input.Proof.DeviceID = "device-overflow"
	input.Proof.KeyID = input.Device.KeyID
	input.Heartbeat.DeviceID = "device-overflow"
	input.Heartbeat.InstanceID = "runner-overflow"
	if _, _, err := CommitPersistedEnrollmentHeartbeatLifecycleRegistry(current, owner, 0, input); !errors.Is(err, ErrLifecycleRegistryCapacityExceeded) {
		t.Fatalf("capacity error=%v, want %v", err, ErrLifecycleRegistryCapacityExceeded)
	}

	overflow := lifecycleRegistryState(t, "device-overflow", "runner-overflow")
	overflow.Revision = math.MaxUint64
	overflow.Heartbeat.Revision = math.MaxUint64
	overflow.Inventory.Revision = math.MaxUint64
	overflowCurrent := &PersistedEnrollmentHeartbeatLifecycleRegistryState{Owner: owner, States: []PersistedEnrollmentHeartbeatLifecycleState{overflow}}
	overflowInput := lifecyclePersistenceInput(t, 1, 2)
	overflowInput.Device.DeviceID = "device-overflow"
	overflowInput.Device.KeyID = "key-device-overflow"
	overflowInput.Proof.DeviceID = "device-overflow"
	overflowInput.Proof.KeyID = overflowInput.Device.KeyID
	overflowInput.Heartbeat.DeviceID = "device-overflow"
	overflowInput.Heartbeat.InstanceID = "runner-overflow"
	if _, _, err := CommitPersistedEnrollmentHeartbeatLifecycleRegistry(overflowCurrent, owner, math.MaxUint64, overflowInput); !errors.Is(err, ErrLifecycleRevisionOverflow) {
		t.Fatalf("overflow error=%v, want %v", err, ErrLifecycleRevisionOverflow)
	}
}

func TestCanonicalizePersistedEnrollmentHeartbeatLifecycleRegistryCopiesAndSorts(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	first := stateForOwner(t, owner, "device-z", "runner-z")
	second := stateForOwner(t, owner, "device-a", "runner-a")
	input := PersistedEnrollmentHeartbeatLifecycleRegistryState{Owner: owner, States: []PersistedEnrollmentHeartbeatLifecycleState{first, second}}
	canonical, err := CanonicalizePersistedEnrollmentHeartbeatLifecycleRegistry(input)
	if err != nil {
		t.Fatalf("canonicalize registry: %v", err)
	}
	if len(canonical.States) != 2 || canonical.States[0].Device.DeviceID != "device-a" || canonical.States[1].Device.DeviceID != "device-z" {
		t.Fatalf("canonical states=%#v", canonical.States)
	}
	canonical.States[0].Heartbeat.Instance.Capabilities.Runtimes[0] = "mutated"
	if input.States[1].Heartbeat.Instance.Capabilities.Runtimes[0] == "mutated" {
		t.Fatal("canonical registry shares capability slices with input")
	}
	if err := ValidatePersistedEnrollmentHeartbeatLifecycleRegistry(input); err != nil {
		t.Fatalf("validate canonical registry: %v", err)
	}
	if _, err := CanonicalizePersistedEnrollmentHeartbeatLifecycleRegistry(PersistedEnrollmentHeartbeatLifecycleRegistryState{}); !errors.Is(err, ErrLifecycleRegistryOwnerMismatch) {
		t.Fatalf("invalid empty owner error=%v, want %v", err, ErrLifecycleRegistryOwnerMismatch)
	}
}

func TestCanonicalizePersistedEnrollmentHeartbeatLifecycleRegistryRejectsInvalidCredentialCandidate(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	state := stateForOwner(t, owner, "device-a", "runner-a")
	state.CredentialCandidate = &devicecredential.State{
		CredentialID:    "credential-a",
		DeviceID:        state.Device.DeviceID,
		Owner:           owner,
		ApprovalState:   devicecredential.ApprovalApproved,
		CredentialState: devicecredential.CredentialActive,
		KeyID:           state.Device.KeyID,
		PublicKeySHA256: state.Device.PublicKeySHA256,
		KeyGeneration:   0,
		IssuedAtMS:      100_000,
		ExpiresAtMS:     101_000,
	}

	_, err := CanonicalizePersistedEnrollmentHeartbeatLifecycleRegistry(
		PersistedEnrollmentHeartbeatLifecycleRegistryState{Owner: owner, States: []PersistedEnrollmentHeartbeatLifecycleState{state}},
	)
	if !errors.Is(err, ErrLifecycleRegistryInvalidState) {
		t.Fatalf("invalid credential candidate error=%v, want %v", err, ErrLifecycleRegistryInvalidState)
	}
}
