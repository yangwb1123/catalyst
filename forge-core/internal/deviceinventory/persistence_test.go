package deviceinventory

import (
	"math"
	"reflect"
	"testing"

	"forgeos/forge-core/internal/deviceheartbeat"
)

func TestRestoreAndProjectPersistedInventory(t *testing.T) {
	state := validPersistedInventory(t, 3)
	projection, err := ProjectPersistedInventory(state, state.Device.Owner, 1_500, DefaultStaleAfterMS)
	if err != nil {
		t.Fatalf("project persisted inventory: %v", err)
	}
	want := PersistedInventoryProjection{
		Revision:         3,
		DeviceID:         "device-a",
		InstanceID:       "runner-a",
		Status:           StatusOnline,
		Fresh:            true,
		DeclaredEligible: true,
	}
	if !reflect.DeepEqual(projection, want) {
		t.Fatalf("projection=%#v, want %#v", projection, want)
	}

	restored, err := RestorePersistedInventory(state.Revision, state.Device, state.Runner)
	if err != nil {
		t.Fatalf("restore persisted inventory: %v", err)
	}
	if !reflect.DeepEqual(restored, state) {
		t.Fatalf("restored=%#v, want %#v", restored, state)
	}
	restored.Runner.Capabilities.Runtimes[0] = "tampered"
	if state.Runner.Capabilities.Runtimes[0] == "tampered" {
		t.Fatal("restore shared the capability slice with its input")
	}
}

func TestCommitPersistedInventoryUsesExactRevisionAndCompleteReplacement(t *testing.T) {
	current := validPersistedInventory(t, 3)
	nextRunner := current.Runner
	nextRunner.HeartbeatSequence++
	nextRunner.ServerObservedAtMS = 2_000
	nextRunner.CapabilityLeaseExpiresAtMS = 6_000
	before := current

	next, err := CommitPersistedInventory(&current, 3, current.Device, nextRunner)
	if err != nil {
		t.Fatalf("commit persisted inventory: %v", err)
	}
	if next.Revision != 4 || next.Runner.HeartbeatSequence != 2 {
		t.Fatalf("next=%#v, want revision 4 and sequence 2", next)
	}
	if !reflect.DeepEqual(current, before) {
		t.Fatal("commit mutated the current value")
	}
	if _, err := CommitPersistedInventory(&current, 2, current.Device, nextRunner); err != ErrRevisionConflict {
		t.Fatalf("stale revision error=%v, want %v", err, ErrRevisionConflict)
	}
}

func TestCommitPersistedInventoryRejectsRevisionOverflow(t *testing.T) {
	current := validPersistedInventory(t, math.MaxUint64)
	if _, err := CommitPersistedInventory(&current, math.MaxUint64, current.Device, current.Runner); err != ErrRevisionOverflow {
		t.Fatalf("overflow error=%v, want %v", err, ErrRevisionOverflow)
	}
}

func TestPersistedInventoryRejectsInvalidRestoreAndBindingChanges(t *testing.T) {
	valid := validPersistedInventory(t, 1)
	if _, err := RestorePersistedInventory(0, valid.Device, valid.Runner); err != ErrInvalidPersistedState {
		t.Fatalf("zero revision error=%v, want %v", err, ErrInvalidPersistedState)
	}
	foreignRunner := valid.Runner
	foreignRunner.DeviceID = "device-b"
	if _, err := RestorePersistedInventory(valid.Revision, valid.Device, foreignRunner); err != ErrRunnerDeviceMismatch {
		t.Fatalf("foreign runner error=%v, want %v", err, ErrRunnerDeviceMismatch)
	}
	foreignDevice := valid.Device
	foreignDevice.DeviceID = "device-b"
	if _, err := CommitPersistedInventory(&valid, valid.Revision, foreignDevice, valid.Runner); err != ErrDeviceBindingChanged {
		t.Fatalf("changed device error=%v, want %v", err, ErrDeviceBindingChanged)
	}
	foreignOwner := valid.Device.Owner
	foreignOwner.Subject = "other-user"
	if _, err := ProjectPersistedInventory(valid, foreignOwner, 1_500, DefaultStaleAfterMS); err != ErrInventoryOwnerMismatch {
		t.Fatalf("foreign owner error=%v, want %v", err, ErrInventoryOwnerMismatch)
	}
}

