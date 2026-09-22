package deviceapproval

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"testing"

	"forgeos/forge-core/internal/deviceidentity"
)

type approvalFixture struct {
	SchemaVersion  string                   `json:"schema_version"`
	EvaluationMode string                   `json:"evaluation_mode"`
	Notice         string                   `json:"notice"`
	Initial        State                    `json:"initial"`
	Authority      approvalFixtureAuthority `json:"authority"`
	Cases          []approvalFixtureCase    `json:"cases"`
}

type approvalFixtureAuthority struct {
	OwnerAuthenticated     bool `json:"owner_authenticated"`
	Persisted              bool `json:"persisted"`
	CredentialIssued       bool `json:"credential_issued"`
	InventoryAuthoritative bool `json:"inventory_authoritative"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
}

type approvalFixtureCase struct {
	Name                string               `json:"name"`
	Action              Action               `json:"action"`
	Owner               deviceidentity.Owner `json:"owner"`
	DeviceID            string               `json:"device_id"`
	NextKeyID           string               `json:"next_key_id"`
	NextPublicKeySHA256 string               `json:"next_public_key_sha256"`
	ExpectedState       *State               `json:"expected_state"`
	ExpectedError       string               `json:"expected_error"`
}

func TestDeviceApprovalRotationContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_DEVICE_APPROVAL_ROTATION_FIXTURE")
	if path == "" {
		t.Skip("FORGE_DEVICE_APPROVAL_ROTATION_FIXTURE is set by the contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var fixture approvalFixture
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode device approval fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("device approval fixture has trailing JSON: %v", err)
	}
	if fixture.SchemaVersion != SchemaVersion || fixture.EvaluationMode != EvaluationMode ||
		fixture.Initial.ApprovalState != ApprovalPending || fixture.Initial.KeyGeneration != 1 ||
		fixture.Authority.OwnerAuthenticated || fixture.Authority.Persisted || fixture.Authority.CredentialIssued ||
		fixture.Authority.InventoryAuthoritative || fixture.Authority.ExecutionAuthorized || len(fixture.Cases) != 7 {
		t.Fatalf("invalid device approval fixture envelope: %#v", fixture)
	}
	if err := fixture.Initial.Validate(); err != nil {
		t.Fatalf("invalid fixture initial state: %v", err)
	}
	for _, testCase := range fixture.Cases {
		t.Run(testCase.Name, func(t *testing.T) {
			transition, err := Apply(Request{
				Current:             fixture.Initial,
				Action:              testCase.Action,
				Owner:               testCase.Owner,
				DeviceID:            testCase.DeviceID,
				NextKeyID:           testCase.NextKeyID,
				NextPublicKeySHA256: testCase.NextPublicKeySHA256,
			})
			if testCase.ExpectedError != "" {
				if err == nil || err.Error() != testCase.ExpectedError {
					t.Fatalf("error = %v, want %q", err, testCase.ExpectedError)
				}
				return
			}
			if err != nil {
				t.Fatalf("transition error = %v", err)
			}
			if testCase.ExpectedState == nil || transition.Next != *testCase.ExpectedState {
				t.Fatalf("next = %#v, want %#v", transition.Next, testCase.ExpectedState)
			}
			if transition.SchemaVersion != SchemaVersion || transition.EvaluationMode != EvaluationMode ||
				!transition.PreviewOnly || !transition.Authority.OwnerBindingMatched || transition.Authority.OwnerAuthenticated ||
				transition.Authority.Persisted || transition.Authority.CredentialIssued ||
				transition.Authority.InventoryAuthoritative || transition.Authority.ExecutionAuthorized {
				t.Fatalf("authority boundary = %#v", transition)
			}
		})
	}
}
