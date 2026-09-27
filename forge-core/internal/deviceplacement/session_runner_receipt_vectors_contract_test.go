package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"strings"
	"testing"
)

type sessionRunnerReceiptVectorsFixture struct {
	SchemaVersion  string                        `json:"schema_version"`
	EvaluationMode string                        `json:"evaluation_mode"`
	Authority      SessionRunnerReceiptAuthority `json:"authority"`
	Vectors        []sessionRunnerReceiptVector  `json:"vectors"`
}

type sessionRunnerReceiptVector struct {
	Name        string                             `json:"name"`
	Observation SessionRunnerReceiptObservation    `json:"observation"`
	Expected    sessionRunnerReceiptVectorExpected `json:"expected"`
}

type sessionRunnerReceiptVectorExpected struct {
	CommandID              string `json:"command_id"`
	CommandSHA256          string `json:"command_sha256"`
	AttemptID              string `json:"attempt_id"`
	TargetID               string `json:"target_id"`
	DispositionKind        string `json:"disposition_kind"`
	ObservedAtMS           uint64 `json:"observed_at_ms"`
	Uncertain              bool   `json:"uncertain"`
	ReconciliationRequired bool   `json:"reconciliation_required"`
	ManualReviewRequired   bool   `json:"manual_review_required"`
	AutomaticRetry         bool   `json:"automatic_retry"`
	FollowUp               string `json:"follow_up"`
}

func TestSessionRunnerReceiptVectors(t *testing.T) {
	fixture := readSessionRunnerReceiptVectorsFixture(t)
	if fixture.SchemaVersion != "forge.session-runner-receipt-vectors/v1" ||
		fixture.EvaluationMode != "pure_session_runner_receipt_vectors_only" ||
		fixture.Authority != (SessionRunnerReceiptAuthority{}) || len(fixture.Vectors) != 3 {
		t.Fatalf("invalid session Runner receipt vectors envelope: %#v", fixture)
	}
	seen := map[string]bool{}
	for _, vector := range fixture.Vectors {
		if vector.Name == "" || seen[vector.Name] {
			t.Fatalf("duplicate or empty session Runner receipt vector name: %q", vector.Name)
		}
		seen[vector.Name] = true
		if err := vector.Observation.Validate(); err != nil {
			t.Fatalf("vector %q observation: %v", vector.Name, err)
		}
		if err := validateSessionRunnerReceiptVector(vector); err != nil {
			t.Fatalf("vector %q: %v", vector.Name, err)
		}
	}
}

func TestSessionRunnerReceiptVectorsRejectWireAndOutcomeDrift(t *testing.T) {
	path := os.Getenv("FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE")
	if path == "" {
		t.Skip("FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE is set by the contract script")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	mutations := map[string][]byte{
		"unknown":   append(bytes.TrimSuffix(bytes.TrimSpace(encoded), []byte("}")), []byte(`,"unexpected":true}`)...),
		"duplicate": append(bytes.TrimSuffix(bytes.TrimSpace(encoded), []byte("}")), []byte(`,"schema_version":"forge.session-runner-receipt-vectors/v1"}`)...),
		"trailing":  append(bytes.TrimSpace(encoded), []byte(" {}")...),
	}
	for name, mutation := range mutations {
		t.Run(name, func(t *testing.T) {
			if _, err := decodeSessionRunnerReceiptVectorsFixture(mutation); err == nil {
				t.Fatal("wire drift was accepted")
			}
		})
	}

	fixture := readSessionRunnerReceiptVectorsFixture(t)
	base := fixture.Vectors[0]
	base.Observation.ReceiptObservation.CommandID = "command-foreign"
	if err := validateSessionRunnerReceiptVector(base); err == nil {
		t.Fatal("command binding drift was accepted")
	}
	base = fixture.Vectors[2]
	base.Observation.ReceiptObservation.AutomaticRetry = true
	if err := base.Observation.Validate(); err == nil {
		t.Fatal("uncertain automatic retry was accepted")
	}
	base = fixture.Vectors[2]
	base.Observation.SelectedTargetID = sessionRunnerReceiptStringPtr("runner-1")
	if err := base.Observation.Validate(); err == nil {
		t.Fatal("selected target was accepted")
	}
}

func validateSessionRunnerReceiptVector(vector sessionRunnerReceiptVector) error {
	receipt := vector.Observation.ReceiptObservation
	expected := vector.Expected
	if receipt.CommandID != expected.CommandID ||
		receipt.CommandSHA256 != expected.CommandSHA256 ||
		receipt.AttemptID != expected.AttemptID ||
		receipt.TargetID != expected.TargetID ||
		receipt.DispositionKind != expected.DispositionKind ||
		receipt.ObservedAtMS != expected.ObservedAtMS ||
		receipt.Uncertain != expected.Uncertain ||
		receipt.ReconciliationRequired != expected.ReconciliationRequired ||
		receipt.ManualReviewRequired != expected.ManualReviewRequired ||
		receipt.AutomaticRetry != expected.AutomaticRetry ||
		receipt.FollowUp != expected.FollowUp ||
		!validSessionRunnerReceiptVectorDigest(receipt.CommandSHA256) {
		return errInvalidRequest
	}
	return nil
}

func readSessionRunnerReceiptVectorsFixture(t *testing.T) sessionRunnerReceiptVectorsFixture {
	t.Helper()
	path := os.Getenv("FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE")
	if path == "" {
		t.Skip("FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE is set by the contract script")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	fixture, err := decodeSessionRunnerReceiptVectorsFixture(encoded)
	if err != nil {
		t.Fatalf("decode session Runner receipt vectors: %v", err)
	}
	return fixture
}

func decodeSessionRunnerReceiptVectorsFixture(encoded []byte) (sessionRunnerReceiptVectorsFixture, error) {
	if err := rejectDuplicateFields(encoded); err != nil {
		return sessionRunnerReceiptVectorsFixture{}, err
	}
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	var fixture sessionRunnerReceiptVectorsFixture
	if err := decoder.Decode(&fixture); err != nil {
		return sessionRunnerReceiptVectorsFixture{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return sessionRunnerReceiptVectorsFixture{}, errInvalidRequest
		}
		return sessionRunnerReceiptVectorsFixture{}, err
	}
	return fixture, nil
}

func validSessionRunnerReceiptVectorDigest(value string) bool {
	if len(value) != 64 {
		return false
	}
	return strings.Trim(value, "0123456789abcdef") == ""
}

func sessionRunnerReceiptStringPtr(value string) *string { return &value }
