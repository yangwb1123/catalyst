package deviceinventory

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"testing"
)

type statusFixture struct {
	SchemaVersion  string                 `json:"schema_version"`
	EvaluationMode string                 `json:"evaluation_mode"`
	StaleAfterMS   uint64                 `json:"stale_after_ms"`
	Authority      statusAuthorityFixture `json:"authority"`
	Cases          []statusCaseFixture    `json:"cases"`
}

type statusAuthorityFixture struct {
	IdentityVerified       bool `json:"identity_verified"`
	HeartbeatPersisted     bool `json:"heartbeat_persisted"`
	InventoryAuthoritative bool `json:"inventory_authoritative"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
}

type statusCaseFixture struct {
	Name     string                `json:"name"`
	Input    Observation           `json:"input"`
	Expected statusExpectedFixture `json:"expected"`
}

type statusExpectedFixture struct {
	Status           string `json:"status"`
	Fresh            bool   `json:"fresh"`
	DeclaredEligible bool   `json:"declared_eligible"`
	Error            string `json:"error"`
}

func TestInventoryStatusContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_INVENTORY_STATUS_CONTRACT_FIXTURE")
	if path == "" {
		t.Skip("FORGE_INVENTORY_STATUS_CONTRACT_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture statusFixture
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode inventory status fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("inventory status fixture has trailing JSON: %v", err)
	}
	if fixture.SchemaVersion != "forge.device-inventory-status-contract/v1" ||
		fixture.EvaluationMode != "pure_projection_only" || fixture.StaleAfterMS != DefaultStaleAfterMS ||
		fixture.Authority.IdentityVerified || fixture.Authority.HeartbeatPersisted ||
		fixture.Authority.InventoryAuthoritative || fixture.Authority.ReservationCreated ||
		fixture.Authority.ExecutionAuthorized || fixture.Authority.DispatchPerformed || len(fixture.Cases) != 11 {
		t.Fatalf("invalid inventory status fixture envelope: %#v", fixture)
	}
	for _, testCase := range fixture.Cases {
		t.Run(testCase.Name, func(t *testing.T) {
			result, err := Project(testCase.Input, fixture.StaleAfterMS)
			if testCase.Expected.Error != "" {
				if err == nil || err.Error() != testCase.Expected.Error {
					t.Fatalf("error=%v, want %q", err, testCase.Expected.Error)
				}
				return
			}
			if err != nil {
				t.Fatalf("projection rejected: %v", err)
			}
			if result.Status != testCase.Expected.Status || result.Fresh != testCase.Expected.Fresh ||
				result.DeclaredEligible != testCase.Expected.DeclaredEligible {
				t.Fatalf("projection=%#v, want %#v", result, testCase.Expected)
			}
		})
	}
}
