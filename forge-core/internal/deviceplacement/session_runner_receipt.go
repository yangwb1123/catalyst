package deviceplacement

// SessionRunnerReceiptObservationSchemaVersion identifies the metadata-only
// bridge that binds one terminal receipt observation to an existing
// Conversation/Prompt/Run observation.
const SessionRunnerReceiptObservationSchemaVersion = "forge.session-runner-receipt-observation/v1"

const SessionRunnerReceiptObservationEvaluationMode = "pure_session_runner_receipt_binding_only"

// SessionRunnerReceiptAuthority makes the absence of session or Runner
// authority explicit. A valid observation never proves identity, persists a
// receipt, authorizes execution, dispatches work, or publishes audit.
type SessionRunnerReceiptAuthority struct {
	IdentityVerified    bool `json:"identity_verified"`
	ReceiptPersisted    bool `json:"receipt_persisted"`
	ExecutionAuthorized bool `json:"execution_authorized"`
	DispatchPerformed   bool `json:"dispatch_performed"`
	AuditPublished      bool `json:"audit_published"`
}

// SessionRunnerReceiptObservationRequest joins an already validated pure
// Runner execution-intent observation to an already validated terminal
// receipt observation. Neither value is loaded from a service here.
type SessionRunnerReceiptObservationRequest struct {
	Owner          Owner                            `json:"owner"`
	ConversationID string                           `json:"conversation_id"`
	PromptID       string                           `json:"prompt_id"`
	RunID          string                           `json:"run_id"`
	Intent         RunnerExecutionIntentObservation `json:"runner_execution_intent"`
	Receipt        RunnerTerminalReceiptObservation `json:"receipt_observation"`
}

// SessionRunnerReceiptObservation is a payload-free, preview-only bridge for
// clients that need to display terminal Runner evidence alongside a session.
// The nested receipt deliberately contains no argv, output, lease grant, or
// transport data.
type SessionRunnerReceiptObservation struct {
	SchemaVersion         string                           `json:"schema_version"`
	EvaluationMode        string                           `json:"evaluation_mode"`
	Owner                 Owner                            `json:"owner"`
	ConversationID        string                           `json:"conversation_id"`
	PromptID              string                           `json:"prompt_id"`
	RunID                 string                           `json:"run_id"`
	ReceiptObservation    RunnerTerminalReceiptObservation `json:"receipt_observation"`
	PromptRunBindingValid bool                             `json:"prompt_run_binding_valid"`
	ReceiptBindingValid   bool                             `json:"receipt_binding_valid"`
	PreviewOnly           bool                             `json:"preview_only"`
	SelectedTargetID      *string                          `json:"selected_target_id"`
	Authority             SessionRunnerReceiptAuthority    `json:"authority"`
}

// ObserveSessionRunnerReceipt binds the existing Prompt/Run observation to a
// terminal receipt by repeating and checking attempt, command, target, and
// digest identities. It does not read a clock, select a target, issue a
// lease, persist a receipt, contact a Runner, or grant authority.
func ObserveSessionRunnerReceipt(input SessionRunnerReceiptObservationRequest) (SessionRunnerReceiptObservation, error) {
	if !validSessionRunnerIntentObservation(input.Intent) ||
		input.Owner != input.Intent.Owner || input.ConversationID != input.Intent.ConversationID ||
		input.PromptID != input.Intent.PromptID || input.RunID != input.Intent.RunID ||
		!validSessionRunnerTerminalObservation(input.Receipt) ||
		input.Receipt.AttemptID != input.Intent.AttemptID ||
		input.Receipt.CommandID != input.Intent.CommandID ||
		input.Receipt.TargetID != input.Intent.TargetID ||
		input.Receipt.CommandSHA256 != input.Intent.CommandSHA256 {
		return SessionRunnerReceiptObservation{}, errInvalidRequest
	}
	return SessionRunnerReceiptObservation{
		SchemaVersion:         SessionRunnerReceiptObservationSchemaVersion,
		EvaluationMode:        SessionRunnerReceiptObservationEvaluationMode,
		Owner:                 input.Intent.Owner,
		ConversationID:        input.Intent.ConversationID,
		PromptID:              input.Intent.PromptID,
		RunID:                 input.Intent.RunID,
		ReceiptObservation:    input.Receipt,
		PromptRunBindingValid: true,
		ReceiptBindingValid:   true,
		PreviewOnly:           true,
		SelectedTargetID:      nil,
		Authority:             SessionRunnerReceiptAuthority{},
	}, nil
}

