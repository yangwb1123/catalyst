package deviceplacement

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"reflect"
	"testing"

	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceinventory"
)

type persistedInventoryObservationContractFixture struct {
	SchemaVersion   string                                    `json:"schema_version"`
	EvaluationMode  string                                    `json:"evaluation_mode"`
	EvaluationOwner deviceinventory.SnapshotOwner             `json:"evaluation_owner"`
	EvaluatedAtMS   uint64                                    `json:"evaluated_at_ms"`
	Authority       persistedInventoryObservationAuthority    `json:"authority"`
	States          []deviceinventory.PersistedInventoryState `json:"states"`
	Expected        SessionDeviceObservationInventory         `json:"expected"`
}

type persistedInventoryObservationAuthority struct {
	IdentityVerified       bool `json:"identity_verified"`
	HeartbeatPersisted     bool `json:"heartbeat_persisted"`
	InventoryAuthoritative bool `json:"inventory_authoritative"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
}

func TestPersistedInventoryObservationContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_INVENTORY_PERSISTED_OBSERVATION_FIXTURE")
	if path == "" {
		t.Skip("FORGE_INVENTORY_PERSISTED_OBSERVATION_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := rejectDuplicateFields(data); err != nil {
		t.Fatalf("persisted inventory observation fixture has duplicate fields: %v", err)
	}
	var fixture persistedInventoryObservationContractFixture
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode persisted inventory observation fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("persisted inventory observation fixture has trailing JSON: %v", err)
	}
	if fixture.SchemaVersion != "forge.device-inventory-persisted-observation/v1" ||
		fixture.EvaluationMode != "pure_persisted_inventory_to_observation" ||
		fixture.EvaluatedAtMS != 1500 || fixture.Authority != (persistedInventoryObservationAuthority{}) {
		t.Fatalf("invalid persisted inventory observation envelope: %#v", fixture)
	}
	owner := fixture.EvaluationOwner
	if owner != (deviceinventory.SnapshotOwner{Issuer: "issuer", Subject: "user", TenantID: "tenant"}) || len(fixture.States) != 2 {
		t.Fatalf("invalid persisted inventory owner/states: %#v", fixture)
	}
	// The persisted fixture deliberately uses human-readable mixed-case tags.
	// Restore canonicalizes these tags before the value-only projection; the
	// resulting observation remains the lower-case wire contract.
	for index := range fixture.States {
		value := fixture.States[index].Runner.Capabilities
		canonical, err := deviceheartbeat.NewCapabilitySnapshot(
			value.OperatingSystem, value.Architecture, value.CPUCores, value.AvailableCPUCores,
			value.MemoryBytes, value.AvailableMemoryBytes, value.StorageBytes, value.AvailableStorageBytes,
			value.GPUs, value.Runtimes,
		)
		if err != nil {
			t.Fatalf("canonicalize persisted state %d: %v", index, err)
		}
		fixture.States[index].Runner.Capabilities = canonical
	}
	got, err := BuildPersistedInventoryObservation(fixture.States, owner, fixture.EvaluatedAtMS)
	if err != nil {
		t.Fatalf("build persisted inventory observation: %v", err)
	}
	if !reflect.DeepEqual(got, fixture.Expected) {
		t.Fatalf("persisted inventory projection = %#v, want %#v", got, fixture.Expected)
	}
	if got.ExecutionAuthorized || got.ReservationCreated || got.DispatchPerformed {
		t.Fatal("persisted inventory observation fixture grants authority")
	}
}
