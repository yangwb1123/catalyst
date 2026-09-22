package deviceinventory

import (
	"math"
	"reflect"
	"testing"

	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceidentity"
)

func TestPersistedEnrollmentHeartbeatLifecycleCommitsAndRestoresOneImage(t *testing.T) {
	input := lifecyclePersistenceInput(t, 1, 1)
	first, firstResult, err := CommitPersistedEnrollmentHeartbeatLifecycle(nil, 0, input)
	if err != nil {
		t.Fatalf("first lifecycle commit: %v", err)
	}
	if first.Revision != 1 || first.Heartbeat.Revision != 1 || first.Inventory.Revision != 1 ||
		firstResult.Heartbeat.Revision != 1 || firstResult.Inventory.Revision != 1 {
		t.Fatalf("first image=%#v result=%#v", first, firstResult)
	}

	restored, err := RestorePersistedEnrollmentHeartbeatLifecycle(
		first.Revision, first.Owner, first.Device, first.Heartbeat, first.Inventory,
	)
	if err != nil {
		t.Fatalf("restore lifecycle image: %v", err)
	}
	if !reflect.DeepEqual(restored, first) {
		t.Fatalf("restored=%#v, want %#v", restored, first)
	}

	// The complete image owns its capability slices across a restart boundary.
	firstResult.Heartbeat.Instance.Capabilities.Runtimes[0] = "mutated"
	if first.Heartbeat.Instance.Capabilities.Runtimes[0] == "mutated" {
		t.Fatal("lifecycle image shares heartbeat capability storage with result")
	}

	nextInput := lifecyclePersistenceInput(t, 1, 2)
	next, nextResult, err := CommitPersistedEnrollmentHeartbeatLifecycle(&first, 1, nextInput)
	if err != nil {
		t.Fatalf("second lifecycle commit: %v", err)
	}
	if next.Revision != 2 || next.Heartbeat.Revision != 2 || next.Inventory.Revision != 2 ||
		next.Heartbeat.Instance.HeartbeatSequence != 2 || nextResult.Projection.Revision != 2 {
		t.Fatalf("second image=%#v result=%#v", next, nextResult)
	}

	before := first
	if _, _, err := CommitPersistedEnrollmentHeartbeatLifecycle(&first, 0, nextInput); err != ErrLifecycleRevisionConflict {
		t.Fatalf("stale outer revision error=%v, want %v", err, ErrLifecycleRevisionConflict)
	}
	if !reflect.DeepEqual(first, before) {
		t.Fatal("stale outer revision mutated current image")
	}

	foreignInput := lifecyclePersistenceInput(t, 1, 2)
	foreignInput.Owner.Subject = "other-user"
	if _, _, err := CommitPersistedEnrollmentHeartbeatLifecycle(&first, 1, foreignInput); err != ErrLifecycleBindingChanged {
		t.Fatalf("owner drift error=%v, want %v", err, ErrLifecycleBindingChanged)
	}
}

func TestPersistedEnrollmentHeartbeatLifecycleRejectsPartialOrInconsistentImages(t *testing.T) {
	input := lifecyclePersistenceInput(t, 1, 1)
	state, _, err := CommitPersistedEnrollmentHeartbeatLifecycle(nil, 0, input)
	if err != nil {
		t.Fatalf("build lifecycle image: %v", err)
	}

	tests := []struct {
		name   string
		mutate func(*PersistedEnrollmentHeartbeatLifecycleState)
	}{
		{name: "outer_revision_drift", mutate: func(value *PersistedEnrollmentHeartbeatLifecycleState) { value.Revision = 2 }},
		{name: "heartbeat_revision_drift", mutate: func(value *PersistedEnrollmentHeartbeatLifecycleState) { value.Heartbeat.Revision = 2 }},
		{name: "inventory_revision_drift", mutate: func(value *PersistedEnrollmentHeartbeatLifecycleState) { value.Inventory.Revision = 2 }},
		{name: "runner_sequence_drift", mutate: func(value *PersistedEnrollmentHeartbeatLifecycleState) { value.Inventory.Runner.HeartbeatSequence = 2 }},
		{name: "runner_capability_drift", mutate: func(value *PersistedEnrollmentHeartbeatLifecycleState) {
			value.Inventory.Runner.Capabilities.Runtimes[0] = "cuda"
		}},
		{name: "runner_instance_drift", mutate: func(value *PersistedEnrollmentHeartbeatLifecycleState) {
			value.Inventory.Runner.InstanceID = "runner-b"
		}},
		{name: "owner_drift", mutate: func(value *PersistedEnrollmentHeartbeatLifecycleState) { value.Owner.Subject = "other-user" }},
		{name: "device_owner_drift", mutate: func(value *PersistedEnrollmentHeartbeatLifecycleState) {
			value.Device.Owner.Subject = "other-user"
		}},
		{name: "inventory_owner_drift", mutate: func(value *PersistedEnrollmentHeartbeatLifecycleState) {
			value.Inventory.Device.Owner.Subject = "other-user"
		}},
		{name: "approval_drift", mutate: func(value *PersistedEnrollmentHeartbeatLifecycleState) { value.Device.ApprovalState = "pending" }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			candidate := state
			test.mutate(&candidate)
			if err := validatePersistedEnrollmentHeartbeatLifecycle(candidate); err != ErrLifecycleInvalidState {
				t.Fatalf("validation error=%v, want %v", err, ErrLifecycleInvalidState)
			}
		})
	}
}

