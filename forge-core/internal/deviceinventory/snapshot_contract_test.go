package deviceinventory

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"reflect"
	"testing"
)

type snapshotFixture struct {
	SchemaVersion              string                   `json:"schema_version"`
	EvaluationMode             string                   `json:"evaluation_mode"`
	OwnerDeclarationUnverified bool                     `json:"owner_declaration_unverified"`
	InventoryUnverified        bool                     `json:"inventory_declarations_unverified"`
	Authority                  snapshotAuthorityFixture `json:"authority"`
	Cases                      []snapshotCaseFixture    `json:"cases"`
}

type snapshotAuthorityFixture struct {
	IdentityVerified       bool `json:"identity_verified"`
	HeartbeatPersisted     bool `json:"heartbeat_persisted"`
	InventoryAuthoritative bool `json:"inventory_authoritative"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
}

type snapshotCaseFixture struct {
	Name     string                  `json:"name"`
	Input    snapshotInputFixture    `json:"input"`
	Expected snapshotExpectedFixture `json:"expected"`
}

type snapshotInputFixture struct {
	SnapshotID   string               `json:"snapshot_id"`
	ObservedAtMS uint64               `json:"observed_at_ms"`
	Owner        SnapshotOwner        `json:"owner"`
	Rows         []snapshotRowFixture `json:"rows"`
}

type snapshotRowFixture struct {
	DeviceID   string        `json:"device_id"`
	InstanceID string        `json:"instance_id"`
	Owner      SnapshotOwner `json:"owner"`
}

type snapshotExpectedFixture struct {
	OrderedKeys     []string `json:"ordered_keys"`
	CanonicalSHA256 string   `json:"canonical_sha256"`
	Error           string   `json:"error"`
}

func TestInventorySnapshotCanonicalContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_INVENTORY_SNAPSHOT_CANONICAL_FIXTURE")
	if path == "" {
		t.Skip("FORGE_INVENTORY_SNAPSHOT_CANONICAL_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var fixture snapshotFixture
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode inventory snapshot fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("inventory snapshot fixture has trailing JSON: %v", err)
	}
	if fixture.SchemaVersion != "forge.device-inventory-snapshot-canonical/v1" ||
		fixture.EvaluationMode != "pure_owner_scoped_snapshot_only" ||
		!fixture.OwnerDeclarationUnverified || !fixture.InventoryUnverified ||
		fixture.Authority.IdentityVerified || fixture.Authority.HeartbeatPersisted ||
		fixture.Authority.InventoryAuthoritative || fixture.Authority.ReservationCreated ||
		fixture.Authority.ExecutionAuthorized || fixture.Authority.DispatchPerformed ||
		len(fixture.Cases) != 6 {
		t.Fatalf("invalid inventory snapshot fixture envelope: %#v", fixture)
	}
	for _, testCase := range fixture.Cases {
		t.Run(testCase.Name, func(t *testing.T) {
			rows := make([]SnapshotRow, 0, len(testCase.Input.Rows))
			for _, row := range testCase.Input.Rows {
				rows = append(rows, SnapshotRow{DeviceID: row.DeviceID, InstanceID: row.InstanceID, Owner: row.Owner})
			}
			input := Snapshot{SnapshotID: testCase.Input.SnapshotID, ObservedAtMS: testCase.Input.ObservedAtMS, Owner: testCase.Input.Owner, Rows: rows}
			before := append([]SnapshotRow{}, input.Rows...)
			canonical, err := CanonicalizeSnapshot(input)
			if testCase.Expected.Error != "" {
				if err == nil || err.Error() != testCase.Expected.Error {
					t.Fatalf("error=%v, want %q", err, testCase.Expected.Error)
				}
				return
			}
			if err != nil {
				t.Fatalf("canonicalization rejected: %v", err)
			}
			if !reflect.DeepEqual(input.Rows, before) {
				t.Fatal("canonicalization mutated input rows")
			}
			if got := snapshotKeys(canonical.Rows); !reflect.DeepEqual(got, testCase.Expected.OrderedKeys) {
				t.Fatalf("ordered rows=%v, want %v", got, testCase.Expected.OrderedKeys)
			}
			digest, err := SnapshotDigest(input)
			if err != nil || digest != testCase.Expected.CanonicalSHA256 {
				t.Fatalf("digest=%q err=%v, want %q", digest, err, testCase.Expected.CanonicalSHA256)
			}
		})
	}
}

func snapshotKeys(rows []SnapshotRow) []string {
	keys := make([]string, 0, len(rows))
	for _, row := range rows {
		keys = append(keys, row.DeviceID+"/"+row.InstanceID)
	}
	return keys
}
