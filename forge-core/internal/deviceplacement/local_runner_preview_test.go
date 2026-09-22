package deviceplacement

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"os/exec"
	"strings"
	"testing"
	"time"
)

type localRunnerPreviewFake struct {
	output   string
	exitCode int
	err      error
	calls    int
	argv     []string
	stdin    string
	timeout  time.Duration
}

// localProcessPreviewRunner is deliberately test-only. It demonstrates the
// adapter with a real direct-argv local process while keeping process access
// out of the production deviceplacement package and routes.
type localProcessPreviewRunner struct{}

func (localProcessPreviewRunner) Run(
	ctx context.Context, argv []string, stdin string, _ time.Duration,
) (string, int, error) {
	if stdin != "" {
		return "", 0, errors.New("test runner received unexpected stdin")
	}
	command := exec.CommandContext(ctx, argv[0], argv[1:]...)
	output, err := command.CombinedOutput()
	if err == nil {
		return string(output), 0, nil
	}
	var exitError *exec.ExitError
	if errors.As(err, &exitError) {
		return string(output), exitError.ExitCode(), nil
	}
	return string(output), 0, err
}

func (fake *localRunnerPreviewFake) Run(
	_ context.Context, argv []string, stdin string, timeout time.Duration,
) (string, int, error) {
	fake.calls++
	fake.argv = append([]string(nil), argv...)
	fake.stdin = stdin
	fake.timeout = timeout
	return fake.output, fake.exitCode, fake.err
}

func TestLocalRunnerPreviewAdapterBindsCommandToSessionReceipt(t *testing.T) {
	request := localRunnerPreviewRequest(t)
	fake := &localRunnerPreviewFake{output: "computed"}
	observation, err := (LocalRunnerPreviewAdapter{Executor: fake}).Execute(
		context.Background(), request,
	)
	if err != nil {
		t.Fatalf("execute local Runner preview: %v", err)
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate local Runner preview: %v", err)
	}
	if fake.calls != 1 || fake.stdin != "" || fake.timeout != 5*time.Second {
		t.Fatalf("executor call = %d stdin=%q timeout=%s", fake.calls, fake.stdin, fake.timeout)
	}
	if strings.Join(fake.argv, " ") != "forge-task --prompt-ref prompt-1" {
		t.Fatalf("executor argv = %#v", fake.argv)
	}
	if observation.DispositionKind != "completed" || observation.OutputBytes != 8 || observation.ExitCode != 0 {
		t.Fatalf("completed preview = %#v", observation)
	}
	if observation.CommandSHA256 != request.Intent.Binding.CommandSHA256 ||
		observation.SessionReceipt.ReceiptObservation.CommandSHA256 != observation.CommandSHA256 {
		t.Fatalf("command binding drifted = %#v", observation)
	}
	if observation.Authority != (LocalRunnerPreviewAuthority{}) ||
		observation.SessionReceipt.Authority != (SessionRunnerReceiptAuthority{}) {
		t.Fatalf("preview gained authority = %#v", observation)
	}
	encoded, err := json.Marshal(observation)
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Contains(encoded, []byte("computed")) {
		t.Fatalf("local output leaked into preview: %s", encoded)
	}
}

func TestLocalRunnerPreviewAdapterWithDirectArgvProcess(t *testing.T) {
	request := localRunnerPreviewRequest(t)
	request.Intent.Command.Argv = []string{"printf", "FORGE-LOCAL-OK"}
	digest, err := request.Intent.Command.commandSHA256()
	if err != nil {
		t.Fatal(err)
	}
	request.Intent.Binding.CommandSHA256 = digest
	observation, err := (LocalRunnerPreviewAdapter{Executor: localProcessPreviewRunner{}}).Execute(
		context.Background(), request,
	)
	if err != nil {
		t.Fatalf("direct argv preview: %v", err)
	}
	if observation.DispositionKind != "completed" || observation.OutputBytes != uint64(len("FORGE-LOCAL-OK")) || observation.ExitCode != 0 {
		t.Fatalf("direct argv result = %#v", observation)
	}
	if observation.SessionReceipt.ReceiptObservation.CommandSHA256 != digest {
		t.Fatalf("direct argv digest = %s, want %s", observation.SessionReceipt.ReceiptObservation.CommandSHA256, digest)
	}
}

