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

	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceinventory"
)

type persistedInventoryPlacementBatchFixture struct {
	SchemaVersion      string                                      `json:"schema_version"`
	EvaluationMode     string                                      `json:"evaluation_mode"`
	SourceFixture      string                                      `json:"source_fixture"`
	EvaluationOwner    deviceinventory.SnapshotOwner               `json:"evaluation_owner"`
	EvaluatedAtMS      int64                                       `json:"evaluated_at_ms"`
	Requirements       Requirements                                `json:"requirements"`
	Cases              []persistedInventoryPlacementBatchCase      `json:"cases"`
	EmptyInputs        bool                                        `json:"empty_inputs_allowed"`
	SelectedDeviceID   *string                                     `json:"selected_device_id"`
	SelectedInstanceID *string                                     `json:"selected_instance_id"`
	Authority          persistedInventoryPlacementBatchAuthority   `json:"authority"`
	ErrorCases         []persistedInventoryPlacementBatchErrorCase `json:"error_cases"`
}

type persistedInventoryPlacementBatchCase struct {
	Name                       string                                   `json:"name"`
	SourceCase                 string                                   `json:"source_case"`
	DeviceID                   string                                   `json:"device_id"`
	InstanceID                 string                                   `json:"instance_id"`
	SnapshotObservedAtMS       *uint64                                  `json:"snapshot_observed_at_ms"`
	CapabilityLeaseExpiresAtMS *uint64                                  `json:"capability_lease_expires_at_ms"`
	Expected                   persistedInventoryPlacementBatchExpected `json:"expected"`
}

type persistedInventoryPlacementBatchExpected struct {
	Revision            uint64   `json:"revision"`
	DeviceID            string   `json:"device_id"`
	InstanceID          string   `json:"instance_id"`
	MatchesRequirements bool     `json:"matches_requirements"`
	ExclusionReasons    []string `json:"exclusion_reasons"`
}

type persistedInventoryPlacementBatchErrorCase struct {
	Name  string `json:"name"`
	Error string `json:"error"`
}

