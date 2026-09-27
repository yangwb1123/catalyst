package deviceplacement

// SessionRunnerReceiptHistorySchemaVersion identifies the bounded, pure
// reduction of terminal receipt observations for one owner/session Run.
const SessionRunnerReceiptHistorySchemaVersion = "forge.session-runner-receipt-history/v1"

// SessionRunnerReceiptHistoryEvaluationMode keeps this value separate from
// receipt persistence, retry, lease, and Runner authority.
const SessionRunnerReceiptHistoryEvaluationMode = "pure_session_runner_receipt_history_only"

const MaxSessionRunnerReceiptHistory = 16

// SessionRunnerReceiptHistoryAuthority makes the absence of authority
// explicit in a history projection. A valid value never proves identity,
// persists evidence, authorizes execution, dispatches work, or publishes
// audit.
type SessionRunnerReceiptHistoryAuthority struct {
	IdentityVerified    bool `json:"identity_verified"`
	ReceiptPersisted    bool `json:"receipt_persisted"`
	ExecutionAuthorized bool `json:"execution_authorized"`
	DispatchPerformed   bool `json:"dispatch_performed"`
	AuditPublished      bool `json:"audit_published"`
}

// SessionRunnerReceiptHistoryRequest contains already validated, content-free
// receipt observations. The operation reads no store or clock and performs no
// retry or terminal mutation.
type SessionRunnerReceiptHistoryRequest struct {
	Owner          Owner                             `json:"owner"`
	ConversationID string                            `json:"conversation_id"`
	PromptID       string                            `json:"prompt_id"`
	RunID          string                            `json:"run_id"`
	Receipts       []SessionRunnerReceiptObservation `json:"receipts"`
}

// SessionRunnerReceiptHistoryObservation keeps the ordered observations and a
// deterministic latest-attempt summary together. Keeping the observations in
// the value lets every consumer re-check the reduction instead of trusting a
// caller-supplied count or disposition.
type SessionRunnerReceiptHistoryObservation struct {
	SchemaVersion          string                               `json:"schema_version"`
	EvaluationMode         string                               `json:"evaluation_mode"`
	Owner                  Owner                                `json:"owner"`
	ConversationID         string                               `json:"conversation_id"`
	PromptID               string                               `json:"prompt_id"`
	RunID                  string                               `json:"run_id"`
	Receipts               []SessionRunnerReceiptObservation    `json:"receipts"`
	AttemptCount           uint32                               `json:"attempt_count"`
	LatestAttemptID        string                               `json:"latest_attempt_id"`
	LatestCommandID        string                               `json:"latest_command_id"`
	LatestTargetID         string                               `json:"latest_target_id"`
	LatestDispositionKind  string                               `json:"latest_disposition_kind"`
	LatestObservedAtMS     uint64                               `json:"latest_observed_at_ms"`
	ReconciliationRequired bool                                 `json:"reconciliation_required"`
	ManualReviewRequired   bool                                 `json:"manual_review_required"`
	AutomaticRetry         bool                                 `json:"automatic_retry"`
	FollowUp               string                               `json:"follow_up"`
	SelectedTargetID       *string                              `json:"selected_target_id"`
	PreviewOnly            bool                                 `json:"preview_only"`
	Authority              SessionRunnerReceiptHistoryAuthority `json:"authority"`
}

// ObserveSessionRunnerReceiptHistory reduces a bounded, ordered set of
// session receipt observations. Definite failures may precede another attempt;
// a completed or uncertain receipt closes the history. Uncertain is therefore
// terminal and remains manual reconciliation evidence, never an automatic
// retry instruction.
func ObserveSessionRunnerReceiptHistory(input SessionRunnerReceiptHistoryRequest) (SessionRunnerReceiptHistoryObservation, error) {
	if !validSessionRunnerReceiptHistoryInput(input) {
		return SessionRunnerReceiptHistoryObservation{}, errInvalidRequest
	}
	receipts := append([]SessionRunnerReceiptObservation(nil), input.Receipts...)
	latest := receipts[len(receipts)-1].ReceiptObservation
	uncertain := latest.Uncertain
	followUp := "none"
	if uncertain {
		followUp = "reconciliation_manual"
	}
	return SessionRunnerReceiptHistoryObservation{
		SchemaVersion:          SessionRunnerReceiptHistorySchemaVersion,
		EvaluationMode:         SessionRunnerReceiptHistoryEvaluationMode,
		Owner:                  input.Owner,
		ConversationID:         input.ConversationID,
		PromptID:               input.PromptID,
		RunID:                  input.RunID,
		Receipts:               receipts,
		AttemptCount:           uint32(len(receipts)),
		LatestAttemptID:        latest.AttemptID,
		LatestCommandID:        latest.CommandID,
		LatestTargetID:         latest.TargetID,
		LatestDispositionKind:  latest.DispositionKind,
		LatestObservedAtMS:     latest.ObservedAtMS,
		ReconciliationRequired: uncertain,
		ManualReviewRequired:   uncertain,
		AutomaticRetry:         false,
		FollowUp:               followUp,
		SelectedTargetID:       nil,
		PreviewOnly:            true,
		Authority:              SessionRunnerReceiptHistoryAuthority{},
	}, nil
}