func TestPersistedEnrollmentHeartbeatLifecycleRejectsReplayAndOverflow(t *testing.T) {
	input := lifecyclePersistenceInput(t, 1, 1)
	state, _, err := CommitPersistedEnrollmentHeartbeatLifecycle(nil, 0, input)
	if err != nil {
		t.Fatalf("build lifecycle image: %v", err)
	}
	if _, _, err := CommitPersistedEnrollmentHeartbeatLifecycle(&state, 1, input); err != deviceheartbeat.ErrSequenceNotIncreasing {
		t.Fatalf("replayed heartbeat error=%v, want %v", err, deviceheartbeat.ErrSequenceNotIncreasing)
	}
	serverStateDrift := lifecyclePersistenceInput(t, 1, 2)
	serverStateDrift.CordonState = "cordoned"
	if _, _, err := CommitPersistedEnrollmentHeartbeatLifecycle(&state, 1, serverStateDrift); err != ErrLifecycleServerStateChanged {
		t.Fatalf("server-state drift error=%v, want %v", err, ErrLifecycleServerStateChanged)
	}

	overflow := state
	overflow.Revision = math.MaxUint64
	overflow.Heartbeat.Revision = math.MaxUint64
	overflow.Inventory.Revision = math.MaxUint64
	if _, _, err := CommitPersistedEnrollmentHeartbeatLifecycle(&overflow, math.MaxUint64, input); err != ErrLifecycleRevisionOverflow {
		t.Fatalf("outer revision overflow error=%v, want %v", err, ErrLifecycleRevisionOverflow)
	}
}

func lifecyclePersistenceInput(t *testing.T, generation, sequence uint64) EnrollmentHeartbeatLifecycleInput {
	t.Helper()
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	device := deviceidentity.DeviceBinding{
		DeviceID: "device-a", Owner: owner, KeyID: "key-a",
		PublicKeySHA256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		ApprovalState:   "approved", CredentialState: "active",
	}
	challenge := deviceidentity.Challenge{
		ChallengeID: "challenge-a", ChallengeSHA256: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
		IssuedAtMS: 100_000, ExpiresAtMS: 160_000,
	}
	proof := deviceidentity.Proof{
		DeviceID: "device-a", KeyID: "key-a", PublicKeySHA256: device.PublicKeySHA256, Owner: owner,
		ChallengeID: challenge.ChallengeID, ChallengeSHA256: challenge.ChallengeSHA256,
		ProofSHA256: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
		IssuedAtMS:  110_000, ExpiresAtMS: 150_000,
	}
	capabilities, err := deviceheartbeat.NewCapabilitySnapshot(
		"linux", "amd64", 8, 7, 16<<30, 8<<30, 100<<30, 50<<30,
		[]deviceheartbeat.GPUCapability{{ID: "gpu-a", Vendor: "NVIDIA", MemoryBytes: 8 << 30, AvailableMemoryBytes: 4 << 30}},
		[]string{"oci", "python"},
	)
	if err != nil {
		t.Fatalf("capabilities: %v", err)
	}
	return EnrollmentHeartbeatLifecycleInput{
		Owner: owner, Device: device, Challenge: challenge, Proof: proof,
		Heartbeat: deviceheartbeat.Heartbeat{
			DeviceID: "device-a", InstanceID: "runner-a", Generation: generation,
			Sequence: sequence, Capabilities: capabilities,
		},
		CordonState: "clear", ReservationState: "none",
		ServerObservedAtMS: 110_000 + sequence*1_000, LeaseTTLMS: 60_000,
		IdentityNowMS: 120_000, EvaluatedAtMS: 120_000, StaleAfterMS: 60_000,
	}
}
