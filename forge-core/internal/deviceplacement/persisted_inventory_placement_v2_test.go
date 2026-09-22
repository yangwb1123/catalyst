package deviceplacement

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

type persistedInventoryPlacementV2Fixture struct {
	SchemaVersion          string                                    `json:"schema_version"`
	EvaluationMode         string                                    `json:"evaluation_mode"`
	SourceSchemaVersion    string                                    `json:"source_schema_version"`
	EvaluationOwner        Owner                                     `json:"evaluation_owner"`
	EvaluatedAtMS          int64                                     `json:"evaluated_at_ms"`
	Notice                 string                                    `json:"notice"`
	Requirements           Requirements                              `json:"requirements"`
	Observation            SessionDeviceObservationInventoryV2       `json:"observation"`
	Expected               []persistedInventoryPlacementV2Expected   `json:"expected"`
	EligibleCandidateCount int                                       `json:"eligible_candidate_count"`
	SelectedDeviceID       *string                                   `json:"selected_device_id"`
	SelectedInstanceID     *string                                   `json:"selected_instance_id"`
	Authority              PersistedInventoryPlacementBatchAuthority `json:"authority"`
}

type persistedInventoryPlacementV2Expected struct {
	Revision                uint64   `json:"revision"`
	Generation              uint64   `json:"generation"`
	HeartbeatSequence       uint64   `json:"heartbeat_sequence"`
	DeviceID                string   `json:"device_id"`
	InstanceID              string   `json:"instance_id"`
	ReservationState        string   `json:"reservation_state"`
	GPUCount                int      `json:"gpu_count"`
	AvailableGPUMemoryBytes uint64   `json:"available_gpu_memory_bytes"`
	MatchesRequirements     bool     `json:"matches_requirements"`
	ExclusionReasons        []string `json:"exclusion_reasons"`
}

func TestPersistedInventoryPlacementV2ContractFixture(t *testing.T) {
	fixture := readPersistedInventoryPlacementV2Fixture(t)
	actual, err := EvaluatePersistedInventoryObservationV2(
		fixture.Observation, fixture.EvaluationOwner, fixture.Requirements, fixture.EvaluatedAtMS,
	)
	if err != nil {
		t.Fatalf("evaluate v2 fixture: %v", err)
	}
	if actual.SchemaVersion != fixture.SchemaVersion || actual.EvaluationMode != fixture.EvaluationMode ||
		actual.SourceSchemaVersion != fixture.SourceSchemaVersion || actual.Owner != fixture.EvaluationOwner ||
		actual.EvaluatedAtMS != fixture.EvaluatedAtMS || actual.Notice != fixture.Notice ||
		actual.EligibleCandidateCount != fixture.EligibleCandidateCount || actual.SelectedDeviceID != nil ||
		actual.SelectedInstanceID != nil || actual.Authority != fixture.Authority {
		t.Fatalf("v2 envelope = %#v", actual)
	}
	if len(actual.Decisions) != len(fixture.Expected) {
		t.Fatalf("decision count = %d, want %d", len(actual.Decisions), len(fixture.Expected))
	}
	for index, got := range actual.Decisions {
		want := fixture.Expected[index]
		if got.Revision != want.Revision || got.Generation != want.Generation || got.HeartbeatSequence != want.HeartbeatSequence ||
			got.DeviceID != want.DeviceID || got.InstanceID != want.InstanceID || got.ReservationState != want.ReservationState ||
			got.GPUCount != want.GPUCount || got.AvailableGPUMemoryBytes != want.AvailableGPUMemoryBytes ||
			got.MatchesRequirements != want.MatchesRequirements || !reflect.DeepEqual(got.ExclusionReasons, want.ExclusionReasons) ||
			!got.OwnerDeclarationUnverified || !got.DeviceAttributesUnverified {
			t.Errorf("decision %d = %#v, want %#v", index, got, want)
		}
	}
}

func TestPersistedInventoryPlacementV2RejectsOwnerDriftAndGPURuntimeRequirement(t *testing.T) {
	fixture := readPersistedInventoryPlacementV2Fixture(t)
	foreign := fixture.EvaluationOwner
	foreign.Subject = "other-user"
	if _, err := EvaluatePersistedInventoryObservationV2(fixture.Observation, foreign, fixture.Requirements, fixture.EvaluatedAtMS); err == nil {
		t.Fatal("foreign owner was accepted")
	}
	fixture.Requirements.GPU.Runtime = "cuda"
	if _, err := EvaluatePersistedInventoryObservationV2(fixture.Observation, fixture.EvaluationOwner, fixture.Requirements, fixture.EvaluatedAtMS); err == nil {
		t.Fatal("GPU runtime requirement was accepted without a v2 runtime declaration")
	}
}

func TestPersistedInventoryPlacementV2RejectsDuplicateDevice(t *testing.T) {
	fixture := readPersistedInventoryPlacementV2Fixture(t)
	fixture.Observation.Devices = append(fixture.Observation.Devices, fixture.Observation.Devices[0])
	if _, err := EvaluatePersistedInventoryObservationV2(fixture.Observation, fixture.EvaluationOwner, fixture.Requirements, fixture.EvaluatedAtMS); err == nil {
		t.Fatal("duplicate device was accepted")
	}
}

func TestPersistedInventoryPlacementV2RejectsAggregateGPUBytesAboveJSONSafeInteger(t *testing.T) {
	fixture := readPersistedInventoryPlacementV2Fixture(t)
	available := uint64(5_000_000_000_000_000)
	fixture.Observation.Devices[0].Device.GPUs = []GPUDeclarationV2{
		{ID: "gpu-a", Vendor: "NVIDIA", MemoryBytes: available, AvailableMemoryBytes: available},
		{ID: "gpu-b", Vendor: "NVIDIA", MemoryBytes: available, AvailableMemoryBytes: available},
	}
	if _, err := EvaluatePersistedInventoryObservationV2(
		fixture.Observation, fixture.EvaluationOwner, fixture.Requirements, fixture.EvaluatedAtMS,
	); err == nil {
		t.Fatal("aggregate GPU memory above JSON-safe integer was accepted")
	}
}

func readPersistedInventoryPlacementV2Fixture(t *testing.T) persistedInventoryPlacementV2Fixture {
	t.Helper()
	path := os.Getenv("FORGE_DEVICE_INVENTORY_PLACEMENT_V2_FIXTURE")
	if path == "" {
		path = filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "forge-device-inventory-placement-evaluation-v2.json")
	}
	bytes, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture persistedInventoryPlacementV2Fixture
	if err := json.Unmarshal(bytes, &fixture); err != nil {
		t.Fatal(err)
	}
	return fixture
}