// Validate re-runs the history reduction and compares every derived field.
// It cannot authenticate the owner or turn the result into execution
// authority; it only rejects confused or upgraded display values.
func (observation SessionRunnerReceiptHistoryObservation) Validate() error {
	if observation.SchemaVersion != SessionRunnerReceiptHistorySchemaVersion ||
		observation.EvaluationMode != SessionRunnerReceiptHistoryEvaluationMode {
		return errInvalidRequest
	}
	derived, err := ObserveSessionRunnerReceiptHistory(SessionRunnerReceiptHistoryRequest{
		Owner: observation.Owner, ConversationID: observation.ConversationID,
		PromptID: observation.PromptID, RunID: observation.RunID, Receipts: observation.Receipts,
	})
	if err != nil || !sameSessionRunnerReceiptHistory(observation, derived) {
		return errInvalidRequest
	}
	return nil
}

func validSessionRunnerReceiptHistoryInput(input SessionRunnerReceiptHistoryRequest) bool {
	if !validOwner(input.Owner) || !validSessionIdentifier(input.ConversationID) ||
		!validSessionIdentifier(input.PromptID) || !validSessionIdentifier(input.RunID) ||
		len(input.Receipts) == 0 || len(input.Receipts) > MaxSessionRunnerReceiptHistory {
		return false
	}
	seenAttempts := make(map[string]struct{}, len(input.Receipts))
	for index, receipt := range input.Receipts {
		if err := receipt.Validate(); err != nil || receipt.Owner != input.Owner ||
			receipt.ConversationID != input.ConversationID || receipt.PromptID != input.PromptID ||
			receipt.RunID != input.RunID {
			return false
		}
		attemptID := receipt.ReceiptObservation.AttemptID
		if _, exists := seenAttempts[attemptID]; exists {
			return false
		}
		seenAttempts[attemptID] = struct{}{}
		if index > 0 {
			previous := input.Receipts[index-1]
			if sessionRunnerReceiptAfter(previous, receipt) ||
				previous.ReceiptObservation.DispositionKind == "completed" ||
				previous.ReceiptObservation.DispositionKind == "uncertain" {
				return false
			}
		}
	}
	return true
}

func sessionRunnerReceiptAfter(left, right SessionRunnerReceiptObservation) bool {
	leftTime := left.ReceiptObservation.ObservedAtMS
	rightTime := right.ReceiptObservation.ObservedAtMS
	return leftTime > rightTime ||
		(leftTime == rightTime && left.ReceiptObservation.AttemptID >= right.ReceiptObservation.AttemptID)
}

func sameSessionRunnerReceiptHistory(left, right SessionRunnerReceiptHistoryObservation) bool {
	return left.Owner == right.Owner && left.ConversationID == right.ConversationID &&
		left.PromptID == right.PromptID && left.RunID == right.RunID &&
		jsonEqualSessionRunnerReceiptHistory(left.Receipts, right.Receipts) &&
		left.AttemptCount == right.AttemptCount && left.LatestAttemptID == right.LatestAttemptID &&
		left.LatestCommandID == right.LatestCommandID && left.LatestTargetID == right.LatestTargetID &&
		left.LatestDispositionKind == right.LatestDispositionKind &&
		left.LatestObservedAtMS == right.LatestObservedAtMS &&
		left.ReconciliationRequired == right.ReconciliationRequired &&
		left.ManualReviewRequired == right.ManualReviewRequired &&
		left.AutomaticRetry == right.AutomaticRetry && left.FollowUp == right.FollowUp &&
		left.SelectedTargetID == right.SelectedTargetID && left.PreviewOnly == right.PreviewOnly &&
		left.Authority == right.Authority
}

func jsonEqualSessionRunnerReceiptHistory(left, right []SessionRunnerReceiptObservation) bool {
	if len(left) != len(right) {
		return false
	}
	for index := range left {
		if left[index].SchemaVersion != right[index].SchemaVersion ||
			left[index].EvaluationMode != right[index].EvaluationMode ||
			left[index].Owner != right[index].Owner ||
			left[index].ConversationID != right[index].ConversationID ||
			left[index].PromptID != right[index].PromptID || left[index].RunID != right[index].RunID ||
			left[index].ReceiptObservation != right[index].ReceiptObservation ||
			left[index].PromptRunBindingValid != right[index].PromptRunBindingValid ||
			left[index].ReceiptBindingValid != right[index].ReceiptBindingValid ||
			left[index].PreviewOnly != right[index].PreviewOnly ||
			left[index].SelectedTargetID != right[index].SelectedTargetID ||
			left[index].Authority != right[index].Authority {
			return false
		}
	}
	return true
}
