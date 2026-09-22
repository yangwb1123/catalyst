package deviceplacement

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"strconv"
	"time"
)

// LocalRunnerPreviewSchemaVersion identifies the explicit local adapter
// boundary. It is a test/local integration value, not a production execution
// API.
const LocalRunnerPreviewSchemaVersion = "forge.runner-local-execution-preview/v1"

// LocalRunnerPreviewEvaluationMode makes the adapter's non-authoritative
// nature visible to every consumer.
const LocalRunnerPreviewEvaluationMode = "injected_local_runner_preview_only"

var (
	// ErrLocalRunnerExecutorMissing is returned before a callback can be
	// invoked when the adapter was not explicitly wired.
	ErrLocalRunnerExecutorMissing = errors.New("local Runner preview executor is not configured")
	// ErrLocalRunnerContextMissing prevents an accidental nil context from
	// reaching an injected runner implementation.
	ErrLocalRunnerContextMissing = errors.New("local Runner preview context is nil")
)

// LocalRunnerExecutor is the only effectful seam in this adapter. Production
// routing never constructs one. Tests may inject a sandbox.Runner-shaped
// implementation (or a deterministic fake) to prove command-to-receipt
// binding without granting device or dispatch authority here.
type LocalRunnerExecutor interface {
	Run(context.Context, []string, string, time.Duration) (output string, exitCode int, err error)
}

// LocalRunnerPreviewRequest supplies already value-validated intent and a
// caller-supplied lease observation. The adapter does not issue, renew, or
// persist the lease and reads no clock; ObservedAtMS is explicit input.
type LocalRunnerPreviewRequest struct {
	Intent       RunnerExecutionIntentRequest `json:"intent"`
	Grant        RunnerTerminalLeaseGrant     `json:"grant"`
	ObservedAtMS uint64                       `json:"observed_at_ms"`
}

// LocalRunnerPreviewAdapter is deliberately a small, explicit local/test
// boundary. It invokes only the injected executor and converts its bounded
// result into the existing terminal/session receipt observations. It never
// discovers or selects a device, opens transport, persists a command/receipt,
// or publishes Audit.
type LocalRunnerPreviewAdapter struct {
	Executor LocalRunnerExecutor
}

