package deviceplacement

const SessionRunnerReconciliationProjectionSchemaVersion = "forge.session-runner-reconciliation-projection/v1"

const SessionRunnerReconciliationProjectionEvaluationMode = "pure_session_runner_reconciliation_projection_only"

// SessionRunnerReconciliationSource is the bounded history summary carried by
// the projection. It contains no receipt payload and grants no authority.
type SessionRunnerReconciliationSource struct {
	SchemaVersion         string `json:"schema_version"`
	Owner                 Owner  `json:"owner"`
	ConversationID        string `json:"conversation_id"`
	PromptID              string `json:"prompt_id"`
	RunID                 string `json:"run_id"`
	AttemptCount          uint32 `json:"attempt_count"`
	LatestAttemptID       string `json:"latest_attempt_id"`
	LatestCommandID       string `json:"latest_command_id"`
	LatestTargetID        string `json:"latest_target_id"`
	LatestDispositionKind string `json:"latest_disposition_kind"`
	LatestObservedAtMS    uint64 `json:"latest_observed_at_ms"`
}

// SessionRunnerReconciliationProjection is a display-only instruction to put
// an uncertain terminal receipt into a manual reconciliation queue. It never
// authorizes retry, target selection, execution, persistence, or publication.
type SessionRunnerReconciliationProjection struct {
	SchemaVersion          string                               `json:"schema_version"`
	EvaluationMode         string                               `json:"evaluation_mode"`
	Owner                  Owner                                `json:"owner"`
	ConversationID         string                               `json:"conversation_id"`
	PromptID               string                               `json:"prompt_id"`
	RunID                  string                               `json:"run_id"`
	Source                 SessionRunnerReconciliationSource    `json:"source"`
	LatestAttemptID        string                               `json:"latest_attempt_id"`
	LatestCommandID        string                               `json:"latest_command_id"`
	LatestTargetID         string                               `json:"latest_target_id"`
	LatestDispositionKind  string                               `json:"latest_disposition_kind"`
	LatestObservedAtMS     uint64                               `json:"latest_observed_at_ms"`
	ReconciliationKind     string                               `json:"reconciliation_kind"`
	ReconciliationReason   string                               `json:"reconciliation_reason"`
	ReconciliationRequired bool                                 `json:"reconciliation_required"`
	ManualReviewRequired   bool                                 `json:"manual_review_required"`
	AutomaticRetry         bool                                 `json:"automatic_retry"`
	FollowUp               string                               `json:"follow_up"`
	SelectedTargetID       *string                              `json:"selected_target_id"`
	PreviewOnly            bool                                 `json:"preview_only"`
	Authority              SessionRunnerReceiptHistoryAuthority `json:"authority"`
}

// ProjectSessionRunnerReconciliation derives the canonical manual projection
// from a caller-supplied, already complete history value. Only an uncertain
// terminal history can produce a projection.
func ProjectSessionRunnerReconciliation(history SessionRunnerReceiptHistoryObservation) (SessionRunnerReconciliationProjection, error) {
	if history.Validate() != nil || !history.ReconciliationRequired || !history.ManualReviewRequired ||
		history.LatestDispositionKind != "uncertain" || history.AutomaticRetry ||
		history.FollowUp != "reconciliation_manual" {
		return SessionRunnerReconciliationProjection{}, errInvalidRequest
	}
	source := sessionRunnerReconciliationSource(history)
	return SessionRunnerReconciliationProjection{
		SchemaVersion:  SessionRunnerReconciliationProjectionSchemaVersion,
		EvaluationMode: SessionRunnerReconciliationProjectionEvaluationMode,
		Owner:          history.Owner, ConversationID: history.ConversationID, PromptID: history.PromptID, RunID: history.RunID,
		Source: source, LatestAttemptID: source.LatestAttemptID, LatestCommandID: source.LatestCommandID,
		LatestTargetID: source.LatestTargetID, LatestDispositionKind: source.LatestDispositionKind,
		LatestObservedAtMS: source.LatestObservedAtMS, ReconciliationKind: "manual",
		ReconciliationReason: "uncertain_terminal_receipt", ReconciliationRequired: true,
		ManualReviewRequired: true, AutomaticRetry: false, FollowUp: "reconciliation_manual",
		SelectedTargetID: nil, PreviewOnly: true, Authority: SessionRunnerReceiptHistoryAuthority{},
	}, nil
}

func sessionRunnerReconciliationSource(history SessionRunnerReceiptHistoryObservation) SessionRunnerReconciliationSource {
	return SessionRunnerReconciliationSource{
		SchemaVersion: history.SchemaVersion, Owner: history.Owner, ConversationID: history.ConversationID,
		PromptID: history.PromptID, RunID: history.RunID, AttemptCount: history.AttemptCount,
		LatestAttemptID: history.LatestAttemptID, LatestCommandID: history.LatestCommandID,
		LatestTargetID: history.LatestTargetID, LatestDispositionKind: history.LatestDispositionKind,
		LatestObservedAtMS: history.LatestObservedAtMS,
	}
}

// Validate rejects drift in a decoded projection. It validates only the
// self-contained summary; ProjectSessionRunnerReconciliation is the boundary
// that proves derivation from a full history.
func (projection SessionRunnerReconciliationProjection) Validate() error {
	if projection.SchemaVersion != SessionRunnerReconciliationProjectionSchemaVersion ||
		projection.EvaluationMode != SessionRunnerReconciliationProjectionEvaluationMode ||
		!validSessionRunnerReconciliationSource(projection.Source) ||
		!sameSessionRunnerReconciliationSummary(projection) ||
		projection.ReconciliationKind != "manual" || projection.ReconciliationReason != "uncertain_terminal_receipt" ||
		!projection.ReconciliationRequired || !projection.ManualReviewRequired || projection.AutomaticRetry ||
		projection.FollowUp != "reconciliation_manual" || projection.SelectedTargetID != nil ||
		!projection.PreviewOnly || projection.Authority != (SessionRunnerReceiptHistoryAuthority{}) {
		return errInvalidRequest
	}
	return nil
}

func validSessionRunnerReconciliationSource(source SessionRunnerReconciliationSource) bool {
	return source.SchemaVersion == SessionRunnerReceiptHistorySchemaVersion && validOwner(source.Owner) &&
		validSessionIdentifier(source.ConversationID) && validSessionIdentifier(source.PromptID) &&
		validSessionIdentifier(source.RunID) && source.AttemptCount > 0 &&
		source.AttemptCount <= MaxSessionRunnerReceiptHistory && validSessionIdentifier(source.LatestAttemptID) &&
		validSessionIdentifier(source.LatestCommandID) && validSessionIdentifier(source.LatestTargetID) &&
		source.LatestDispositionKind == "uncertain" && source.LatestObservedAtMS <= uint64(MaxSafeIntegerMS)
}

func sameSessionRunnerReconciliationSummary(projection SessionRunnerReconciliationProjection) bool {
	source := projection.Source
	return projection.Owner == source.Owner && projection.ConversationID == source.ConversationID &&
		projection.PromptID == source.PromptID && projection.RunID == source.RunID &&
		projection.LatestAttemptID == source.LatestAttemptID &&
		projection.LatestCommandID == source.LatestCommandID && projection.LatestTargetID == source.LatestTargetID &&
		projection.LatestDispositionKind == source.LatestDispositionKind &&
		projection.LatestObservedAtMS == source.LatestObservedAtMS
}
