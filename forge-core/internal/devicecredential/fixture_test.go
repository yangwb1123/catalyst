package devicecredential

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"testing"

	"forgeos/forge-core/internal/deviceidentity"
)

type credentialFixture struct {
	SchemaVersion  string                     `json:"schema_version"`
	EvaluationMode string                     `json:"evaluation_mode"`
	Notice         string                     `json:"notice"`
	Authority      credentialFixtureAuthority `json:"authority"`
	Cases          []credentialFixtureCase    `json:"cases"`
}

type credentialFixtureAuthority struct {
	OwnerAuthenticated     bool `json:"owner_authenticated"`
	CredentialMaterialMade bool `json:"credential_material_made"`
	Persisted              bool `json:"persisted"`
	InventoryAuthoritative bool `json:"inventory_authoritative"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
}

type credentialFixtureCase struct {
	Name                string               `json:"name"`
	Action              Action               `json:"action"`
	Current             *State               `json:"current,omitempty"`
	Owner               deviceidentity.Owner `json:"owner"`
	DeviceID            string               `json:"device_id"`
	ApprovalState       string               `json:"approval_state"`
	CredentialID        string               `json:"credential_id"`
	KeyID               string               `json:"key_id"`
	PublicKeySHA256     string               `json:"public_key_sha256"`
	KeyGeneration       uint64               `json:"key_generation"`
	IssuedAtMS          uint64               `json:"issued_at_ms"`
	ExpiresAtMS         uint64               `json:"expires_at_ms"`
	NextCredentialID    string               `json:"next_credential_id"`
	NextKeyID           string               `json:"next_key_id"`
	NextPublicKeySHA256 string               `json:"next_public_key_sha256"`
	ObservedAtMS        uint64               `json:"observed_at_ms"`
	ExpectedState       *State               `json:"expected_state,omitempty"`
	ExpectedError       string               `json:"expected_error,omitempty"`
}

func TestDeviceCredentialLifecycleContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_DEVICE_CREDENTIAL_LIFECYCLE_FIXTURE")
	if path == "" {
		t.Skip("FORGE_DEVICE_CREDENTIAL_LIFECYCLE_FIXTURE is set by the contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var fixture credentialFixture
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode device credential fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("device credential fixture has trailing JSON: %v", err)
	}
	if fixture.SchemaVersion != SchemaVersion || fixture.EvaluationMode != EvaluationMode ||
		fixture.Authority.OwnerAuthenticated || fixture.Authority.CredentialMaterialMade ||
		fixture.Authority.Persisted || fixture.Authority.InventoryAuthoritative ||
		fixture.Authority.ExecutionAuthorized || len(fixture.Cases) != 6 {
		t.Fatalf("invalid device credential fixture envelope: %#v", fixture)
	}
	for _, testCase := range fixture.Cases {
		t.Run(testCase.Name, func(t *testing.T) {
			transition, err := Apply(Request{
				Current:             testCase.Current,
				Action:              testCase.Action,
				Owner:               testCase.Owner,
				DeviceID:            testCase.DeviceID,
				ApprovalState:       testCase.ApprovalState,
				CredentialID:        testCase.CredentialID,
				KeyID:               testCase.KeyID,
				PublicKeySHA256:     testCase.PublicKeySHA256,
				KeyGeneration:       testCase.KeyGeneration,
				IssuedAtMS:          testCase.IssuedAtMS,
				ExpiresAtMS:         testCase.ExpiresAtMS,
				NextCredentialID:    testCase.NextCredentialID,
				NextKeyID:           testCase.NextKeyID,
				NextPublicKeySHA256: testCase.NextPublicKeySHA256,
				ObservedAtMS:        testCase.ObservedAtMS,
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
			if !transition.PreviewOnly || !transition.Authority.OwnerBindingMatched ||
				transition.Authority.OwnerAuthenticated || transition.Authority.CredentialMaterialMade ||
				transition.Authority.Persisted || transition.Authority.InventoryAuthoritative ||
				transition.Authority.ExecutionAuthorized {
				t.Fatalf("authority boundary = %#v", transition)
			}
		})
	}
}
