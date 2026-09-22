package pendingwrite

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"reflect"
	"testing"
)

type contractFixture struct {
	SchemaVersion  string            `json:"schema_version"`
	EvaluationMode string            `json:"evaluation_mode"`
	Authority      contractAuthority `json:"authority"`
	Cases          []contractCase    `json:"cases"`
}

type contractAuthority struct {
	BodyIncluded       bool `json:"body_included"`
	RunCreated         bool `json:"run_created"`
	NetworkContacted   bool `json:"network_contacted"`
	PersistenceWritten bool `json:"persistence_written"`
}

type contractCase struct {
	Name     string              `json:"name"`
	Metadata Metadata            `json:"metadata"`
	Expected contractExpectation `json:"expected"`
}

type contractExpectation struct {
	Accepted             bool    `json:"accepted"`
	Error                string  `json:"error,omitempty"`
	Operation            string  `json:"operation,omitempty"`
	ConversationID       *string `json:"conversation_id,omitempty"`
	ExpectedVersion      *uint64 `json:"expected_version,omitempty"`
	IdempotencyKey       string  `json:"idempotency_key,omitempty"`
	State                string  `json:"state,omitempty"`
	Pending              bool    `json:"pending,omitempty"`
	Unconfirmed          bool    `json:"unconfirmed,omitempty"`
	RetryAllowed         bool    `json:"retry_allowed,omitempty"`
	SameKeyRequired      bool    `json:"same_key_required,omitempty"`
	ReconcileBeforeRetry bool    `json:"reconcile_before_retry,omitempty"`
	AttemptedAtMS        *uint64 `json:"attempted_at_ms,omitempty"`
	LastObservedAtMS     *uint64 `json:"last_observed_at_ms,omitempty"`
}

func TestRecoveryContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_PENDING_WRITE_RECOVERY_CONTRACT_FIXTURE")
	if path == "" {
		t.Skip("FORGE_PENDING_WRITE_RECOVERY_CONTRACT_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture contractFixture
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode pending write fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("pending write fixture has trailing JSON: %v", err)
	}
	if fixture.SchemaVersion != SchemaVersion || fixture.EvaluationMode != EvaluationMode ||
		fixture.Authority.BodyIncluded || fixture.Authority.RunCreated ||
		fixture.Authority.NetworkContacted || fixture.Authority.PersistenceWritten ||
		len(fixture.Cases) != 6 {
		t.Fatalf("invalid pending write fixture envelope: %#v", fixture)
	}
	for _, testCase := range fixture.Cases {
		t.Run(testCase.Name, func(t *testing.T) {
			actual, err := Project(testCase.Metadata)
			if !testCase.Expected.Accepted {
				if err == nil || err.Error() != testCase.Expected.Error {
					t.Fatalf("error=%v, want %q", err, testCase.Expected.Error)
				}
				return
			}
			if err != nil {
				t.Fatalf("metadata rejected: %v", err)
			}
			assertProjection(t, testCase.Expected, actual)
		})
	}
}

func assertProjection(t *testing.T, expected contractExpectation, actual Projection) {
	t.Helper()
	if actual.SchemaVersion != SchemaVersion || actual.EvaluationMode != EvaluationMode ||
		actual.Operation != expected.Operation || !reflect.DeepEqual(actual.ConversationID, expected.ConversationID) ||
		!reflect.DeepEqual(actual.ExpectedVersion, expected.ExpectedVersion) || actual.IdempotencyKey != expected.IdempotencyKey ||
		actual.State != expected.State || actual.Pending != expected.Pending ||
		actual.Unconfirmed != expected.Unconfirmed || actual.RetryAllowed != expected.RetryAllowed ||
		actual.SameKeyRequired != expected.SameKeyRequired ||
		actual.ReconcileBeforeRetry != expected.ReconcileBeforeRetry ||
		!reflect.DeepEqual(actual.AttemptedAtMS, expected.AttemptedAtMS) ||
		!reflect.DeepEqual(actual.LastObservedAtMS, expected.LastObservedAtMS) {
		t.Fatalf("projection=%#v, expected=%#v", actual, expected)
	}
}

func TestProjectRejectsBodyShapedMetadataByConstruction(t *testing.T) {
	metadata := Metadata{
		Operation: "append_prompt", ConversationID: stringPointer("conversation-1"),
		ExpectedVersion: uint64Pointer(1), IdempotencyKey: "key", State: "pending",
	}
	if _, err := Project(metadata); err != nil {
		t.Fatalf("valid metadata rejected: %v", err)
	}
}

func stringPointer(value string) *string { return &value }

func uint64Pointer(value uint64) *uint64 { return &value }
