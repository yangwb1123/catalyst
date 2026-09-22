package deviceplacement

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"forgeos/forge-core/internal/deviceinventory"
)

type persistedInventoryPlacementFixture struct {
	SchemaVersion      string                                  `json:"schema_version"`
	EvaluationMode     string                                  `json:"evaluation_mode"`
	EvaluationOwner    deviceinventory.SnapshotOwner           `json:"evaluation_owner"`
	PolicyRequirements persistedInventoryPlacementPolicy       `json:"policy_requirements"`
	Authority          persistedInventoryPlacementAuthority    `json:"authority"`
	State              deviceinventory.PersistedInventoryState `json:"state"`
	Cases              []persistedInventoryPlacementCase       `json:"cases"`
}

type persistedInventoryPlacementPolicy struct {
	DataResidencyZones []string `json:"data_residency_zones"`
	MinimumTrustZone   string   `json:"minimum_trust_zone"`
	SandboxFloor       string   `json:"sandbox_floor"`
	ConcurrencySlots   uint16   `json:"concurrency_slots"`
}

type persistedInventoryPlacementAuthority struct {
	IdentityVerified       bool `json:"identity_verified"`
	HeartbeatPersisted     bool `json:"heartbeat_persisted"`
	InventoryAuthoritative bool `json:"inventory_authoritative"`
	PlacementSelected      bool `json:"placement_selected"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
}

type persistedInventoryPlacementCase struct {
	Name                       string                              `json:"name"`
	EvaluationOwner            *deviceinventory.SnapshotOwner      `json:"evaluation_owner"`
	RunnerDeviceID             string                              `json:"runner_device_id"`
	ApprovalState              string                              `json:"approval_state"`
	CordonState                string                              `json:"cordon_state"`
	Liveness                   string                              `json:"liveness"`
	ServerObservedAtMS         *uint64                             `json:"server_observed_at_ms"`
	CapabilityLeaseExpiresAtMS *uint64                             `json:"capability_lease_expires_at_ms"`
	Expected                   persistedInventoryPlacementExpected `json:"expected"`
}

type persistedInventoryPlacementExpected struct {
	Accepted                   bool     `json:"accepted"`
	Error                      string   `json:"error"`
	Revision                   uint64   `json:"revision"`
	DeviceID                   string   `json:"device_id"`
	InstanceID                 string   `json:"instance_id"`
	Generation                 uint64   `json:"generation"`
	HeartbeatSequence          uint64   `json:"heartbeat_sequence"`
	ApprovalState              string   `json:"approval_state"`
	CordonState                string   `json:"cordon_state"`
	ReservationState           string   `json:"reservation_state"`
	Liveness                   string   `json:"liveness"`
	SnapshotObservedAtMS       int64    `json:"snapshot_observed_at_ms"`
	LeaseExpiresAtMS           int64    `json:"lease_expires_at_ms"`
	OwnerDeclarationUnverified bool     `json:"owner_declaration_unverified"`
	PolicyAttributesUnverified bool     `json:"policy_attributes_unverified"`
	DataResidencyZones         []string `json:"data_residency_zones"`
	TrustZone                  string   `json:"trust_zone"`
	SandboxLevels              []string `json:"sandbox_levels"`
	ConcurrencyLimit           uint16   `json:"concurrency_limit"`
	ActiveConcurrency          uint16   `json:"active_concurrency"`
	PolicyRequirementsMet      bool     `json:"policy_requirements_met"`
}

func TestPersistedInventoryPlacementInputContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_DEVICE_INVENTORY_PLACEMENT_INPUT_FIXTURE")
	if path == "" {
		path = filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "forge-device-inventory-placement-input-v1.json")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture persistedInventoryPlacementFixture
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode persisted inventory placement fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("persisted inventory placement fixture has trailing JSON: %v", err)
	}
	assertPersistedInventoryPlacementEnvelope(t, fixture)

	for _, testCase := range fixture.Cases {
		t.Run(testCase.Name, func(t *testing.T) {
			state := fixture.State
			if testCase.RunnerDeviceID != "" {
				state.Runner.DeviceID = testCase.RunnerDeviceID
			}
			if testCase.ApprovalState != "" {
				state.Device.ApprovalState = testCase.ApprovalState
			}
			if testCase.CordonState != "" {
				state.Device.CordonState = testCase.CordonState
			}
			if testCase.Liveness != "" {
				state.Runner.Liveness = testCase.Liveness
			}
			if testCase.ServerObservedAtMS != nil {
				state.Runner.ServerObservedAtMS = *testCase.ServerObservedAtMS
			}
			if testCase.CapabilityLeaseExpiresAtMS != nil {
				state.Runner.CapabilityLeaseExpiresAtMS = *testCase.CapabilityLeaseExpiresAtMS
			}
			owner := fixture.EvaluationOwner
			if testCase.EvaluationOwner != nil {
				owner = *testCase.EvaluationOwner
			}
			input, err := BuildPersistedInventoryPlacementInput(state, owner)
			if !testCase.Expected.Accepted {
				if err == nil || err.Error() != testCase.Expected.Error {
					t.Fatalf("error = %v, want %q", err, testCase.Expected.Error)
				}
				return
			}
			if err != nil {
				t.Fatalf("build placement input: %v", err)
			}
			assertPersistedInventoryPlacementInput(t, input, testCase.Expected)
			if !reflect.DeepEqual(input.Capabilities, state.Runner.Capabilities) {
				t.Fatalf("capabilities = %#v, want persisted %#v", input.Capabilities, state.Runner.Capabilities)
			}
			if policyRequirementsMet(input, fixture.PolicyRequirements) != testCase.Expected.PolicyRequirementsMet {
				t.Fatalf("policy result = %t, want %t", policyRequirementsMet(input, fixture.PolicyRequirements), testCase.Expected.PolicyRequirementsMet)
			}
			mutated := input
			mutated.TrustZone = "standard"
			if ValidatePersistedInventoryPlacementInput(mutated) == nil {
				t.Fatal("supplied policy attributes must be rejected at the persisted-inventory boundary")
			}
		})
	}
}

func assertPersistedInventoryPlacementEnvelope(t *testing.T, fixture persistedInventoryPlacementFixture) {
	t.Helper()
	if fixture.SchemaVersion != "forge.device-inventory-placement-input/v1" ||
		fixture.EvaluationMode != "pure_persisted_inventory_to_placement_input" ||
		fixture.Authority.IdentityVerified || fixture.Authority.HeartbeatPersisted ||
		fixture.Authority.InventoryAuthoritative || fixture.Authority.PlacementSelected ||
		fixture.Authority.ReservationCreated || fixture.Authority.ExecutionAuthorized ||
		fixture.Authority.DispatchPerformed || len(fixture.Cases) != 12 ||
		len(fixture.PolicyRequirements.DataResidencyZones) == 0 || fixture.PolicyRequirements.MinimumTrustZone == "" ||
		fixture.PolicyRequirements.SandboxFloor == "" || fixture.PolicyRequirements.ConcurrencySlots == 0 {
		t.Fatalf("invalid persisted inventory placement envelope: %#v", fixture)
	}
}

func assertPersistedInventoryPlacementInput(t *testing.T, input PersistedInventoryPlacementInput, expected persistedInventoryPlacementExpected) {
	t.Helper()
	if input.Revision != expected.Revision || input.DeviceID != expected.DeviceID || input.InstanceID != expected.InstanceID ||
		input.Generation != expected.Generation || input.HeartbeatSequence != expected.HeartbeatSequence ||
		input.ApprovalState != expected.ApprovalState || input.CordonState != expected.CordonState ||
		input.ReservationState != expected.ReservationState || input.Liveness != expected.Liveness ||
		input.SnapshotObservedAtMS != expected.SnapshotObservedAtMS || input.LeaseExpiresAtMS != expected.LeaseExpiresAtMS ||
		input.OwnerDeclarationUnverified != expected.OwnerDeclarationUnverified ||
		input.PolicyAttributesUnverified != expected.PolicyAttributesUnverified ||
		!reflect.DeepEqual(input.DataResidencyZones, expected.DataResidencyZones) || input.TrustZone != expected.TrustZone ||
		!reflect.DeepEqual(input.SandboxLevels, expected.SandboxLevels) || input.ConcurrencyLimit != expected.ConcurrencyLimit ||
		input.ActiveConcurrency != expected.ActiveConcurrency {
		t.Fatalf("placement input = %#v, want fixture fields %#v", input, expected)
	}
}

func policyRequirementsMet(input PersistedInventoryPlacementInput, requirements persistedInventoryPlacementPolicy) bool {
	if len(requirements.DataResidencyZones) != 0 && len(input.DataResidencyZones) == 0 {
		return false
	}
	if requirements.MinimumTrustZone != "" && input.TrustZone == "unknown" {
		return false
	}
	if requirements.SandboxFloor != "" && len(input.SandboxLevels) == 0 {
		return false
	}
	return requirements.ConcurrencySlots == 0 ||
		(input.ConcurrencyLimit >= input.ActiveConcurrency &&
			requirements.ConcurrencySlots <= input.ConcurrencyLimit-input.ActiveConcurrency)
}
