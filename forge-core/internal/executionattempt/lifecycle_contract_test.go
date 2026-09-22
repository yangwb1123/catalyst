package executionattempt

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"testing"

	core "forgeos/forge-core/internal/platformcorecontract"
	corestate "forgeos/forge-core/internal/platformcorecontract/state"
)

type lifecycleFixture struct {
	SchemaVersion  string                 `json:"schema_version"`
	EvaluationMode string                 `json:"evaluation_mode"`
	Authority      lifecycleAuthority     `json:"authority"`
	Cases          []lifecycleFixtureCase `json:"cases"`
}

type lifecycleAuthority struct {
	DeviceIdentityVerified bool `json:"device_identity_verified"`
	CommandPersisted       bool `json:"command_persisted"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
	AuditPublished         bool `json:"audit_published"`
}

type lifecycleFixtureCase struct {
	Name     string `json:"name"`
	From     string `json:"from"`
	To       string `json:"to"`
	Accepted bool   `json:"accepted"`
	Error    string `json:"error"`
}

func TestAttemptLifecycleContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_ATTEMPT_LIFECYCLE_FIXTURE")
	if path == "" {
		t.Skip("FORGE_ATTEMPT_LIFECYCLE_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var fixture lifecycleFixture
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode Attempt lifecycle fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("Attempt lifecycle fixture has trailing JSON: %v", err)
	}
	if fixture.SchemaVersion != "forge.attempt-lifecycle/v1" ||
		fixture.EvaluationMode != "pure_attempt_lifecycle_only" ||
		fixture.Authority.DeviceIdentityVerified || fixture.Authority.CommandPersisted ||
		fixture.Authority.ReservationCreated || fixture.Authority.ExecutionAuthorized ||
		fixture.Authority.DispatchPerformed || fixture.Authority.AuditPublished ||
		len(fixture.Cases) != 19 {
		t.Fatalf("invalid Attempt lifecycle fixture envelope: %#v", fixture)
	}
	for _, testCase := range fixture.Cases {
		t.Run(testCase.Name, func(t *testing.T) {
			value := Lifecycle{state: corestate.AttemptState(testCase.From)}
			next, err := value.Reduce(transitionForTarget(testCase.To))
			if testCase.Accepted {
				if err != nil || next.State() != corestate.AttemptState(testCase.To) {
					t.Fatalf("edge rejected or wrong state: state=%q err=%v", next.State(), err)
				}
				return
			}
			if err == nil {
				t.Fatal("edge accepted unexpectedly")
			}
			code, ok := core.RejectionCodeOf(err)
			if !ok || string(code) != testCase.Error {
				t.Fatalf("error code=%q, want %q (%v)", code, testCase.Error, err)
			}
		})
	}

	initial := NewLifecycle()
	if initial.State() != corestate.AttemptState("requested") {
		t.Fatalf("initial state=%q", initial.State())
	}
	next, err := initial.Reduce(Accept)
	if err != nil || next.State() != corestate.AttemptState("accepted") {
		t.Fatalf("initial Accept: state=%q err=%v", next.State(), err)
	}
	if initial.State() != corestate.AttemptState("requested") {
		t.Fatal("Reduce mutated the original lifecycle")
	}
}

func transitionForTarget(value string) Transition {
	switch value {
	case "accepted":
		return Accept
	case "starting":
		return BeginStarting
	case "running":
		return ObserveRunning
	case "interrupted":
		return ObserveInterrupted
	case "completed":
		return ObserveCompleted
	case "failed":
		return ObserveFailed
	case "uncertain":
		return ObserveEffectOutcomeUncertain
	default:
		return Transition("unknown")
	}
}
