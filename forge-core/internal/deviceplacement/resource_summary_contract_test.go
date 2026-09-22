package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

type resourceSummaryFixture struct {
	APIVersion               string                      `json:"api_version"`
	InventoryContractFixture string                      `json:"inventory_contract_fixture"`
	PlacementContractFixture string                      `json:"placement_contract_fixture"`
	Owner                    Owner                       `json:"owner"`
	Inventory                inventoryObservationFixture `json:"inventory"`
	Placement                resourceSummaryPlacement    `json:"placement_observation"`
	Expected                 resourceSummaryExpected     `json:"expected"`
}

type resourceSummaryPlacement struct {
	SchemaVersion              string                     `json:"schema_version"`
	EvaluationMode             string                     `json:"evaluation_mode"`
	Owner                      Owner                      `json:"owner"`
	ConversationID             string                     `json:"conversation_id"`
	RunID                      string                     `json:"run_id"`
	EvaluatedAtMS              int64                      `json:"evaluated_at_ms"`
	OwnerDeclarationUnverified bool                       `json:"owner_declaration_unverified"`
	DeviceAttributesUnverified bool                       `json:"device_attributes_unverified"`
	Decisions                  []sessionPlacementDecision `json:"decisions"`
	SelectedDeviceID           *string                    `json:"selected_device_id"`
	SelectedInstanceID         *string                    `json:"selected_instance_id"`
	Authority                  SessionPlacementAuthority  `json:"authority"`
}

type resourceSummaryExpected struct {
	SchemaVersion                   string                    `json:"schema_version"`
	EvaluationMode                  string                    `json:"evaluation_mode"`
	ConversationID                  string                    `json:"conversation_id"`
	RunID                           string                    `json:"run_id"`
	EvaluatedAtMS                   int64                     `json:"evaluated_at_ms"`
	OwnerDeclarationUnverified      bool                      `json:"owner_declaration_unverified"`
	InventoryDeclarationsUnverified bool                      `json:"inventory_declarations_unverified"`
	PlacementDeclarationUnverified  bool                      `json:"placement_declaration_unverified"`
	Notice                          string                    `json:"notice"`
	DeviceCount                     int                       `json:"device_count"`
	RunnerInstanceCount             int                       `json:"runner_instance_count"`
	AvailableCPUCores               uint64                    `json:"available_cpu_cores"`
	AvailableMemoryBytes            uint64                    `json:"available_memory_bytes"`
	AvailableStorageBytes           uint64                    `json:"available_storage_bytes"`
	AvailableGPUCount               int                       `json:"available_gpu_count"`
	AvailableGPUMemoryBytes         uint64                    `json:"available_gpu_memory_bytes"`
	EligibleDeviceCount             int                       `json:"eligible_device_count"`
	EligibleInstanceCount           int                       `json:"eligible_instance_count"`
	SelectedDeviceID                *string                   `json:"selected_device_id"`
	SelectedInstanceID              *string                   `json:"selected_instance_id"`
	Authority                       SessionPlacementAuthority `json:"authority"`
}

func TestDeviceResourceSummaryContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_DEVICE_RESOURCE_SUMMARY_CONTRACT_FIXTURE")
	if path == "" {
		path = filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "forge-device-resource-summary-v1.json")
	}
	fixture := readResourceSummaryFixture(t, path)
	if fixture.APIVersion != "forgeos.device-resource-summary-contract/v1" ||
		fixture.InventoryContractFixture != "forge-device-inventory-observation-v1" ||
		fixture.PlacementContractFixture != "forge-session-placement-observation-v1" ||
		fixture.Inventory.Owner != fixture.Owner || fixture.Placement.Owner != fixture.Owner {
		t.Fatalf("unexpected resource summary binding: %#v", fixture)
	}

	inventory := make([]SessionPlacementCandidate, 0, len(fixture.Inventory.Devices))
	for _, candidate := range fixture.Inventory.Devices {
		inventory = append(inventory, SessionPlacementCandidate{
			InstanceID: candidate.InstanceID,
			Device:     candidate.Device,
		})
	}
	placement := SessionPlacementObservation{
		SchemaVersion:              fixture.Placement.SchemaVersion,
		EvaluationMode:             fixture.Placement.EvaluationMode,
		Owner:                      fixture.Placement.Owner,
		ConversationID:             fixture.Placement.ConversationID,
		RunID:                      fixture.Placement.RunID,
		EvaluatedAtMS:              fixture.Placement.EvaluatedAtMS,
		OwnerDeclarationUnverified: fixture.Placement.OwnerDeclarationUnverified,
		DeviceAttributesUnverified: fixture.Placement.DeviceAttributesUnverified,
		Decisions:                  make([]SessionPlacementDecision, 0, len(fixture.Placement.Decisions)),
		SelectedDeviceID:           fixture.Placement.SelectedDeviceID,
		SelectedInstanceID:         fixture.Placement.SelectedInstanceID,
		Authority:                  fixture.Placement.Authority,
	}
	for _, decision := range fixture.Placement.Decisions {
		placement.Decisions = append(placement.Decisions, SessionPlacementDecision{
			DeviceID:            decision.DeviceID,
			InstanceID:          decision.InstanceID,
			MatchesRequirements: decision.MatchesRequirements,
			ExclusionReasons:    decision.ExclusionReasons,
		})
	}

	observation, err := ObserveDeviceResourceSummary(DeviceResourceSummaryRequest{
		Owner: fixture.Owner, Inventory: inventory, Placement: placement,
	})
	if err != nil {
		t.Fatalf("observe device resource summary: %v", err)
	}
	want := fixture.Expected
	if observation.SchemaVersion != want.SchemaVersion || observation.EvaluationMode != want.EvaluationMode ||
		observation.Owner != fixture.Owner || observation.ConversationID != want.ConversationID ||
		observation.RunID != want.RunID || observation.EvaluatedAtMS != want.EvaluatedAtMS ||
		observation.OwnerDeclarationUnverified != want.OwnerDeclarationUnverified ||
		observation.InventoryDeclarationsUnverified != want.InventoryDeclarationsUnverified ||
		observation.PlacementDeclarationUnverified != want.PlacementDeclarationUnverified ||
		observation.Notice != want.Notice || observation.DeviceCount != want.DeviceCount ||
		observation.RunnerInstanceCount != want.RunnerInstanceCount ||
		observation.AvailableCPUCores != want.AvailableCPUCores ||
		observation.AvailableMemoryBytes != want.AvailableMemoryBytes ||
		observation.AvailableStorageBytes != want.AvailableStorageBytes ||
		observation.AvailableGPUCount != want.AvailableGPUCount ||
		observation.AvailableGPUMemoryBytes != want.AvailableGPUMemoryBytes ||
		observation.EligibleDeviceCount != want.EligibleDeviceCount ||
		observation.EligibleInstanceCount != want.EligibleInstanceCount ||
		!reflect.DeepEqual(observation.SelectedDeviceID, want.SelectedDeviceID) ||
		!reflect.DeepEqual(observation.SelectedInstanceID, want.SelectedInstanceID) ||
		observation.Authority != want.Authority {
		t.Fatalf("resource summary = %#v, want %#v", observation, want)
	}
}

func TestDeviceResourceSummaryRejectsConfusedDeclarations(t *testing.T) {
	owner := Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	device := validRequest().Devices[0]
	base := DeviceResourceSummaryRequest{
		Owner:     owner,
		Inventory: []SessionPlacementCandidate{{InstanceID: "runner-1", Device: device}},
		Placement: SessionPlacementObservation{
			SchemaVersion: SessionPlacementObservationSchemaVersion, EvaluationMode: EvaluationMode,
			Owner: owner, ConversationID: "conversation-1", RunID: "run-1", EvaluatedAtMS: 10,
			OwnerDeclarationUnverified: true, DeviceAttributesUnverified: true,
			Decisions: []SessionPlacementDecision{{DeviceID: device.DeviceID, InstanceID: "runner-1", MatchesRequirements: true}},
			Authority: SessionPlacementAuthority{},
		},
	}
	for name, mutate := range map[string]func(*DeviceResourceSummaryRequest){
		"decision device": func(request *DeviceResourceSummaryRequest) {
			request.Placement.Decisions[0].DeviceID = "foreign-device"
		},
		"placement authority": func(request *DeviceResourceSummaryRequest) {
			request.Placement.Authority.ExecutionAuthorized = true
		},
		"placement selection": func(request *DeviceResourceSummaryRequest) {
			selected := device.DeviceID
			request.Placement.SelectedDeviceID = &selected
		},
	} {
		t.Run(name, func(t *testing.T) {
			request := base
			request.Placement.Decisions = append([]SessionPlacementDecision(nil), base.Placement.Decisions...)
			mutate(&request)
			if _, err := ObserveDeviceResourceSummary(request); err == nil {
				t.Fatal("confused resource summary declaration was accepted")
			}
		})
	}
}

func readResourceSummaryFixture(t *testing.T, path string) resourceSummaryFixture {
	t.Helper()
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	var fixture resourceSummaryFixture
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode resource summary fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		t.Fatalf("resource summary fixture has trailing JSON: %v", err)
	}
	return fixture
}
