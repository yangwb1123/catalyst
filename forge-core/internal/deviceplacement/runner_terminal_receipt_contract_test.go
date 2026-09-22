package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"testing"
)

type runnerTerminalReceiptFixture struct {
	SchemaVersion  string                         `json:"schema_version"`
	EvaluationMode string                         `json:"evaluation_mode"`
	Authority      RunnerTerminalReceiptAuthority `json:"authority"`
	Grant          RunnerTerminalLeaseGrant       `json:"grant"`
	Command        RunnerTerminalCommand          `json:"command"`
	Receipt        RunnerTerminalReceipt          `json:"receipt"`
	Expected       runnerTerminalReceiptExpected  `json:"expected"`
}

type runnerTerminalReceiptExpected struct {
	CommandSHA256 string `json:"command_sha256"`
	ReceiptValid  bool   `json:"receipt_valid"`
}

func TestRunnerTerminalReceiptContractFixture(t *testing.T) {
	fixture := readRunnerTerminalReceiptFixture(t)
	if fixture.SchemaVersion != RunnerTerminalReceiptSchemaVersion ||
		fixture.EvaluationMode != RunnerTerminalReceiptEvaluationMode ||
		!validRunnerTerminalReceiptAuthority(fixture.Authority) {
		t.Fatalf("invalid Runner terminal receipt fixture envelope: %#v", fixture)
	}
	if err := fixture.Grant.Validate(); err != nil {
		t.Fatalf("invalid Runner terminal receipt grant: %v", err)
	}
	digest, err := fixture.Command.CommandSHA256()
	if err != nil {
		t.Fatalf("command digest: %v", err)
	}
	if digest != fixture.Expected.CommandSHA256 || fixture.Receipt.CommandSHA256 != fixture.Expected.CommandSHA256 {
		t.Fatalf("command digest = %q, want %q", digest, fixture.Expected.CommandSHA256)
	}
	observation, err := ObserveRunnerTerminalReceipt(RunnerTerminalReceiptRequest{
		Grant: fixture.Grant, Command: fixture.Command, Receipt: fixture.Receipt,
	})
	if err != nil {
		t.Fatalf("observe Runner terminal receipt: %v", err)
	}
	if observation.SchemaVersion != fixture.SchemaVersion ||
		observation.EvaluationMode != fixture.EvaluationMode ||
		observation.CommandID != fixture.Receipt.CommandID ||
		observation.CommandSHA256 != fixture.Expected.CommandSHA256 ||
		observation.AttemptID != fixture.Receipt.Proof.AttemptID ||
		observation.TargetID != fixture.Receipt.Proof.TargetID ||
		observation.DispositionKind != fixture.Receipt.Disposition.Kind ||
		observation.ObservedAtMS != fixture.Receipt.ObservedAtMS ||
		observation.ReceiptValid != fixture.Expected.ReceiptValid ||
		!observation.PreviewOnly || !validRunnerTerminalReceiptAuthority(observation.Authority) {
		t.Fatalf("unexpected Runner terminal receipt observation: %#v", observation)
	}
}

