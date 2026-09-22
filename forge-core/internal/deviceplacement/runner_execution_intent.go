package deviceplacement

import "strings"

// RunnerExecutionIntentSchemaVersion identifies a pure binding of a Prompt
// and Run reference to one future Runner command declaration.
const RunnerExecutionIntentSchemaVersion = "forge.runner-execution-intent/v1"

const (
	RunnerExecutionIntentEvaluationMode = "pure_runner_binding_only"
	runnerCommandABI                    = uint16(1)
	runnerCommandMaxArguments           = 64
	runnerCommandMaxArgumentBytes       = 4096
	runnerCommandMaxArgumentTotalBytes  = 65536
	runnerCommandMaxTimeoutMS           = uint64(600000)
	runnerCommandMaxOutputBytes         = uint64(8 * 1024 * 1024)
)

// RunnerExecutionLeaseProof is the caller-declared proof carried by a
// RunnerCommand. It is checked for exact equality only; this package does not
// verify a lease or issue execution authority.
type RunnerExecutionLeaseProof struct {
	AttemptID    string `json:"attempt_id"`
	TargetID     string `json:"target_id"`
	Epoch        uint64 `json:"epoch"`
	FencingToken string `json:"fencing_token"`
}

// RunnerExecutionCommand is the bounded direct-argv command declaration that
// will eventually be understood by the Runtime Runner ABI.
type RunnerExecutionCommand struct {
	V              uint16                    `json:"v"`
	CommandID      string                    `json:"command_id"`
	LeaseProof     RunnerExecutionLeaseProof `json:"lease_proof"`
	IdempotencyKey string                    `json:"idempotency_key"`
	WorkspaceRef   string                    `json:"workspace_ref"`
	Argv           []string                  `json:"argv"`
	TimeoutMS      uint64                    `json:"timeout_ms"`
	MaxOutputBytes uint64                    `json:"max_output_bytes"`
}

// RunnerExecutionIntentBinding repeats every cross-aggregate identity at the
// handoff. Repetition is deliberate: a receiver can reject a confused Run or
// command without loading another aggregate.
type RunnerExecutionIntentBinding struct {
	ConversationID   string  `json:"conversation_id"`
	PromptID         string  `json:"prompt_id"`
	RunID            string  `json:"run_id"`
	AttemptID        string  `json:"attempt_id"`
	CommandID        string  `json:"command_id"`
	TargetID         string  `json:"target_id"`
	CommandSHA256    string  `json:"command_sha256"`
	IdempotencyKey   string  `json:"idempotency_key"`
	SelectedTargetID *string `json:"selected_target_id"`
}

// RunnerExecutionIntentRequest is a value-only request. Prompt and Run are
// existing references; this function never creates or starts either one.
type RunnerExecutionIntentRequest struct {
	Owner          Owner                        `json:"owner"`
	ConversationID string                       `json:"conversation_id"`
	Prompt         RunIntentPromptReceipt       `json:"prompt_receipt"`
	Run            RunIntentRunReference        `json:"run_reference"`
	Binding        RunnerExecutionIntentBinding `json:"execution_intent"`
	Command        RunnerExecutionCommand       `json:"command"`
}

