package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"strings"
	"testing"
)

type runnerTerminalReceiptVectorsFixture struct {
	SchemaVersion  string                         `json:"schema_version"`
	EvaluationMode string                         `json:"evaluation_mode"`
	Authority      RunnerTerminalReceiptAuthority `json:"authority"`
	Vectors        []runnerTerminalReceiptVector  `json:"vectors"`
}

type runnerTerminalReceiptVector struct {
	Name     string                              `json:"name"`
	Grant    RunnerTerminalLeaseGrant            `json:"grant"`
	Command  RunnerTerminalCommand               `json:"command"`
	Receipt  RunnerTerminalReceipt               `json:"receipt"`
	Expected runnerTerminalReceiptVectorExpected `json:"expected"`
}

type runnerTerminalReceiptVectorExpected struct {
	CommandSHA256          string `json:"command_sha256"`
	DispositionKind        string `json:"disposition_kind"`
	ReceiptValid           bool   `json:"receipt_valid"`
	Uncertain              bool   `json:"uncertain"`
	ReconciliationRequired bool   `json:"reconciliation_required"`
	ManualReviewRequired   bool   `json:"manual_review_required"`
	AutomaticRetry         bool   `json:"automatic_retry"`
	FollowUp               string `json:"follow_up"`
}

func TestRunnerTerminalReceiptVectors(t *testing.T) {
	fixture := readRunnerTerminalReceiptVectorsFixture(t)
	if fixture.SchemaVersion != "forge.runner-terminal-receipt-vectors/v1" ||
		fixture.EvaluationMode != "pure_runner_terminal_receipt_vectors_only" ||
		!validRunnerTerminalReceiptAuthority(fixture.Authority) || len(fixture.Vectors) != 3 {
		t.Fatalf("invalid Runner terminal receipt vector envelope: %#v", fixture)
	}
	seen := map[string]bool{}
	for _, vector := range fixture.Vectors {
		if vector.Name == "" || seen[vector.Name] {
			t.Fatalf("duplicate or empty terminal receipt vector name: %q", vector.Name)
		}
		seen[vector.Name] = true
		if err := vector.Grant.Validate(); err != nil {
			t.Fatalf("vector %q grant: %v", vector.Name, err)
		}
		digest, err := vector.Command.CommandSHA256()
		if err != nil || digest != vector.Expected.CommandSHA256 || vector.Receipt.CommandSHA256 != digest {
			t.Fatalf("vector %q digest=%q expected=%q err=%v", vector.Name, digest, vector.Expected.CommandSHA256, err)
		}
		observation, err := ObserveRunnerTerminalReceipt(RunnerTerminalReceiptRequest{
			Grant: vector.Grant, Command: vector.Command, Receipt: vector.Receipt,
		})
		if err != nil {
			t.Fatalf("vector %q observation: %v", vector.Name, err)
		}
		expected := vector.Expected
		if observation.DispositionKind != expected.DispositionKind ||
			observation.ReceiptValid != expected.ReceiptValid ||
			observation.Uncertain != expected.Uncertain ||
			observation.ReconciliationRequired != expected.ReconciliationRequired ||
			observation.ManualReviewRequired != expected.ManualReviewRequired ||
			observation.AutomaticRetry != expected.AutomaticRetry ||
			observation.FollowUp != expected.FollowUp ||
			!observation.PreviewOnly || !validRunnerTerminalReceiptAuthority(observation.Authority) {
			t.Fatalf("vector %q observation=%#v expected=%#v", vector.Name, observation, expected)
		}
	}
}

func TestRunnerTerminalReceiptVectorsRejectWireDrift(t *testing.T) {
	path := os.Getenv("FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE")
	if path == "" {
		t.Skip("FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE is set by the contract script")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	mutations := map[string][]byte{
		"unknown":   append(bytes.TrimSuffix(bytes.TrimSpace(encoded), []byte("}")), []byte(`,"unexpected":true}`)...),
		"duplicate": append(bytes.TrimSuffix(bytes.TrimSpace(encoded), []byte("}")), []byte(`,"schema_version":"forge.runner-terminal-receipt-vectors/v1"}`)...),
		"trailing":  append(bytes.TrimSpace(encoded), []byte(" {}")...),
	}
	for name, mutation := range mutations {
		t.Run(name, func(t *testing.T) {
			if _, err := decodeRunnerTerminalReceiptVectorsFixture(mutation); err == nil {
				t.Fatal("wire drift was accepted")
			}
		})
	}

	fixture := readRunnerTerminalReceiptVectorsFixture(t)
	base := fixture.Vectors[0]
	base.Receipt.CommandSHA256 = strings.Repeat("b", 64)
	if _, err := ObserveRunnerTerminalReceipt(RunnerTerminalReceiptRequest{
		Grant: base.Grant, Command: base.Command, Receipt: base.Receipt,
	}); err == nil {
		t.Fatal("digest drift was accepted")
	}
	base = fixture.Vectors[2]
	base.Receipt.ObservedAtMS = base.Grant.ExpiresAtMS
	if _, err := ObserveRunnerTerminalReceipt(RunnerTerminalReceiptRequest{
		Grant: base.Grant, Command: base.Command, Receipt: base.Receipt,
	}); err == nil {
		t.Fatal("expired uncertain receipt was accepted")
	}
}

func readRunnerTerminalReceiptVectorsFixture(t *testing.T) runnerTerminalReceiptVectorsFixture {
	t.Helper()
	path := os.Getenv("FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE")
	if path == "" {
		t.Skip("FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE is set by the contract script")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	fixture, err := decodeRunnerTerminalReceiptVectorsFixture(encoded)
	if err != nil {
		t.Fatalf("decode Runner terminal receipt vectors: %v", err)
	}
	return fixture
}

func decodeRunnerTerminalReceiptVectorsFixture(encoded []byte) (runnerTerminalReceiptVectorsFixture, error) {
	if err := rejectDuplicateFields(encoded); err != nil {
		return runnerTerminalReceiptVectorsFixture{}, err
	}
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	var fixture runnerTerminalReceiptVectorsFixture
	if err := decoder.Decode(&fixture); err != nil {
		return runnerTerminalReceiptVectorsFixture{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return runnerTerminalReceiptVectorsFixture{}, errInvalidRequest
		}
		return runnerTerminalReceiptVectorsFixture{}, err
	}
	return fixture, nil
}