func TestPersistedInventoryRejectsNonCanonicalCapabilities(t *testing.T) {
	state := validPersistedInventory(t, 1)
	state.Runner.Capabilities.Runtimes = []string{"rust", "go"}
	if _, err := RestorePersistedInventory(state.Revision, state.Device, state.Runner); err != ErrInvalidRunnerRecord {
		t.Fatalf("noncanonical capability error=%v, want %v", err, ErrInvalidRunnerRecord)
	}
}

func TestPersistedInventoryProjectionKeepsStatusRules(t *testing.T) {
	tests := []struct {
		name         string
		approval     string
		cordon       string
		liveness     string
		observedAt   uint64
		expiresAt    uint64
		evaluatedAt  uint64
		wantStatus   string
		wantFresh    bool
		wantEligible bool
	}{
		{name: "stale", observedAt: 1_000, expiresAt: 2_000, evaluatedAt: 100_001, wantStatus: StatusStale},
		{name: "expired", observedAt: 1_000, expiresAt: 2_000, evaluatedAt: 2_000, wantStatus: StatusStale},
		{name: "offline", liveness: "offline", observedAt: 1_000, expiresAt: 5_000, evaluatedAt: 1_500, wantStatus: StatusOffline, wantFresh: true},
		{name: "pending", approval: "pending", observedAt: 1_000, expiresAt: 5_000, evaluatedAt: 1_500, wantStatus: StatusPending, wantFresh: true},
		{name: "cordoned", cordon: "cordoned", observedAt: 1_000, expiresAt: 5_000, evaluatedAt: 1_500, wantStatus: StatusCordoned, wantFresh: true},
		{name: "revoked", approval: "revoked", observedAt: 1_000, expiresAt: 5_000, evaluatedAt: 1_500, wantStatus: StatusRevoked, wantFresh: true},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			state := validPersistedInventory(t, 1)
			if test.approval != "" {
				state.Device.ApprovalState = test.approval
			}
			if test.cordon != "" {
				state.Device.CordonState = test.cordon
			}
			if test.liveness != "" {
				state.Runner.Liveness = test.liveness
			}
			state.Runner.ServerObservedAtMS = test.observedAt
			state.Runner.CapabilityLeaseExpiresAtMS = test.expiresAt
			projection, err := ProjectPersistedInventory(state, state.Device.Owner, test.evaluatedAt, DefaultStaleAfterMS)
			if err != nil {
				t.Fatalf("project persisted inventory: %v", err)
			}
			if projection.Status != test.wantStatus || projection.Fresh != test.wantFresh || projection.DeclaredEligible != test.wantEligible {
				t.Fatalf("projection=%#v, want status=%q fresh=%v eligible=%v", projection, test.wantStatus, test.wantFresh, test.wantEligible)
			}
		})
	}
}

func validPersistedInventory(t *testing.T, revision uint64) PersistedInventoryState {
	t.Helper()
	capabilities, err := deviceheartbeat.NewCapabilitySnapshot(
		"Linux", "AMD64", 8, 7,
		16<<30, 8<<30,
		100<<30, 50<<30,
		nil, []string{"Go", "Rust"},
	)
	if err != nil {
		t.Fatalf("build capabilities: %v", err)
	}
	return PersistedInventoryState{
		Revision: revision,
		Device: DeviceRecord{
			DeviceID:         "device-a",
			Owner:            SnapshotOwner{Issuer: "issuer", Subject: "user", TenantID: "tenant"},
			ApprovalState:    "approved",
			CordonState:      "clear",
			ReservationState: "none",
		},
		Runner: RunnerInstanceRecord{
			DeviceID:                   "device-a",
			InstanceID:                 "runner-a",
			Generation:                 1,
			HeartbeatSequence:          1,
			ServerObservedAtMS:         1_000,
			CapabilityLeaseExpiresAtMS: 5_000,
			Liveness:                   "online",
			Capabilities:               capabilities,
		},
	}
}
