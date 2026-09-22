package executionlease

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"testing"
)

type leaseFixture struct {
	SchemaVersion  string             `json:"schema_version"`
	EvaluationMode string             `json:"evaluation_mode"`
	Authority      leaseAuthority     `json:"authority"`
	Grant          LeaseGrant         `json:"grant"`
	Cases          []leaseFixtureCase `json:"cases"`
}

type leaseAuthority struct {
	DeviceIdentityVerified bool `json:"device_identity_verified"`
	CommandPersisted       bool `json:"command_persisted"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
	AuditPublished         bool `json:"audit_published"`
}

type leaseFixtureCase struct {
	Name             string               `json:"name"`
	Operation        string               `json:"operation"`
	ObservedAtMS     uint64               `json:"observed_at_ms"`
	FencingToken     string               `json:"fencing_token"`
	TTLMS            uint64               `json:"ttl_ms"`
	Proof            *LeaseProof          `json:"proof"`
	Disposition      *TerminalDisposition `json:"disposition"`
	SeedDisposition  *TerminalDisposition `json:"seed_disposition"`
	SeedObservedAtMS uint64               `json:"seed_observed_at_ms"`
	Expected         leaseFixtureExpected `json:"expected"`
}

type leaseFixtureExpected struct {
	Active         *bool  `json:"active"`
	Accepted       bool   `json:"accepted"`
	Error          string `json:"error"`
	Epoch          uint64 `json:"epoch"`
	IssuedAtMS     uint64 `json:"issued_at_ms"`
	ExpiresAtMS    uint64 `json:"expires_at_ms"`
	Replayed       bool   `json:"replayed"`
	Uncertain      bool   `json:"uncertain"`
	AutomaticRetry *bool  `json:"automatic_retry"`
}

func TestLeaseFencingContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_LEASE_FENCING_FIXTURE")
	if path == "" {
		t.Skip("FORGE_LEASE_FENCING_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var fixture leaseFixture
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode lease fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("lease fixture has trailing JSON: %v", err)
	}
	if fixture.SchemaVersion != "forge.runner-lease-fencing/v1" ||
		fixture.EvaluationMode != "pure_lease_fencing_only" ||
		fixture.Authority.DeviceIdentityVerified || fixture.Authority.CommandPersisted ||
		fixture.Authority.ReservationCreated || fixture.Authority.ExecutionAuthorized ||
		fixture.Authority.DispatchPerformed || fixture.Authority.AuditPublished ||
		len(fixture.Cases) != 16 {
		t.Fatalf("invalid lease fixture envelope: %#v", fixture)
	}
	if err := fixture.Grant.Validate(); err != nil {
		t.Fatalf("fixture grant invalid: %v", err)
	}
	for _, testCase := range fixture.Cases {
		t.Run(testCase.Name, func(t *testing.T) {
			runLeaseFixtureCase(t, fixture.Grant, testCase)
		})
	}
}

func runLeaseFixtureCase(t *testing.T, grant LeaseGrant, testCase leaseFixtureCase) {
	t.Helper()
	switch testCase.Operation {
	case "active":
		if testCase.Expected.Active == nil || grant.IsActive(testCase.ObservedAtMS) != *testCase.Expected.Active {
			t.Fatalf("active=%v, want %v", grant.IsActive(testCase.ObservedAtMS), testCase.Expected.Active)
		}
	case "renew":
		result, err := grant.Renew(testCase.ObservedAtMS, testCase.FencingToken, testCase.TTLMS)
		assertLeaseResult(t, testCase.Expected, err)
		if err == nil && (result.Epoch != testCase.Expected.Epoch || result.IssuedAtMS != testCase.Expected.IssuedAtMS || result.ExpiresAtMS != testCase.Expected.ExpiresAtMS) {
			t.Fatalf("renewed grant=%#v, want epoch=%d issued=%d expires=%d", result, testCase.Expected.Epoch, testCase.Expected.IssuedAtMS, testCase.Expected.ExpiresAtMS)
		}
	case "proof":
		if testCase.Proof == nil {
			t.Fatal("proof case has no proof")
		}
		assertLeaseResult(t, testCase.Expected, grant.ValidateProof(*testCase.Proof, testCase.ObservedAtMS))
	case "terminal", "terminal_replay", "terminal_conflict":
		state := newFixtureLeaseState(t, grant, testCase)
		if testCase.Disposition == nil {
			t.Fatal("terminal case has no disposition")
		}
		result, err := state.SubmitTerminal(grant.Proof(), *testCase.Disposition, testCase.ObservedAtMS)
		assertLeaseResult(t, testCase.Expected, err)
		if err == nil {
			if result.Replayed != testCase.Expected.Replayed || result.Receipt.Disposition.IsUncertain() != testCase.Expected.Uncertain {
				t.Fatalf("terminal result=%#v, want replayed=%v uncertain=%v", result, testCase.Expected.Replayed, testCase.Expected.Uncertain)
			}
			if testCase.Expected.AutomaticRetry != nil && *testCase.Expected.AutomaticRetry {
				t.Fatal("fixture must never enable automatic retry")
			}
		}
	case "renew_after_terminal":
		state := newFixtureLeaseState(t, grant, testCase)
		assertLeaseResult(t, testCase.Expected, state.Renew(testCase.ObservedAtMS, testCase.FencingToken, testCase.TTLMS))
	default:
		t.Fatalf("unsupported operation %q", testCase.Operation)
	}
}

func newFixtureLeaseState(t *testing.T, grant LeaseGrant, testCase leaseFixtureCase) *LeaseState {
	t.Helper()
	state, err := NewState(grant)
	if err != nil {
		t.Fatalf("new fixture state: %v", err)
	}
	if testCase.SeedDisposition != nil {
		if _, err := state.SubmitTerminal(grant.Proof(), *testCase.SeedDisposition, testCase.SeedObservedAtMS); err != nil {
			t.Fatalf("seed terminal: %v", err)
		}
	}
	return state
}

func assertLeaseResult(t *testing.T, expected leaseFixtureExpected, err error) {
	t.Helper()
	if expected.Accepted {
		if err != nil {
			t.Fatalf("error=%v, want accepted", err)
		}
		return
	}
	if err == nil || err.Error() != expected.Error {
		t.Fatalf("error=%v, want %q", err, expected.Error)
	}
}