func TestRunnerTerminalReceiptRejectsConfusedOrUnsafeValues(t *testing.T) {
	fixture := readRunnerTerminalReceiptFixture(t)
	base := RunnerTerminalReceiptRequest{
		Grant: fixture.Grant, Command: fixture.Command, Receipt: fixture.Receipt,
	}
	for name, mutate := range map[string]func(*RunnerTerminalReceiptRequest){
		"command digest": func(request *RunnerTerminalReceiptRequest) {
			request.Receipt.CommandSHA256 = "b" + request.Receipt.CommandSHA256[1:]
		},
		"foreign command target": func(request *RunnerTerminalReceiptRequest) {
			request.Command.LeaseProof.TargetID = "runner-foreign"
		},
		"foreign receipt proof": func(request *RunnerTerminalReceiptRequest) {
			request.Receipt.Proof.TargetID = "runner-foreign"
		},
		"foreign grant epoch": func(request *RunnerTerminalReceiptRequest) {
			request.Grant.Epoch++
		},
		"expired observation": func(request *RunnerTerminalReceiptRequest) {
			request.Receipt.ObservedAtMS = request.Grant.ExpiresAtMS
		},
		"unknown disposition": func(request *RunnerTerminalReceiptRequest) {
			request.Receipt.Disposition.Kind = "unknown"
		},
		"granted authority is not accepted as receipt": func(request *RunnerTerminalReceiptRequest) {
			request.Receipt.Disposition.ReceiptSHA256 = ""
		},
	} {
		t.Run(name, func(t *testing.T) {
			request := base
			mutate(&request)
			if _, err := ObserveRunnerTerminalReceipt(request); err == nil {
				t.Fatal("unsafe or confused terminal receipt was accepted")
			}
		})
	}
}

func TestRunnerTerminalReceiptUncertainRequiresManualReconciliation(t *testing.T) {
	fixture := readRunnerTerminalReceiptFixture(t)
	request := RunnerTerminalReceiptRequest{
		Grant: fixture.Grant, Command: fixture.Command, Receipt: fixture.Receipt,
	}
	request.Receipt.Disposition = RunnerTerminalDisposition{
		Kind:   "uncertain",
		Reason: "transport ended after effect boundary",
	}
	observation, err := ObserveRunnerTerminalReceipt(request)
	if err != nil {
		t.Fatalf("uncertain receipt: %v", err)
	}
	if !observation.Uncertain || !observation.ReconciliationRequired ||
		!observation.ManualReviewRequired || observation.AutomaticRetry ||
		observation.FollowUp != "reconciliation_manual" || !validRunnerTerminalReceiptAuthority(observation.Authority) {
		t.Fatalf("uncertain observation = %#v", observation)
	}
}

func TestRunnerTerminalReceiptFixtureRejectsUnknownFields(t *testing.T) {
	path := os.Getenv("FORGE_RUNNER_COMMAND_CONTRACT_FIXTURE")
	if path == "" {
		t.Skip("FORGE_RUNNER_COMMAND_CONTRACT_FIXTURE is set by the cross-repository contract test")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	encoded = append(bytes.TrimSpace(encoded), []byte(`{"unexpected":true}`)...)
	if _, err := decodeRunnerTerminalReceiptFixture(encoded); err == nil {
		t.Fatal("unknown fixture field was accepted")
	}
}

func readRunnerTerminalReceiptFixture(t *testing.T) runnerTerminalReceiptFixture {
	t.Helper()
	path := os.Getenv("FORGE_RUNNER_COMMAND_CONTRACT_FIXTURE")
	if path == "" {
		t.Skip("FORGE_RUNNER_COMMAND_CONTRACT_FIXTURE is set by the cross-repository contract test")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	fixture, err := decodeRunnerTerminalReceiptFixture(encoded)
	if err != nil {
		t.Fatalf("decode Runner terminal receipt fixture: %v", err)
	}
	return fixture
}

func decodeRunnerTerminalReceiptFixture(encoded []byte) (runnerTerminalReceiptFixture, error) {
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	var fixture runnerTerminalReceiptFixture
	if err := decoder.Decode(&fixture); err != nil {
		return runnerTerminalReceiptFixture{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return runnerTerminalReceiptFixture{}, errInvalidRequest
		}
		return runnerTerminalReceiptFixture{}, err
	}
	return fixture, nil
}

func validRunnerTerminalReceiptAuthority(authority RunnerTerminalReceiptAuthority) bool {
	return !authority.DeviceIdentityVerified && !authority.CommandPersisted &&
		!authority.ReservationCreated && !authority.ExecutionAuthorized &&
		!authority.DispatchPerformed && !authority.AuditPublished
}
