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

const inventoryObservationNotice = "Every owner, instance, state, timestamp, resource, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only observation selects no target and grants no execution authority."

type inventoryObservationFixture struct {
	SchemaVersion              string                          `json:"schema_version"`
	EvaluationMode             string                          `json:"evaluation_mode"`
	EvaluatedAtMS              int64                           `json:"evaluated_at_ms"`
	Owner                      Owner                           `json:"owner_declaration"`
	OwnerDeclarationUnverified bool                            `json:"owner_declaration_unverified"`
	InventoryUnverified        bool                            `json:"inventory_declarations_unverified"`
	Notice                     string                          `json:"notice"`
	Devices                    []inventoryObservationCandidate `json:"devices"`
	ExecutionAuthorized        bool                            `json:"execution_authorized"`
	ReservationCreated         bool                            `json:"reservation_created"`
	DispatchPerformed          bool                            `json:"dispatch_performed"`
}

type inventoryObservationCandidate struct {
	InstanceID string `json:"instance_id"`
	Device     Device `json:"device"`
}

func TestInventoryObservationContractFixture(t *testing.T) {
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "forge-device-inventory-observation-v1.json")
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture inventoryObservationFixture
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode inventory observation fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		t.Fatalf("inventory observation fixture has trailing JSON: %v", err)
	}
	validateInventoryObservationFixture(t, fixture)

	request := Request{
		SchemaVersion: RequestSchemaVersion, EvaluatedAtMS: fixture.EvaluatedAtMS,
		Owner: fixture.Owner, MaxSnapshotAgeMS: 90_000,
		Requirements: Requirements{
			OS: "linux", Architecture: "amd64", MinCPUCores: 4,
			MinMemoryBytes: 8_192, MinStorageBytes: 4_096, Runtime: "oci",
			GPU: GPURequirement{}, DataResidencyZones: []string{"us-west"},
			MinimumTrustZone: "standard", SandboxFloor: "container", ConcurrencySlots: 1,
		},
		Devices: []Device{fixture.Devices[0].Device, fixture.Devices[1].Device},
	}
	result, err := Evaluate(request)
	if err != nil {
		t.Fatal(err)
	}
	if got, want := result.DeviceResults, []DeviceResult{
		{DeviceID: "device-a", AttributesUnverified: true, MatchesRequirements: true, ExclusionReasons: []string{}},
		{DeviceID: "device-b", AttributesUnverified: true, ExclusionReasons: []string{"approval_pending"}},
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("offline inventory projection = %#v, want %#v", got, want)
	}
	if result.ExecutionAuthorized || result.ReservationCreated || result.DispatchPerformed {
		t.Fatal("inventory observation fixture must remain non-authoritative")
	}
}

func validateInventoryObservationFixture(t *testing.T, fixture inventoryObservationFixture) {
	t.Helper()
	if fixture.SchemaVersion != "forge.device-inventory-observation/v1" ||
		fixture.EvaluationMode != EvaluationMode || fixture.EvaluatedAtMS != 200_000 ||
		!fixture.OwnerDeclarationUnverified || !fixture.InventoryUnverified ||
		fixture.Notice != inventoryObservationNotice || fixture.ExecutionAuthorized ||
		fixture.ReservationCreated || fixture.DispatchPerformed || !validOwner(fixture.Owner) ||
		len(fixture.Devices) != 2 {
		t.Fatalf("invalid inventory observation envelope: %#v", fixture)
	}
	seenDevices := make(map[string]struct{}, len(fixture.Devices))
	seenInstances := make(map[string]struct{}, len(fixture.Devices))
	for index, candidate := range fixture.Devices {
		if !validDeviceID(candidate.InstanceID) || !validDeviceDeclaration(candidate.Device) ||
			candidate.Device.Owner != fixture.Owner {
			t.Fatalf("invalid inventory observation candidate %d: %#v", index, candidate)
		}
		if _, exists := seenDevices[candidate.Device.DeviceID]; exists {
			t.Fatalf("duplicate inventory device %q", candidate.Device.DeviceID)
		}
		if _, exists := seenInstances[candidate.InstanceID]; exists {
			t.Fatalf("duplicate inventory instance %q", candidate.InstanceID)
		}
		seenDevices[candidate.Device.DeviceID] = struct{}{}
		seenInstances[candidate.InstanceID] = struct{}{}
		if index > 0 && fixture.Devices[index-1].Device.DeviceID >= candidate.Device.DeviceID {
			t.Fatal("inventory devices must be sorted by device ID")
		}
	}
}