// RunnerExecutionIntentAuthority is intentionally all false. It prevents a
// value-level binding from being mistaken for lease, inventory, or dispatch
// authority by a client or an operator.
type RunnerExecutionIntentAuthority struct {
	DeviceIdentityVerified bool `json:"device_identity_verified"`
	CommandPersisted       bool `json:"command_persisted"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
	AuditPublished         bool `json:"audit_published"`
}

// RunnerExecutionIntentObservation is a payload-free, preview-only binding
// suitable for the future execution boundary.
type RunnerExecutionIntentObservation struct {
	SchemaVersion             string                         `json:"schema_version"`
	EvaluationMode            string                         `json:"evaluation_mode"`
	Owner                     Owner                          `json:"owner"`
	ConversationID            string                         `json:"conversation_id"`
	PromptID                  string                         `json:"prompt_id"`
	RunID                     string                         `json:"run_id"`
	AttemptID                 string                         `json:"attempt_id"`
	CommandID                 string                         `json:"command_id"`
	TargetID                  string                         `json:"target_id"`
	CommandSHA256             string                         `json:"command_sha256"`
	IdempotencyKey            string                         `json:"idempotency_key"`
	PromptRunBindingValid     bool                           `json:"prompt_run_binding_valid"`
	RunnerCommandBindingValid bool                           `json:"runner_command_binding_valid"`
	PreviewOnly               bool                           `json:"preview_only"`
	SelectedTargetID          *string                        `json:"selected_target_id"`
	Authority                 RunnerExecutionIntentAuthority `json:"authority"`
}

// ObserveRunnerExecutionIntent binds Prompt, Run, and Runner command
// identities without selecting, reserving, authorizing, dispatching, or
// executing a target. All timestamps and lease state remain caller supplied.
func ObserveRunnerExecutionIntent(input RunnerExecutionIntentRequest) (RunnerExecutionIntentObservation, error) {
	if !validOwner(input.Owner) || !validSessionIdentifier(input.ConversationID) ||
		!validPromptReceipt(input.Prompt, input.ConversationID) ||
		!validRunReference(input.Run, input.Prompt, input.ConversationID) ||
		!validRunnerBinding(input.Binding, input.Command, input.ConversationID, input.Prompt, input.Run) {
		return RunnerExecutionIntentObservation{}, errInvalidRequest
	}
	return RunnerExecutionIntentObservation{
		SchemaVersion: RunnerExecutionIntentSchemaVersion, EvaluationMode: RunnerExecutionIntentEvaluationMode,
		Owner: input.Owner, ConversationID: input.ConversationID, PromptID: input.Prompt.PromptID,
		RunID: input.Run.RunID, AttemptID: input.Binding.AttemptID, CommandID: input.Binding.CommandID,
		TargetID: input.Binding.TargetID, CommandSHA256: input.Binding.CommandSHA256,
		IdempotencyKey: input.Binding.IdempotencyKey, PromptRunBindingValid: true,
		RunnerCommandBindingValid: true, PreviewOnly: true, SelectedTargetID: nil,
		Authority: RunnerExecutionIntentAuthority{},
	}, nil
}

func validRunnerBinding(binding RunnerExecutionIntentBinding, command RunnerExecutionCommand, conversationID string, prompt RunIntentPromptReceipt, run RunIntentRunReference) bool {
	if binding.ConversationID != conversationID || binding.PromptID != prompt.PromptID || binding.RunID != run.RunID ||
		binding.SelectedTargetID != nil || !validSessionIdentifier(binding.AttemptID) ||
		!validSessionIdentifier(binding.CommandID) || !validSessionIdentifier(binding.TargetID) ||
		!validRunnerDigest(binding.CommandSHA256) ||
		binding.IdempotencyKey != run.RunID+":"+binding.AttemptID+":"+binding.CommandID {
		return false
	}
	if command.V != runnerCommandABI || command.CommandID != binding.CommandID ||
		command.LeaseProof.AttemptID != binding.AttemptID || command.LeaseProof.TargetID != binding.TargetID ||
		command.LeaseProof.Epoch == 0 || !validRunnerText(command.LeaseProof.FencingToken, MaxTokenBytes, false) ||
		strings.TrimSpace(command.LeaseProof.FencingToken) != command.LeaseProof.FencingToken ||
		command.IdempotencyKey != binding.IdempotencyKey ||
		!validRunnerText(command.WorkspaceRef, 256, false) || strings.TrimSpace(command.WorkspaceRef) != command.WorkspaceRef ||
		len(command.Argv) == 0 || len(command.Argv) > runnerCommandMaxArguments ||
		command.TimeoutMS == 0 || command.TimeoutMS > runnerCommandMaxTimeoutMS || command.MaxOutputBytes == 0 || command.MaxOutputBytes > runnerCommandMaxOutputBytes {
		return false
	}
	commandSHA256, err := command.commandSHA256()
	if err != nil || commandSHA256 != binding.CommandSHA256 {
		return false
	}
	total := 0
	for index, argument := range command.Argv {
		if !validRunnerText(argument, runnerCommandMaxArgumentBytes, index != 0) || total > runnerCommandMaxArgumentTotalBytes-len(argument) {
			return false
		}
		total += len(argument)
	}
	return true
}

// commandSHA256 uses the same canonical Runner command bytes as the terminal
// receipt contract. The execution-intent boundary carries the digest as a
// repeated identity, so it must be tied to the supplied argv before the
// binding can be observed.
func (command RunnerExecutionCommand) commandSHA256() (string, error) {
	return (RunnerTerminalCommand{
		V:         command.V,
		CommandID: command.CommandID,
		LeaseProof: RunnerTerminalLeaseProof{
			AttemptID:    command.LeaseProof.AttemptID,
			TargetID:     command.LeaseProof.TargetID,
			Epoch:        command.LeaseProof.Epoch,
			FencingToken: command.LeaseProof.FencingToken,
		},
		IdempotencyKey: command.IdempotencyKey,
		WorkspaceRef:   command.WorkspaceRef,
		Argv:           command.Argv,
		TimeoutMS:      command.TimeoutMS,
		MaxOutputBytes: command.MaxOutputBytes,
	}).CommandSHA256()
}

func validRunnerDigest(value string) bool {
	if len(value) != 64 {
		return false
	}
	for _, character := range value {
		if !strings.ContainsRune("0123456789abcdef", character) {
			return false
		}
	}
	return true
}

func validRunnerText(value string, maximum int, allowEmpty bool) bool {
	if (!allowEmpty && value == "") || len(value) > maximum {
		return false
	}
	for _, character := range value {
		if character <= 0x1f || (character >= 0x7f && character <= 0x9f) {
			return false
		}
	}
	return true
}