type persistedInventoryPlacementBatchAuthority struct {
	IdentityVerified       bool `json:"identity_verified"`
	HeartbeatPersisted     bool `json:"heartbeat_persisted"`
	InventoryAuthoritative bool `json:"inventory_authoritative"`
	PlacementSelected      bool `json:"placement_selected"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
}

func TestPersistedInventoryPlacementBatchContractFixture(t *testing.T) {
	fixture := readPersistedInventoryPlacementBatchFixture(t)
	if fixture.SchemaVersion != "forge.device-inventory-placement-batch-evaluation/v1" ||
		fixture.EvaluationMode != "pure_persisted_inventory_placement_dry_run" ||
		fixture.SourceFixture != "forge-device-inventory-placement-input-v1.json" ||
		fixture.EvaluatedAtMS != 200500 || len(fixture.Cases) != 6 ||
		fixture.SelectedDeviceID != nil || fixture.SelectedInstanceID != nil ||
		fixture.Authority != (persistedInventoryPlacementBatchAuthority{}) {
		t.Fatalf("invalid batch fixture envelope: %#v", fixture)
	}
	source := readPersistedInventoryPlacementInputFixture(t)
	inputs := make([]PersistedInventoryPlacementInput, 0, len(fixture.Cases))
	for _, batchCase := range fixture.Cases {
		state := batchStateForCase(source, batchCase)
		input, err := BuildPersistedInventoryPlacementInput(state, source.EvaluationOwner)
		if err != nil {
			t.Fatalf("build %s: %v", batchCase.Name, err)
		}
		inputs = append(inputs, input)
	}
	actual, err := EvaluatePersistedInventoryPlacement(PersistedInventoryPlacementBatchRequest{
		Owner: sourceOwner(source), EvaluatedAtMS: fixture.EvaluatedAtMS,
		Requirements: fixture.Requirements, Inputs: inputs,
	})
	if err != nil {
		t.Fatalf("batch evaluation: %v", err)
	}
	if actual.SchemaVersion != "forge.persisted-inventory-placement-evaluation/v1" ||
		actual.EvaluationMode != fixture.EvaluationMode || actual.Owner != sourceOwner(source) ||
		!actual.OwnerDeclarationUnverified || !actual.DeviceAttributesUnverified ||
		actual.SelectedDeviceID != fixture.SelectedDeviceID || actual.SelectedInstanceID != fixture.SelectedInstanceID ||
		actual.Authority != (PersistedInventoryPlacementBatchAuthority{}) {
		t.Fatalf("invalid batch result envelope: %#v", actual)
	}
	if len(actual.Decisions) != len(fixture.Cases) {
		t.Fatalf("decision count = %d, want %d", len(actual.Decisions), len(fixture.Cases))
	}
	for index, batchCase := range fixture.Cases {
		got := actual.Decisions[index]
		want := batchCase.Expected
		if got.Revision != want.Revision || got.DeviceID != want.DeviceID || got.InstanceID != want.InstanceID ||
			got.MatchesRequirements != want.MatchesRequirements || !reflect.DeepEqual(got.ExclusionReasons, want.ExclusionReasons) {
			t.Errorf("decision %d = %#v, want %#v", index, got, want)
		}
	}
}

func TestPersistedInventoryPlacementBatchErrorsAndEmptyInputs(t *testing.T) {
	fixture := readPersistedInventoryPlacementBatchFixture(t)
	source := readPersistedInventoryPlacementInputFixture(t)
	base := make([]PersistedInventoryPlacementInput, 0, len(fixture.Cases))
	for _, batchCase := range fixture.Cases {
		input, err := BuildPersistedInventoryPlacementInput(batchStateForCase(source, batchCase), source.EvaluationOwner)
		if err != nil {
			t.Fatal(err)
		}
		base = append(base, input)
	}
	for _, errorCase := range fixture.ErrorCases {
		t.Run(errorCase.Name, func(t *testing.T) {
			inputs := append([]PersistedInventoryPlacementInput(nil), base...)
			owner := sourceOwner(source)
			evaluatedAtMS := fixture.EvaluatedAtMS
			switch errorCase.Name {
			case "owner_mismatch":
				owner.Subject = "other-user"
			case "duplicate_device":
				inputs = append(inputs, inputs[0])
			case "duplicate_instance":
				duplicate := inputs[0]
				duplicate.DeviceID = "device-z"
				duplicate.Capabilities = cloneCapabilities(duplicate.Capabilities)
				inputs = append(inputs, duplicate)
			case "invalid_evaluated_at":
				evaluatedAtMS = 0
			default:
				t.Fatalf("unknown error case %q", errorCase.Name)
			}
			_, err := EvaluatePersistedInventoryPlacement(PersistedInventoryPlacementBatchRequest{
				Owner: owner, EvaluatedAtMS: evaluatedAtMS,
				Requirements: fixture.Requirements, Inputs: inputs,
			})
			if err == nil || err.Error() != errorCase.Error {
				t.Fatalf("error = %v, want %q", err, errorCase.Error)
			}
		})
	}
	if !fixture.EmptyInputs {
		t.Fatal("fixture must define empty-input behavior")
	}
	empty, err := EvaluatePersistedInventoryPlacement(PersistedInventoryPlacementBatchRequest{
		Owner: sourceOwner(source), EvaluatedAtMS: fixture.EvaluatedAtMS,
		Requirements: fixture.Requirements,
	})
	if err != nil || len(empty.Decisions) != 0 || empty.SelectedDeviceID != nil || empty.SelectedInstanceID != nil ||
		empty.Authority != (PersistedInventoryPlacementBatchAuthority{}) {
		t.Fatalf("empty batch = %#v, %v", empty, err)
	}
}

func TestPersistedInventoryPlacementBatchRejectsNonEmptyGPU(t *testing.T) {
	source := readPersistedInventoryPlacementInputFixture(t)
	input, err := BuildPersistedInventoryPlacementInput(source.State, source.EvaluationOwner)
	if err != nil {
		t.Fatal(err)
	}
	input.Capabilities.GPUs = []deviceheartbeat.GPUCapability{{ID: "gpu-a", Vendor: "vendor", MemoryBytes: 1, AvailableMemoryBytes: 1}}
	_, err = EvaluatePersistedInventoryPlacement(PersistedInventoryPlacementBatchRequest{
		Owner: sourceOwner(source), EvaluatedAtMS: 200500,
		Requirements: Requirements{
			OS: "linux", Architecture: "amd64", MinCPUCores: 1, MinMemoryBytes: 1, MinStorageBytes: 1,
			Runtime: "oci", GPU: GPURequirement{}, DataResidencyZones: []string{"us-west"},
			MinimumTrustZone: "standard", SandboxFloor: "container", ConcurrencySlots: 1,
		},
		Inputs: []PersistedInventoryPlacementInput{input},
	})
	if err == nil || err.Error() != "unsupported_persisted_placement_capability" {
		t.Fatalf("GPU error = %v", err)
	}
}

func batchStateForCase(source persistedInventoryPlacementFixture, batchCase persistedInventoryPlacementBatchCase) deviceinventory.PersistedInventoryState {
	var sourceCase persistedInventoryPlacementCase
	for _, candidate := range source.Cases {
		if candidate.Name == batchCase.SourceCase {
			sourceCase = candidate
			break
		}
	}
	state := source.State
	if sourceCase.ApprovalState != "" {
		state.Device.ApprovalState = sourceCase.ApprovalState
	}
	if sourceCase.CordonState != "" {
		state.Device.CordonState = sourceCase.CordonState
	}
	if sourceCase.Liveness != "" {
		state.Runner.Liveness = sourceCase.Liveness
	}
	if sourceCase.ServerObservedAtMS != nil {
		state.Runner.ServerObservedAtMS = *sourceCase.ServerObservedAtMS
	}
	if sourceCase.CapabilityLeaseExpiresAtMS != nil {
		state.Runner.CapabilityLeaseExpiresAtMS = *sourceCase.CapabilityLeaseExpiresAtMS
	}
	if batchCase.DeviceID != "" {
		state.Device.DeviceID = batchCase.DeviceID
		state.Runner.DeviceID = batchCase.DeviceID
	}
	if batchCase.InstanceID != "" {
		state.Runner.InstanceID = batchCase.InstanceID
	}
	if batchCase.SnapshotObservedAtMS != nil {
		state.Runner.ServerObservedAtMS = *batchCase.SnapshotObservedAtMS
	}
	if batchCase.CapabilityLeaseExpiresAtMS != nil {
		state.Runner.CapabilityLeaseExpiresAtMS = *batchCase.CapabilityLeaseExpiresAtMS
	}
	return state
}

func sourceOwner(source persistedInventoryPlacementFixture) Owner {
	return Owner{Issuer: source.EvaluationOwner.Issuer, Subject: source.EvaluationOwner.Subject, TenantID: source.EvaluationOwner.TenantID}
}

func cloneCapabilities(value deviceheartbeat.CapabilitySnapshot) deviceheartbeat.CapabilitySnapshot {
	value.GPUs = append([]deviceheartbeat.GPUCapability(nil), value.GPUs...)
	value.Runtimes = append([]string(nil), value.Runtimes...)
	return value
}

func readPersistedInventoryPlacementBatchFixture(t *testing.T) persistedInventoryPlacementBatchFixture {
	t.Helper()
	path := os.Getenv("FORGE_DEVICE_INVENTORY_PLACEMENT_BATCH_EVALUATION_FIXTURE")
	if path == "" {
		path = filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "forge-device-inventory-placement-batch-evaluation-v1.json")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture persistedInventoryPlacementBatchFixture
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode persisted inventory batch fixture: %v", err)
	}
	var trailing any
	if !errors.Is(decoder.Decode(&trailing), io.EOF) {
		t.Fatal("persisted inventory batch fixture has trailing JSON")
	}
	return fixture
}
