package deviceinventory

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"testing"
)

type persistedInventoryFixture struct {
	SchemaVersion  string                          `json:"schema_version"`
	EvaluationMode string                          `json:"evaluation_mode"`
	StaleAfterMS   uint64                          `json:"stale_after_ms"`
	Authority      persistedInventoryAuthority     `json:"authority"`
	State          PersistedInventoryState         `json:"state"`
	Cases          []persistedInventoryFixtureCase `json:"cases"`
}

type persistedInventoryAuthority struct {
	IdentityVerified       bool `json:"identity_verified"`
	HeartbeatPersisted     bool `json:"heartbeat_persisted"`
	InventoryAuthoritative bool `json:"inventory_authoritative"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
}

type persistedInventoryFixtureCase struct {
	Name                string                        `json:"name"`
	Operation           string                        `json:"operation"`
	EvaluationOwner     string                        `json:"evaluation_owner"`
	EvaluatedAtMS       uint64                        `json:"evaluated_at_ms"`
	ExpectedRevision    uint64                        `json:"expected_revision"`
	StateRevision       *uint64                       `json:"state_revision"`
	RunnerDeviceID      string                        `json:"runner_device_id"`
	DeviceApprovalState string                        `json:"device_approval_state"`
	DeviceCordonState   string                        `json:"device_cordon_state"`
	RunnerLiveness      string                        `json:"runner_liveness"`
	Replacement         persistedInventoryReplacement `json:"replacement"`
	Expected            persistedInventoryExpected    `json:"expected"`
}

type persistedInventoryReplacement struct {
	HeartbeatSequence          uint64 `json:"heartbeat_sequence"`
	ServerObservedAtMS         uint64 `json:"server_observed_at_ms"`
	CapabilityLeaseExpiresAtMS uint64 `json:"capability_lease_expires_at_ms"`
}

type persistedInventoryExpected struct {
	Accepted                   bool   `json:"accepted"`
	Error                      string `json:"error"`
	Revision                   uint64 `json:"revision"`
	DeviceID                   string `json:"device_id"`
	InstanceID                 string `json:"instance_id"`
	HeartbeatSequence          uint64 `json:"heartbeat_sequence"`
	ServerObservedAtMS         uint64 `json:"server_observed_at_ms"`
	CapabilityLeaseExpiresAtMS uint64 `json:"capability_lease_expires_at_ms"`
	Status                     string `json:"status"`
	Fresh                      bool   `json:"fresh"`
	DeclaredEligible           bool   `json:"declared_eligible"`
}

func TestPersistedInventoryContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_DEVICE_INVENTORY_PERSISTENCE_FIXTURE")
	if path == "" {
		t.Skip("FORGE_DEVICE_INVENTORY_PERSISTENCE_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture persistedInventoryFixture
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode persisted inventory fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("persisted inventory fixture has trailing JSON: %v", err)
	}
	if fixture.SchemaVersion != "forge.device-inventory-persistence/v1" ||
		fixture.EvaluationMode != "pure_persisted_inventory_cas_projection" ||
		fixture.StaleAfterMS != DefaultStaleAfterMS ||
		fixture.Authority.IdentityVerified || fixture.Authority.HeartbeatPersisted ||
		fixture.Authority.InventoryAuthoritative || fixture.Authority.ReservationCreated ||
		fixture.Authority.ExecutionAuthorized || fixture.Authority.DispatchPerformed ||
		len(fixture.Cases) != 12 {
		t.Fatalf("invalid persisted inventory fixture envelope: %#v", fixture)
	}
	for _, testCase := range fixture.Cases {
		t.Run(testCase.Name, func(t *testing.T) {
			state := persistedInventoryCaseState(fixture.State, testCase)
			switch testCase.Operation {
			case "restore":
				_, err := RestorePersistedInventory(state.Revision, state.Device, state.Runner)
				assertPersistedInventoryError(t, testCase.Expected, err)
			case "commit":
				replacement := state.Runner
				if testCase.Replacement.HeartbeatSequence != 0 {
					replacement.HeartbeatSequence = testCase.Replacement.HeartbeatSequence
				}
				if testCase.Replacement.ServerObservedAtMS != 0 {
					replacement.ServerObservedAtMS = testCase.Replacement.ServerObservedAtMS
				}
				if testCase.Replacement.CapabilityLeaseExpiresAtMS != 0 {
					replacement.CapabilityLeaseExpiresAtMS = testCase.Replacement.CapabilityLeaseExpiresAtMS
				}
				result, err := CommitPersistedInventory(&state, testCase.ExpectedRevision, state.Device, replacement)
				assertPersistedInventoryCommit(t, testCase.Expected, result, err)
			case "project":
				owner := state.Device.Owner
				switch testCase.EvaluationOwner {
				case "foreign":
					owner.Subject = "other-user"
				case "invalid":
					owner = SnapshotOwner{}
				}
				result, err := ProjectPersistedInventory(state, owner, testCase.EvaluatedAtMS, fixture.StaleAfterMS)
				assertPersistedInventoryProjection(t, testCase.Expected, result, err)
			default:
				t.Fatalf("unknown operation %q", testCase.Operation)
			}
		})
	}
}

func persistedInventoryCaseState(base PersistedInventoryState, testCase persistedInventoryFixtureCase) PersistedInventoryState {
	if testCase.StateRevision != nil {
		base.Revision = *testCase.StateRevision
	}
	if testCase.RunnerDeviceID != "" {
		base.Runner.DeviceID = testCase.RunnerDeviceID
	}
	if testCase.DeviceApprovalState != "" {
		base.Device.ApprovalState = testCase.DeviceApprovalState
	}
	if testCase.DeviceCordonState != "" {
		base.Device.CordonState = testCase.DeviceCordonState
	}
	if testCase.RunnerLiveness != "" {
		base.Runner.Liveness = testCase.RunnerLiveness
	}
	return base
}

func assertPersistedInventoryError(t *testing.T, expected persistedInventoryExpected, err error) {
	t.Helper()
	if expected.Accepted {
		t.Fatalf("expected acceptance, got error %v", err)
	}
	if err == nil || err.Error() != expected.Error {
		t.Fatalf("error=%v, want %q", err, expected.Error)
	}
}

func assertPersistedInventoryCommit(t *testing.T, expected persistedInventoryExpected, result PersistedInventoryState, err error) {
	t.Helper()
	if !expected.Accepted {
		assertPersistedInventoryError(t, expected, err)
		return
	}
	if err != nil {
		t.Fatalf("commit rejected: %v", err)
	}
	if result.Revision != expected.Revision || result.Runner.HeartbeatSequence != expected.HeartbeatSequence ||
		result.Runner.ServerObservedAtMS != expected.ServerObservedAtMS ||
		result.Runner.CapabilityLeaseExpiresAtMS != expected.CapabilityLeaseExpiresAtMS {
		t.Fatalf("result=%#v, want revision=%d sequence=%d observed=%d expiry=%d", result, expected.Revision,
			expected.HeartbeatSequence, expected.ServerObservedAtMS, expected.CapabilityLeaseExpiresAtMS)
	}
}

func assertPersistedInventoryProjection(t *testing.T, expected persistedInventoryExpected, result PersistedInventoryProjection, err error) {
	t.Helper()
	if !expected.Accepted {
		assertPersistedInventoryError(t, expected, err)
		return
	}
	if err != nil {
		t.Fatalf("projection rejected: %v", err)
	}
	if result.Revision != expected.Revision || result.DeviceID != expected.DeviceID || result.InstanceID != expected.InstanceID ||
		result.Status != expected.Status || result.Fresh != expected.Fresh || result.DeclaredEligible != expected.DeclaredEligible {
		t.Fatalf("projection=%#v, want %#v", result, expected)
	}
}