// LocalRunnerPreviewAuthority enumerates capabilities deliberately absent
// from this preview. All fields are fixed false by Execute.
type LocalRunnerPreviewAuthority struct {
	DeviceIdentityVerified bool `json:"device_identity_verified"`
	CommandPersisted       bool `json:"command_persisted"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
	AuditPublished         bool `json:"audit_published"`
}

// LocalRunnerPreviewObservation is content-free metadata connecting one
// injected executor call to the existing Runner intent and session receipt
// contracts. Output bytes are counted and hashed only for the terminal
// receipt's opaque digest; output content is never returned.
type LocalRunnerPreviewObservation struct {
	SchemaVersion   string                           `json:"schema_version"`
	EvaluationMode  string                           `json:"evaluation_mode"`
	Intent          RunnerExecutionIntentObservation `json:"runner_execution_intent"`
	SessionReceipt  SessionRunnerReceiptObservation  `json:"session_runner_receipt"`
	CommandID       string                           `json:"command_id"`
	AttemptID       string                           `json:"attempt_id"`
	TargetID        string                           `json:"target_id"`
	CommandSHA256   string                           `json:"command_sha256"`
	DispositionKind string                           `json:"disposition_kind"`
	ObservedAtMS    uint64                           `json:"observed_at_ms"`
	OutputBytes     uint64                           `json:"output_bytes"`
	ExitCode        int                              `json:"exit_code"`
	ExecutorInvoked bool                             `json:"executor_invoked"`
	PreviewOnly     bool                             `json:"preview_only"`
	Authority       LocalRunnerPreviewAuthority      `json:"authority"`
}

// Execute invokes the explicitly injected local executor and returns a
// metadata-only observation. The caller supplies observation time so this
// method remains deterministic and cannot silently obtain authority from a
// local wall clock.
func (adapter LocalRunnerPreviewAdapter) Execute(
	ctx context.Context,
	request LocalRunnerPreviewRequest,
) (LocalRunnerPreviewObservation, error) {
	if adapter.Executor == nil {
		return LocalRunnerPreviewObservation{}, ErrLocalRunnerExecutorMissing
	}
	if ctx == nil {
		return LocalRunnerPreviewObservation{}, ErrLocalRunnerContextMissing
	}
	if request.ObservedAtMS > uint64(MaxSafeIntegerMS) || request.ObservedAtMS == 0 {
		return LocalRunnerPreviewObservation{}, errInvalidRequest
	}
	intent, err := ObserveRunnerExecutionIntent(request.Intent)
	if err != nil {
		return LocalRunnerPreviewObservation{}, err
	}
	if err := request.Grant.Validate(); err != nil {
		return LocalRunnerPreviewObservation{}, err
	}
	if !sameRunnerLeaseProof(request.Intent.Command.LeaseProof, request.Grant) {
		return LocalRunnerPreviewObservation{}, errInvalidRequest
	}
	if request.ObservedAtMS < request.Grant.IssuedAtMS ||
		request.ObservedAtMS >= request.Grant.ExpiresAtMS {
		return LocalRunnerPreviewObservation{}, errInvalidRequest
	}

	output, exitCode, runErr := adapter.Executor.Run(
		ctx,
		append([]string(nil), request.Intent.Command.Argv...),
		"",
		time.Duration(request.Intent.Command.TimeoutMS)*time.Millisecond,
	)
	disposition := runnerPreviewDisposition(output, exitCode, runErr, request.Intent.Command.MaxOutputBytes)
	receipt := RunnerTerminalReceipt{
		V:             1,
		CommandID:     request.Intent.Command.CommandID,
		CommandSHA256: request.Intent.Binding.CommandSHA256,
		Proof: RunnerTerminalLeaseProof{
			AttemptID:    request.Intent.Command.LeaseProof.AttemptID,
			TargetID:     request.Intent.Command.LeaseProof.TargetID,
			Epoch:        request.Intent.Command.LeaseProof.Epoch,
			FencingToken: request.Intent.Command.LeaseProof.FencingToken,
		},
		Disposition:  disposition,
		ObservedAtMS: request.ObservedAtMS,
	}
	terminalCommand := terminalCommandFromExecution(request.Intent.Command)
	terminal, err := ObserveRunnerTerminalReceipt(RunnerTerminalReceiptRequest{
		Grant: request.Grant, Command: terminalCommand, Receipt: receipt,
	})
	if err != nil {
		return LocalRunnerPreviewObservation{}, err
	}
	session, err := ObserveSessionRunnerReceipt(SessionRunnerReceiptObservationRequest{
		Owner:          intent.Owner,
		ConversationID: intent.ConversationID,
		PromptID:       intent.PromptID,
		RunID:          intent.RunID,
		Intent:         intent,
		Receipt:        terminal,
	})
	if err != nil {
		return LocalRunnerPreviewObservation{}, err
	}
	return LocalRunnerPreviewObservation{
		SchemaVersion:   LocalRunnerPreviewSchemaVersion,
		EvaluationMode:  LocalRunnerPreviewEvaluationMode,
		Intent:          intent,
		SessionReceipt:  session,
		CommandID:       intent.CommandID,
		AttemptID:       intent.AttemptID,
		TargetID:        intent.TargetID,
		CommandSHA256:   intent.CommandSHA256,
		DispositionKind: terminal.DispositionKind,
		ObservedAtMS:    request.ObservedAtMS,
		OutputBytes:     uint64(len(output)),
		ExitCode:        exitCode,
		ExecutorInvoked: true,
		PreviewOnly:     true,
		Authority:       LocalRunnerPreviewAuthority{},
	}, nil
}

// Validate checks a returned preview after transport or JSON decoding. It
// remains intentionally structural: this value does not become an execution
// receipt or a device authority token.
func (observation LocalRunnerPreviewObservation) Validate() error {
	if observation.SchemaVersion != LocalRunnerPreviewSchemaVersion ||
		observation.EvaluationMode != LocalRunnerPreviewEvaluationMode ||
		!observation.ExecutorInvoked || !observation.PreviewOnly ||
		observation.Authority != (LocalRunnerPreviewAuthority{}) ||
		observation.CommandID != observation.Intent.CommandID ||
		observation.AttemptID != observation.Intent.AttemptID ||
		observation.TargetID != observation.Intent.TargetID ||
		observation.CommandSHA256 != observation.Intent.CommandSHA256 ||
		observation.DispositionKind != observation.SessionReceipt.ReceiptObservation.DispositionKind ||
		observation.ObservedAtMS != observation.SessionReceipt.ReceiptObservation.ObservedAtMS ||
		observation.ObservedAtMS == 0 || observation.ObservedAtMS > uint64(MaxSafeIntegerMS) {
		return errInvalidRequest
	}
	if err := observation.SessionReceipt.Validate(); err != nil {
		return err
	}
	if observation.Intent.SelectedTargetID != nil || observation.SessionReceipt.SelectedTargetID != nil {
		return errInvalidRequest
	}
	if !sameSessionRunnerIntent(observation.SessionReceipt, observation.Intent) {
		return errInvalidRequest
	}
	return nil
}

// sameSessionRunnerIntent only confirms that this adapter's nested session
// observation repeats the same value identities as the nested intent
// observation.
func sameSessionRunnerIntent(observation SessionRunnerReceiptObservation, intent RunnerExecutionIntentObservation) bool {
	if observation.Owner != intent.Owner || observation.ConversationID != intent.ConversationID ||
		observation.PromptID != intent.PromptID || observation.RunID != intent.RunID ||
		observation.ReceiptObservation.CommandID != intent.CommandID ||
		observation.ReceiptObservation.AttemptID != intent.AttemptID ||
		observation.ReceiptObservation.TargetID != intent.TargetID ||
		observation.ReceiptObservation.CommandSHA256 != intent.CommandSHA256 {
		return false
	}
	return true
}

func sameRunnerLeaseProof(proof RunnerExecutionLeaseProof, grant RunnerTerminalLeaseGrant) bool {
	return proof.AttemptID == grant.AttemptID && proof.TargetID == grant.TargetID &&
		proof.Epoch == grant.Epoch && proof.FencingToken == grant.FencingToken
}

func terminalCommandFromExecution(command RunnerExecutionCommand) RunnerTerminalCommand {
	return RunnerTerminalCommand{
		V: command.V, CommandID: command.CommandID,
		LeaseProof: RunnerTerminalLeaseProof{
			AttemptID: command.LeaseProof.AttemptID, TargetID: command.LeaseProof.TargetID,
			Epoch: command.LeaseProof.Epoch, FencingToken: command.LeaseProof.FencingToken,
		},
		IdempotencyKey: command.IdempotencyKey, WorkspaceRef: command.WorkspaceRef,
		Argv: append([]string(nil), command.Argv...), TimeoutMS: command.TimeoutMS,
		MaxOutputBytes: command.MaxOutputBytes,
	}
}

func runnerPreviewDisposition(output string, exitCode int, runErr error, maxOutputBytes uint64) RunnerTerminalDisposition {
	if runErr != nil {
		return RunnerTerminalDisposition{Kind: "uncertain", Reason: "runner_effect_outcome_uncertain"}
	}
	if uint64(len(output)) > maxOutputBytes {
		return RunnerTerminalDisposition{Kind: "failed", Reason: "output_limit_exceeded"}
	}
	if exitCode != 0 {
		return RunnerTerminalDisposition{Kind: "failed", Reason: "exit_code_" + strconv.Itoa(exitCode)}
	}
	digest := sha256.Sum256([]byte(output))
	return RunnerTerminalDisposition{Kind: "completed", ReceiptSHA256: hex.EncodeToString(digest[:])}
}
