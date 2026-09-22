package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"strings"
	"testing"
)

type runnerExecutionIntentFixture struct {
	SchemaVersion  string                         `json:"schema_version"`
	EvaluationMode string                         `json:"evaluation_mode"`
	Authority      RunnerExecutionIntentAuthority `json:"authority"`
	Owner          Owner                          `json:"owner"`
	ConversationID string                         `json:"conversation_id"`
	Prompt         RunIntentPromptReceipt         `json:"prompt_receipt"`
	Run            RunIntentRunReference          `json:"run_reference"`
	Binding        RunnerExecutionIntentBinding   `json:"execution_intent"`
	Command        RunnerExecutionCommand         `json:"command"`
	Expected       runnerExecutionIntentExpected  `json:"expected"`
}

type runnerExecutionIntentExpected struct {
	PromptRunBindingValid     bool                           `json:"prompt_run_binding_valid"`
	RunnerCommandBindingValid bool                           `json:"runner_command_binding_valid"`
	PreviewOnly               bool                           `json:"preview_only"`
	CommandSHA256             string                         `json:"command_sha256"`
	SelectedTargetID          *string                        `json:"selected_target_id"`
	Authority                 RunnerExecutionIntentAuthority `json:"authority"`
}

func TestRunnerExecutionIntentContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_RUNNER_EXECUTION_INTENT_CONTRACT_FIXTURE")
	if path == "" {
		t.Skip("FORGE_RUNNER_EXECUTION_INTENT_CONTRACT_FIXTURE is set by the cross-repository contract test")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	fixture := decodeRunnerExecutionIntentFixture(t, encoded)
	if fixture.SchemaVersion != RunnerExecutionIntentSchemaVersion ||
		fixture.EvaluationMode != RunnerExecutionIntentEvaluationMode || !validRunnerAuthority(fixture.Authority) ||
		fixture.ConversationID != "conversation-001" || fixture.Expected.CommandSHA256 != fixture.Binding.CommandSHA256 {
		t.Fatalf("invalid Runner execution intent fixture envelope: %#v", fixture)
	}
	observation, err := ObserveRunnerExecutionIntent(RunnerExecutionIntentRequest{
		Owner: fixture.Owner, ConversationID: fixture.ConversationID, Prompt: fixture.Prompt,
		Run: fixture.Run, Binding: fixture.Binding, Command: fixture.Command,
	})
	if err != nil {
		t.Fatalf("observe Runner execution intent: %v", err)
	}
	if observation.SchemaVersion != fixture.SchemaVersion || observation.EvaluationMode != fixture.EvaluationMode ||
		observation.Owner != fixture.Owner || observation.ConversationID != fixture.ConversationID ||
		observation.PromptID != fixture.Prompt.PromptID || observation.RunID != fixture.Run.RunID ||
		observation.AttemptID != fixture.Binding.AttemptID || observation.CommandID != fixture.Binding.CommandID ||
		observation.TargetID != fixture.Binding.TargetID || observation.CommandSHA256 != fixture.Binding.CommandSHA256 ||
		observation.IdempotencyKey != fixture.Binding.IdempotencyKey ||
		observation.PromptRunBindingValid != fixture.Expected.PromptRunBindingValid ||
		observation.RunnerCommandBindingValid != fixture.Expected.RunnerCommandBindingValid ||
		observation.PreviewOnly != fixture.Expected.PreviewOnly || observation.SelectedTargetID != nil ||
		!validRunnerAuthority(observation.Authority) {
		t.Fatalf("unexpected Runner execution intent observation: %#v", observation)
	}
}

func TestRunnerExecutionIntentRejectsConfusedBindings(t *testing.T) {
	path := os.Getenv("FORGE_RUNNER_EXECUTION_INTENT_CONTRACT_FIXTURE")
	if path == "" {
		t.Skip("FORGE_RUNNER_EXECUTION_INTENT_CONTRACT_FIXTURE is set by the cross-repository contract test")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	fixture := decodeRunnerExecutionIntentFixture(t, encoded)
	request := RunnerExecutionIntentRequest{
		Owner: fixture.Owner, ConversationID: fixture.ConversationID, Prompt: fixture.Prompt,
		Run: fixture.Run, Binding: fixture.Binding, Command: fixture.Command,
	}
	request.Binding.RunID = "run-foreign"
	if _, err := ObserveRunnerExecutionIntent(request); err == nil {
		t.Fatal("foreign Run binding was accepted")
	}
	request.Binding = fixture.Binding
	request.Command.LeaseProof.TargetID = "runner-foreign"
	if _, err := ObserveRunnerExecutionIntent(request); err == nil {
		t.Fatal("foreign command target was accepted")
	}
}

func TestRunnerExecutionIntentRejectsMismatchedCommandDigest(t *testing.T) {
	command := RunnerExecutionCommand{
		V: 1, CommandID: "command-1",
		LeaseProof: RunnerExecutionLeaseProof{
			AttemptID: "attempt-1", TargetID: "runner-1", Epoch: 1, FencingToken: "fence-1",
		},
		IdempotencyKey: "run-1:attempt-1:command-1", WorkspaceRef: "workspace-1",
		Argv: []string{"forge-task", "--prompt-ref", "prompt-1"}, TimeoutMS: 5_000, MaxOutputBytes: 65_536,
	}
	digest, err := command.commandSHA256()
	if err != nil {
		t.Fatalf("compute command digest: %v", err)
	}
	request := RunnerExecutionIntentRequest{
		Owner: Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}, ConversationID: "conversation-1",
		Prompt: RunIntentPromptReceipt{
			PromptID: "prompt-1", ConversationID: "conversation-1", Role: "user", AcceptedAtMS: 100,
			IntentID: "intent-1", InitialEventID: "event-1", InitialEventSequence: 1, InitialEventType: "submitted",
		},
		Run: RunIntentRunReference{
			RunID: "run-1", ConversationID: "conversation-1", PromptID: "prompt-1", CreatedAtMS: 100,
			LatestSequence: 1, Status: "nonterminal",
		},
		Binding: RunnerExecutionIntentBinding{
			ConversationID: "conversation-1", PromptID: "prompt-1", RunID: "run-1", AttemptID: "attempt-1",
			CommandID: "command-1", TargetID: "runner-1", CommandSHA256: strings.Repeat("0", 64),
			IdempotencyKey: "run-1:attempt-1:command-1",
		},
		Command: command,
	}
	if _, err := ObserveRunnerExecutionIntent(request); err == nil {
		t.Fatal("mismatched command digest was accepted")
	}
	request.Binding.CommandSHA256 = digest
	if _, err := ObserveRunnerExecutionIntent(request); err != nil {
		t.Fatalf("matching command digest was rejected: %v", err)
	}
}

func decodeRunnerExecutionIntentFixture(t *testing.T, encoded []byte) runnerExecutionIntentFixture {
	t.Helper()
	var fixture runnerExecutionIntentFixture
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode Runner execution intent fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		t.Fatalf("Runner execution intent fixture has trailing JSON: %v", err)
	}
	return fixture
}

func validRunnerAuthority(authority RunnerExecutionIntentAuthority) bool {
	return !authority.DeviceIdentityVerified && !authority.CommandPersisted &&
		!authority.ReservationCreated && !authority.ExecutionAuthorized &&
		!authority.DispatchPerformed && !authority.AuditPublished
}
