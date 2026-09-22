package deviceidentity

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"testing"
)

type identityFixture struct {
	SchemaVersion  string                `json:"schema_version"`
	EvaluationMode string                `json:"evaluation_mode"`
	Notice         string                `json:"notice"`
	Owner          Owner                 `json:"owner_declaration"`
	Device         fixtureDevice         `json:"device"`
	Challenge      fixtureChallenge      `json:"challenge"`
	Authority      fixtureAuthority      `json:"authority"`
	Cases          []identityFixtureCase `json:"cases"`
}

type fixtureDevice struct {
	DeviceID        string `json:"device_id"`
	Owner           Owner  `json:"owner"`
	KeyID           string `json:"key_id"`
	PublicKeySHA256 string `json:"public_key_sha256"`
	ApprovalState   string `json:"approval_state"`
	CredentialState string `json:"credential_state"`
}

type fixtureChallenge struct {
	ChallengeID     string `json:"challenge_id"`
	ChallengeSHA256 string `json:"challenge_sha256"`
	IssuedAtMS      uint64 `json:"issued_at_ms"`
	ExpiresAtMS     uint64 `json:"expires_at_ms"`
	Consumed        bool   `json:"consumed"`
}

type fixtureAuthority struct {
	IdentityVerified    bool `json:"identity_verified"`
	ChallengeConsumed   bool `json:"challenge_consumed"`
	EnrollmentPersisted bool `json:"enrollment_persisted"`
	OwnerApproval       bool `json:"owner_approval_recorded"`
	CredentialIssued    bool `json:"credential_issued"`
	InventoryAuthority  bool `json:"inventory_authoritative"`
	ExecutionAuthority  bool `json:"execution_authorized"`
}

type identityFixtureCase struct {
	Name                  string        `json:"name"`
	NowMS                 uint64        `json:"now_ms"`
	ChallengeConsumed     bool          `json:"challenge_consumed"`
	DeviceApprovalState   string        `json:"device_approval_state"`
	DeviceCredentialState string        `json:"device_credential_state"`
	Proof                 fixtureProof  `json:"proof"`
	Expected              fixtureResult `json:"expected"`
}

type fixtureProof struct {
	DeviceID        string `json:"device_id"`
	KeyID           string `json:"key_id"`
	PublicKeySHA256 string `json:"public_key_sha256"`
	Owner           Owner  `json:"owner"`
	ChallengeID     string `json:"challenge_id"`
	ChallengeSHA256 string `json:"challenge_sha256"`
	ProofSHA256     string `json:"proof_sha256"`
	IssuedAtMS      uint64 `json:"issued_at_ms"`
	ExpiresAtMS     uint64 `json:"expires_at_ms"`
}

type fixtureResult struct {
	Accepted         bool   `json:"accepted"`
	Reason           string `json:"reason"`
	IdentityBound    bool   `json:"identity_bound"`
	ApprovalRequired bool   `json:"approval_required"`
}

func TestIdentityProofContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_DEVICE_IDENTITY_CONTRACT_FIXTURE")
	if path == "" {
		t.Skip("FORGE_DEVICE_IDENTITY_CONTRACT_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture identityFixture
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode identity fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("identity fixture has trailing JSON: %v", err)
	}
	assertIdentityFixtureEnvelope(t, fixture)
	for _, testCase := range fixture.Cases {
		t.Run(testCase.Name, func(t *testing.T) {
			device := DeviceBinding{
				DeviceID: fixture.Device.DeviceID, Owner: fixture.Device.Owner,
				KeyID: fixture.Device.KeyID, PublicKeySHA256: fixture.Device.PublicKeySHA256,
				ApprovalState:   testCase.DeviceApprovalState,
				CredentialState: testCase.DeviceCredentialState,
			}
			challenge := Challenge{
				ChallengeID:     fixture.Challenge.ChallengeID,
				ChallengeSHA256: fixture.Challenge.ChallengeSHA256,
				IssuedAtMS:      fixture.Challenge.IssuedAtMS,
				ExpiresAtMS:     fixture.Challenge.ExpiresAtMS,
				Consumed:        testCase.ChallengeConsumed,
			}
			proof := Proof{
				DeviceID: testCase.Proof.DeviceID, KeyID: testCase.Proof.KeyID,
				PublicKeySHA256: testCase.Proof.PublicKeySHA256, Owner: testCase.Proof.Owner,
				ChallengeID: testCase.Proof.ChallengeID, ChallengeSHA256: testCase.Proof.ChallengeSHA256,
				ProofSHA256: testCase.Proof.ProofSHA256, IssuedAtMS: testCase.Proof.IssuedAtMS,
				ExpiresAtMS: testCase.Proof.ExpiresAtMS,
			}
			decision, err := Evaluate(fixture.Owner, device, challenge, proof, testCase.NowMS)
			assertIdentityOutcome(t, testCase.Expected, decision, err)
		})
	}
}

func assertIdentityFixtureEnvelope(t *testing.T, fixture identityFixture) {
	t.Helper()
	if fixture.SchemaVersion != SchemaVersion || fixture.EvaluationMode != EvaluationMode ||
		fixture.Owner != fixture.Device.Owner || fixture.Device.DeviceID != "device-a" ||
		fixture.Device.KeyID != "key-a" || fixture.Device.ApprovalState != "approved" ||
		fixture.Device.CredentialState != "active" || fixture.Challenge.Consumed ||
		fixture.Authority.IdentityVerified || fixture.Authority.ChallengeConsumed ||
		fixture.Authority.EnrollmentPersisted || fixture.Authority.OwnerApproval ||
		fixture.Authority.CredentialIssued || fixture.Authority.InventoryAuthority ||
		fixture.Authority.ExecutionAuthority || len(fixture.Cases) != 12 {
		t.Fatalf("invalid identity fixture envelope: %#v", fixture)
	}
}

func assertIdentityOutcome(t *testing.T, expected fixtureResult, decision Decision, err error) {
	t.Helper()
	if expected.Accepted {
		if err != nil {
			t.Fatalf("identity proof rejected: %v", err)
		}
		if decision.Reason != expected.Reason || decision.IdentityBound != expected.IdentityBound ||
			decision.ApprovalRequired != expected.ApprovalRequired {
			t.Fatalf("decision=%#v, expected=%#v", decision, expected)
		}
		return
	}
	if err == nil || err.Error() != expected.Reason {
		t.Fatalf("rejection=%v, want %q", err, expected.Reason)
	}
}