func TestLocalRunnerPreviewAdapterKeepsFailureAndUncertaintySemantics(t *testing.T) {
	t.Run("clean nonzero exit", func(t *testing.T) {
		fake := &localRunnerPreviewFake{output: "diagnostic", exitCode: 7}
		observation, err := (LocalRunnerPreviewAdapter{Executor: fake}).Execute(
			context.Background(), localRunnerPreviewRequest(t),
		)
		if err != nil {
			t.Fatal(err)
		}
		if observation.DispositionKind != "failed" || observation.ExitCode != 7 {
			t.Fatalf("failed preview = %#v", observation)
		}
		if observation.SessionReceipt.ReceiptObservation.ReconciliationRequired {
			t.Fatal("clean nonzero exit unexpectedly requires reconciliation")
		}
	})
	t.Run("executor error", func(t *testing.T) {
		fake := &localRunnerPreviewFake{err: errors.New("transport detail must stay private")}
		observation, err := (LocalRunnerPreviewAdapter{Executor: fake}).Execute(
			context.Background(), localRunnerPreviewRequest(t),
		)
		if err != nil {
			t.Fatal(err)
		}
		receipt := observation.SessionReceipt.ReceiptObservation
		if observation.DispositionKind != "uncertain" || !receipt.Uncertain ||
			!receipt.ReconciliationRequired || receipt.FollowUp != "reconciliation_manual" {
			t.Fatalf("uncertain preview = %#v", observation)
		}
		encoded, marshalErr := json.Marshal(observation)
		if marshalErr != nil {
			t.Fatal(marshalErr)
		}
		if bytes.Contains(encoded, []byte("transport detail")) {
			t.Fatalf("executor error leaked into preview: %s", encoded)
		}
	})
}

func TestLocalRunnerPreviewAdapterRejectsLeaseBeforeExecutor(t *testing.T) {
	fake := &localRunnerPreviewFake{output: "must not run"}
	request := localRunnerPreviewRequest(t)
	request.Grant.FencingToken = "foreign-fence"
	if _, err := (LocalRunnerPreviewAdapter{Executor: fake}).Execute(context.Background(), request); err == nil {
		t.Fatal("foreign lease proof was accepted")
	}
	if fake.calls != 0 {
		t.Fatalf("executor called for rejected lease: %d", fake.calls)
	}
	request = localRunnerPreviewRequest(t)
	request.ObservedAtMS = request.Grant.ExpiresAtMS
	if _, err := (LocalRunnerPreviewAdapter{Executor: fake}).Execute(context.Background(), request); err == nil {
		t.Fatal("expired lease observation was accepted")
	}
	if fake.calls != 0 {
		t.Fatalf("executor called for expired lease: %d", fake.calls)
	}
}

func TestLocalRunnerPreviewAdapterBoundsOutputAndContext(t *testing.T) {
	request := localRunnerPreviewRequest(t)
	request.Intent.Command.MaxOutputBytes = 4
	digest, err := request.Intent.Command.commandSHA256()
	if err != nil {
		t.Fatal(err)
	}
	request.Intent.Binding.CommandSHA256 = digest
	fake := &localRunnerPreviewFake{output: "too-large"}
	observation, err := (LocalRunnerPreviewAdapter{Executor: fake}).Execute(context.Background(), request)
	if err != nil {
		t.Fatal(err)
	}
	if observation.DispositionKind != "failed" || observation.OutputBytes != 9 {
		t.Fatalf("output-limited preview = %#v", observation)
	}
	if _, err := (LocalRunnerPreviewAdapter{Executor: fake}).Execute(nil, request); !errors.Is(err, ErrLocalRunnerContextMissing) {
		t.Fatalf("nil context error = %v", err)
	}
	if _, err := (LocalRunnerPreviewAdapter{}).Execute(context.Background(), request); !errors.Is(err, ErrLocalRunnerExecutorMissing) {
		t.Fatalf("missing executor error = %v", err)
	}
}

func TestLocalRunnerPreviewObservationRejectsAuthorityMutation(t *testing.T) {
	request := localRunnerPreviewRequest(t)
	observation, err := (LocalRunnerPreviewAdapter{Executor: &localRunnerPreviewFake{output: "ok"}}).Execute(
		context.Background(), request,
	)
	if err != nil {
		t.Fatal(err)
	}
	observation.Authority.ExecutionAuthorized = true
	if err := observation.Validate(); err == nil {
		t.Fatal("authoritative local Runner preview was accepted")
	}
}

func localRunnerPreviewRequest(t *testing.T) LocalRunnerPreviewRequest {
	t.Helper()
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
		t.Fatal(err)
	}
	return LocalRunnerPreviewRequest{
		Intent: RunnerExecutionIntentRequest{
			Owner:          Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"},
			ConversationID: "conversation-1",
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
				CommandID: "command-1", TargetID: "runner-1", CommandSHA256: digest,
				IdempotencyKey: "run-1:attempt-1:command-1",
			},
			Command: command,
		},
		Grant: RunnerTerminalLeaseGrant{
			V: 1, AttemptID: "attempt-1", TargetID: "runner-1", Epoch: 1, FencingToken: "fence-1",
			IssuedAtMS: 100, ExpiresAtMS: 10_100,
		},
		ObservedAtMS: 300,
	}
}