// Validate checks a canonical session receipt observation after decoding. It
// intentionally cannot establish identity or execution authority; it only
// protects cross-client display consumers from confused or upgraded values.
func (observation SessionRunnerReceiptObservation) Validate() error {
	if observation.SchemaVersion != SessionRunnerReceiptObservationSchemaVersion ||
		observation.EvaluationMode != SessionRunnerReceiptObservationEvaluationMode ||
		!validOwner(observation.Owner) ||
		!validSessionIdentifier(observation.ConversationID) ||
		!validSessionIdentifier(observation.PromptID) ||
		!validSessionIdentifier(observation.RunID) ||
		!observation.PromptRunBindingValid || !observation.ReceiptBindingValid ||
		!observation.PreviewOnly || observation.SelectedTargetID != nil ||
		observation.Authority != (SessionRunnerReceiptAuthority{}) ||
		!validRunnerTerminalObservationEnvelope(observation.ReceiptObservation) {
		return errInvalidRequest
	}
	return nil
}

func validSessionRunnerIntentObservation(observation RunnerExecutionIntentObservation) bool {
	return observation.SchemaVersion == RunnerExecutionIntentSchemaVersion &&
		observation.EvaluationMode == RunnerExecutionIntentEvaluationMode &&
		validOwner(observation.Owner) &&
		validSessionIdentifier(observation.ConversationID) &&
		validSessionIdentifier(observation.PromptID) &&
		validSessionIdentifier(observation.RunID) &&
		validSessionIdentifier(observation.AttemptID) &&
		validSessionIdentifier(observation.CommandID) &&
		validSessionIdentifier(observation.TargetID) &&
		validRunnerDigest(observation.CommandSHA256) &&
		observation.IdempotencyKey == observation.RunID+":"+observation.AttemptID+":"+observation.CommandID &&
		observation.PromptRunBindingValid && observation.RunnerCommandBindingValid &&
		observation.PreviewOnly && observation.SelectedTargetID == nil &&
		observation.Authority == (RunnerExecutionIntentAuthority{})
}

func validSessionRunnerTerminalObservation(observation RunnerTerminalReceiptObservation) bool {
	return observation.SchemaVersion == RunnerTerminalReceiptSchemaVersion &&
		observation.EvaluationMode == RunnerTerminalReceiptEvaluationMode &&
		validSessionIdentifier(observation.CommandID) &&
		validRunnerDigest(observation.CommandSHA256) &&
		validSessionIdentifier(observation.AttemptID) &&
		validSessionIdentifier(observation.TargetID) &&
		observation.ObservedAtMS >= 0 && uint64(observation.ObservedAtMS) <= uint64(MaxSafeIntegerMS) &&
		validRunnerTerminalObservationEnvelope(observation)
}

func validRunnerTerminalObservationEnvelope(observation RunnerTerminalReceiptObservation) bool {
	uncertain := observation.DispositionKind == "uncertain"
	if observation.DispositionKind != "completed" && observation.DispositionKind != "failed" && !uncertain {
		return false
	}
	return observation.ReceiptValid && observation.PreviewOnly &&
		observation.Uncertain == uncertain &&
		observation.ReconciliationRequired == uncertain &&
		observation.ManualReviewRequired == uncertain &&
		!observation.AutomaticRetry &&
		((uncertain && observation.FollowUp == "reconciliation_manual") ||
			(!uncertain && observation.FollowUp == "none")) &&
		observation.Authority == (RunnerTerminalReceiptAuthority{})
}
