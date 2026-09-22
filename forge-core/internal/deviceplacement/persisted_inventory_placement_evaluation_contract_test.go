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
)

type persistedInventoryPlacementEvaluationFixture struct {
	SchemaVersion      string                                   `json:"schema_version"`
	EvaluationMode     string                                   `json:"evaluation_mode"`
	SourceFixture      string                                   `json:"source_fixture"`
	SourceCase         string                                   `json:"source_case"`
	EvaluatedAtMS      int64                                    `json:"evaluated_at_ms"`
	PolicyRequirements Requirements                             `json:"policy_requirements"`
	Authority          persistedInventoryPlacementEvalAuthority `json:"authority"`
	Expected           persistedInventoryPlacementEvalExpected  `json:"expected"`
}

type persistedInventoryPlacementEvalAuthority struct {
	PlacementEvaluated  bool `json:"placement_evaluated"`
	PlacementSelected   bool `json:"placement_selected"`
	ReservationCreated  bool `json:"reservation_created"`
	ExecutionAuthorized bool `json:"execution_authorized"`
	DispatchPerformed   bool `json:"dispatch_performed"`
}

type persistedInventoryPlacementEvalExpected struct {
	Accepted                   bool     `json:"accepted"`
	Error                      string   `json:"error"`
	Revision                   uint64   `json:"revision"`
	DeviceID                   string   `json:"device_id"`
	InstanceID                 string   `json:"instance_id"`
	MatchesRequirements        bool     `json:"matches_requirements"`
	ExclusionReasons           []string `json:"exclusion_reasons"`
	OwnerDeclarationUnverified bool     `json:"owner_declaration_unverified"`
	DeviceAttributesUnverified bool     `json:"device_attributes_unverified"`
}

func TestPersistedInventoryPlacementEvaluationContractFixture(t *testing.T) {
	fixture := readPersistedInventoryPlacementEvaluationFixture(t)
	if fixture.SchemaVersion != "forge.device-inventory-placement-evaluation/v1" ||
		fixture.EvaluationMode != "pure_persisted_inventory_offline_evaluation" ||
		fixture.SourceFixture != "forge-device-inventory-placement-input-v1.json" ||
		fixture.SourceCase != "online" || fixture.EvaluatedAtMS <= 0 ||
		fixture.Authority.PlacementEvaluated || fixture.Authority.PlacementSelected ||
		fixture.Authority.ReservationCreated || fixture.Authority.ExecutionAuthorized ||
		fixture.Authority.DispatchPerformed {
		t.Fatalf("invalid persisted inventory evaluation envelope: %#v", fixture)
	}

	source := readPersistedInventoryPlacementInputFixture(t)
	input, err := BuildPersistedInventoryPlacementInput(source.State, source.EvaluationOwner)
	if err != nil {
		t.Fatalf("build persisted placement input: %v", err)
	}
	actual, err := EvaluatePersistedInventoryPlacementInput(
		input,
		fixture.PolicyRequirements,
		fixture.EvaluatedAtMS,
	)
	if !fixture.Expected.Accepted {
		if err == nil || err.Error() != fixture.Expected.Error {
			t.Fatalf("error = %v, want %q", err, fixture.Expected.Error)
		}
		return
	}
	if err != nil {
		t.Fatalf("evaluate persisted placement input: %v", err)
	}
	if actual.Revision != fixture.Expected.Revision || actual.DeviceID != fixture.Expected.DeviceID ||
		actual.InstanceID != fixture.Expected.InstanceID ||
		actual.MatchesRequirements != fixture.Expected.MatchesRequirements ||
		!reflect.DeepEqual(actual.ExclusionReasons, fixture.Expected.ExclusionReasons) ||
		actual.OwnerDeclarationUnverified != fixture.Expected.OwnerDeclarationUnverified ||
		actual.DeviceAttributesUnverified != fixture.Expected.DeviceAttributesUnverified {
		t.Fatalf("evaluation = %#v, want %#v", actual, fixture.Expected)
	}
}

func TestPersistedInventoryPlacementEvaluationRejectsMultipleGPUs(t *testing.T) {
	source := readPersistedInventoryPlacementInputFixture(t)
	input, err := BuildPersistedInventoryPlacementInput(source.State, source.EvaluationOwner)
	if err != nil {
		t.Fatal(err)
	}
	input.Capabilities.GPUs = []deviceheartbeat.GPUCapability{
		{ID: "gpu-a", Vendor: "vendor", MemoryBytes: 1, AvailableMemoryBytes: 1},
		{ID: "gpu-b", Vendor: "vendor", MemoryBytes: 1, AvailableMemoryBytes: 1},
	}
	if _, err := EvaluatePersistedInventoryPlacementInput(input, Requirements{
		OS: "linux", Architecture: "amd64", MinCPUCores: 1,
		MinMemoryBytes: 1, MinStorageBytes: 1, Runtime: "oci",
		GPU: GPURequirement{}, DataResidencyZones: []string{"us-west"},
		MinimumTrustZone: "standard", SandboxFloor: "container", ConcurrencySlots: 1,
	}, 200500); err == nil || err.Error() != "unsupported_persisted_placement_capability" {
		t.Fatalf("multiple GPU error = %v", err)
	}
}

func readPersistedInventoryPlacementEvaluationFixture(t *testing.T) persistedInventoryPlacementEvaluationFixture {
	t.Helper()
	path := os.Getenv("FORGE_DEVICE_INVENTORY_PLACEMENT_EVALUATION_FIXTURE")
	if path == "" {
		path = filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "forge-device-inventory-placement-evaluation-v1.json")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture persistedInventoryPlacementEvaluationFixture
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode persisted inventory evaluation fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("persisted inventory evaluation fixture has trailing JSON: %v", err)
	}
	return fixture
}

func readPersistedInventoryPlacementInputFixture(t *testing.T) persistedInventoryPlacementFixture {
	t.Helper()
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
		t.Fatalf("decode persisted inventory input fixture: %v", err)
	}
	return fixture
}
