package deviceplacement

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

func TestSessionDeviceObservationRoundTrip(t *testing.T) {
	value := validSessionDeviceObservation()
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := DecodeSessionDeviceObservation(bytes.NewReader(encoded))
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(decoded, value) {
		t.Fatalf("decoded=%#v, want=%#v", decoded, value)
	}
}

func TestSessionDeviceObservationSharedFixture(t *testing.T) {
	path := os.Getenv("FORGE_SESSION_DEVICE_OBSERVATION_FIXTURE")
	if path == "" {
		path = filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "forge-session-device-observation-v1.json")
	}
	file, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()
	value, err := DecodeSessionDeviceObservation(file)
	if err != nil {
		t.Fatal(err)
	}
	if value.ConversationID != "conversation-001" || value.RunID != "run-001" || len(value.Inventory.Devices) != 9 {
		t.Fatalf("unexpected shared observation binding: %#v", value)
	}
}

func TestSessionDeviceObservationRejectsDrift(t *testing.T) {
	base, err := json.Marshal(validSessionDeviceObservation())
	if err != nil {
		t.Fatal(err)
	}
	mutations := map[string]func(map[string]any){
		"unknown root field": func(value map[string]any) { value["extra"] = true },
		"authority claim": func(value map[string]any) {
			value["authority"].(map[string]any)["execution_authorized"] = true
		},
		"foreign run":     func(value map[string]any) { value["run_id"] = "foreign-run" },
		"selected target": func(value map[string]any) { value["selected_device_id"] = "device-1" },
	}
	for name, mutate := range mutations {
		t.Run(name, func(t *testing.T) {
			var value map[string]any
			if err := json.Unmarshal(base, &value); err != nil {
				t.Fatal(err)
			}
			mutate(value)
			encoded, err := json.Marshal(value)
			if err != nil {
				t.Fatal(err)
			}
			if _, err := DecodeSessionDeviceObservation(strings.NewReader(string(encoded))); err == nil {
				t.Fatal("unsafe session device observation was accepted")
			}
		})
	}
}

func TestDecodeSessionPlacementObservationRequestRequiresNestedShape(t *testing.T) {
	placement := validRequest()
	request := SessionPlacementObservationRequest{
		Owner:          validOwnerTuple(),
		ConversationID: "conversation-1",
		RunID:          "run-1",
		Placement:      placement,
		Candidates: []SessionPlacementCandidate{{
			InstanceID: "runner-1",
			Device:     placement.Devices[0],
		}},
	}
	encoded, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := DecodeSessionPlacementObservationRequest(bytes.NewReader(encoded)); err != nil {
		t.Fatalf("valid session placement request rejected: %v", err)
	}

	mutations := map[string]func(map[string]any){
		"missing placement GPU object": func(value map[string]any) {
			delete(value["placement"].(map[string]any)["requirements"].(map[string]any), "gpu")
		},
		"missing candidate device GPU object": func(value map[string]any) {
			delete(value["candidates"].([]any)[0].(map[string]any)["device"].(map[string]any), "gpu")
		},
	}
	for name, mutate := range mutations {
		t.Run(name, func(t *testing.T) {
			var value map[string]any
			if err := json.Unmarshal(encoded, &value); err != nil {
				t.Fatal(err)
			}
			mutate(value)
			mutated, err := json.Marshal(value)
			if err != nil {
				t.Fatal(err)
			}
			if _, err := DecodeSessionPlacementObservationRequest(bytes.NewReader(mutated)); err == nil {
				t.Fatal("incomplete nested request was accepted")
			}
		})
	}
}

func validSessionDeviceObservation() SessionDeviceObservation {
	request := validRequest()
	request.Devices = []Device{validDevice("device-1")}
	candidate := SessionPlacementCandidate{InstanceID: "runner-1", Device: request.Devices[0]}
	placement, err := ObserveSessionPlacement(SessionPlacementObservationRequest{
		Owner: validOwnerTuple(), ConversationID: "conversation-1", RunID: "run-1",
		Placement: request, Candidates: []SessionPlacementCandidate{candidate},
	})
	if err != nil {
		panic(err)
	}
	summary, err := ObserveDeviceResourceSummary(DeviceResourceSummaryRequest{
		Owner: validOwnerTuple(), Inventory: []SessionPlacementCandidate{candidate}, Placement: placement,
	})
	if err != nil {
		panic(err)
	}
	return SessionDeviceObservation{
		SchemaVersion:  SessionDeviceObservationSchemaVersion,
		EvaluationMode: EvaluationMode,
		Owner:          validOwnerTuple(), ConversationID: "conversation-1", RunID: "run-1",
		EvaluatedAtMS: request.EvaluatedAtMS, OwnerDeclarationUnverified: true,
		Inventory: SessionDeviceObservationInventory{
			SchemaVersion: "forge.device-inventory-observation/v1", EvaluationMode: EvaluationMode,
			EvaluatedAtMS: request.EvaluatedAtMS, Owner: validOwnerTuple(),
			OwnerDeclarationUnverified: true, InventoryUnverified: true,
			Notice:  SessionDeviceObservationInventoryNotice,
			Devices: []SessionPlacementCandidate{candidate},
		},
		PlacementObservation: placement, ResourceSummary: summary,
		Authority: SessionPlacementAuthority{},
	}
}
