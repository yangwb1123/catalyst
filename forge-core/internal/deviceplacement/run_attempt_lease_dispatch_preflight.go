package deviceplacement

// RunAttemptLeaseDispatchPreflightSchemaVersion identifies the pure value
// preflight that joins an existing Run declaration to the Attempt, Runner
// intent, lease, and offline placement declarations. It is deliberately
// separate from any route or effectful execution contract.
const RunAttemptLeaseDispatchPreflightSchemaVersion = "forge.run-attempt-lease-dispatch-preflight/v1"

// RunAttemptLeaseDispatchPreflightEvaluationMode makes the boundary explicit:
// this adapter compares caller-supplied values and acquires no authority.
const RunAttemptLeaseDispatchPreflightEvaluationMode = "pure_run_attempt_lease_dispatch_preflight"

// RunAttemptLeaseDispatchPreflightRequest contains an already observed Run
// status and the existing value-only dispatch-plan inputs. The Run status is
// a declaration here; no Hub or Attempt store is consulted.
type RunAttemptLeaseDispatchPreflightRequest struct {
	Owner          Owner                            `json:"owner"`
	ConversationID string                           `json:"conversation_id"`
	RunID          string                           `json:"run_id"`
	RunStatus      string                           `json:"run_status"`
	DispatchPlan   RunnerDispatchPlanPreviewRequest `json:"dispatch_plan"`
}

// RunAttemptLeaseDispatchPreflightAuthority enumerates capabilities that this
// value adapter never acquires. A valid result always has every field false.
type RunAttemptLeaseDispatchPreflightAuthority struct {
	IdentityVerified    bool `json:"identity_verified"`
	RunAuthoritative    bool `json:"run_authoritative"`
	AttemptPersisted    bool `json:"attempt_persisted"`
	LeaseIssued         bool `json:"lease_issued"`
	ReservationCreated  bool `json:"reservation_created"`
	ExecutionAuthorized bool `json:"execution_authorized"`
	DispatchPerformed   bool `json:"dispatch_performed"`
	AuditPublished      bool `json:"audit_published"`
}

// RunAttemptLeaseDispatchPreflightObservation is a metadata-only result. A
// true DeclarativePreflightReady means that supplied declarations line up at
// the fixed placement time; it never selects a target or authorizes dispatch.
type RunAttemptLeaseDispatchPreflightObservation struct {
	SchemaVersion             string                                    `json:"schema_version"`
	EvaluationMode            string                                    `json:"evaluation_mode"`
	Owner                     Owner                                     `json:"owner"`
	ConversationID            string                                    `json:"conversation_id"`
	RunID                     string                                    `json:"run_id"`
	RunStatus                 string                                    `json:"run_status"`
	RunStateAdmissible        bool                                      `json:"run_state_admissible"`
	AttemptID                 string                                    `json:"attempt_id"`
	AttemptState              string                                    `json:"attempt_state"`
	AttemptStateAdmissible    bool                                      `json:"attempt_state_admissible"`
	CommandID                 string                                    `json:"command_id"`
	TargetID                  string                                    `json:"intent_target_id"`
	LeaseEpoch                uint64                                    `json:"lease_epoch"`
	LeaseActive               bool                                      `json:"lease_active"`
	EvaluatedAtMS             int64                                     `json:"evaluated_at_ms"`
	CandidateCount            int                                       `json:"candidate_count"`
	DeclarativeReadyCount     int                                       `json:"declarative_ready_count"`
	DeclarativePreflightReady bool                                      `json:"declarative_preflight_ready"`
	RejectionReasons          []string                                  `json:"rejection_reasons"`
	SelectedTargetID          *string                                   `json:"selected_target_id"`
	PreviewOnly               bool                                      `json:"preview_only"`
	Authority                 RunAttemptLeaseDispatchPreflightAuthority `json:"authority"`
}

// ObserveRunAttemptLeaseDispatchPreflight joins the supplied Run state to the
// existing pure dispatch-plan evaluator. It performs no selection, lease
// issuance, reservation, scheduling, dispatch, Runner contact, persistence,
// or clock read.
func ObserveRunAttemptLeaseDispatchPreflight(
	input RunAttemptLeaseDispatchPreflightRequest,
) (RunAttemptLeaseDispatchPreflightObservation, error) {
	if input.Owner != input.DispatchPlan.Intent.Owner ||
		input.ConversationID != input.DispatchPlan.Intent.ConversationID ||
		input.RunID != input.DispatchPlan.Intent.RunID ||
		!validRunStatus(input.RunStatus) {
		return RunAttemptLeaseDispatchPreflightObservation{}, errInvalidRequest
	}
	plan, err := ObserveRunnerDispatchPlanPreview(input.DispatchPlan)
	if err != nil {
		return RunAttemptLeaseDispatchPreflightObservation{}, err
	}
	runStateAdmissible := input.RunStatus == "nonterminal"
	reasons := preflightRejectionReasons(
		runStateAdmissible, plan.AttemptStateAdmissible, plan.LeaseActive, plan.DeclarativeReadyCount,
	)
	return RunAttemptLeaseDispatchPreflightObservation{
		SchemaVersion:             RunAttemptLeaseDispatchPreflightSchemaVersion,
		EvaluationMode:            RunAttemptLeaseDispatchPreflightEvaluationMode,
		Owner:                     input.Owner,
		ConversationID:            input.ConversationID,
		RunID:                     input.RunID,
		RunStatus:                 input.RunStatus,
		RunStateAdmissible:        runStateAdmissible,
		AttemptID:                 plan.AttemptID,
		AttemptState:              plan.AttemptState,
		AttemptStateAdmissible:    plan.AttemptStateAdmissible,
		CommandID:                 plan.CommandID,
		TargetID:                  plan.TargetID,
		LeaseEpoch:                plan.LeaseEpoch,
		LeaseActive:               plan.LeaseActive,
		EvaluatedAtMS:             plan.EvaluatedAtMS,
		CandidateCount:            plan.CandidateCount,
		DeclarativeReadyCount:     plan.DeclarativeReadyCount,
		DeclarativePreflightReady: runStateAdmissible && plan.DeclarativeReadyCount > 0 && plan.AttemptStateAdmissible && plan.LeaseActive,
		RejectionReasons:          reasons,
		SelectedTargetID:          nil,
		PreviewOnly:               true,
		Authority:                 RunAttemptLeaseDispatchPreflightAuthority{},
	}, nil
}

// Validate checks a decoded preflight observation before a display consumer
// uses it. It does not turn declarative readiness into an execution decision.
func (observation RunAttemptLeaseDispatchPreflightObservation) Validate() error {
	if observation.SchemaVersion != RunAttemptLeaseDispatchPreflightSchemaVersion ||
		observation.EvaluationMode != RunAttemptLeaseDispatchPreflightEvaluationMode ||
		!validOwner(observation.Owner) || !validSessionIdentifier(observation.ConversationID) ||
		!validSessionIdentifier(observation.RunID) || !validRunStatus(observation.RunStatus) ||
		observation.RunStateAdmissible != (observation.RunStatus == "nonterminal") ||
		!validSessionIdentifier(observation.AttemptID) || !validDispatchAttemptState(observation.AttemptState) ||
		observation.AttemptStateAdmissible != dispatchableAttemptState(observation.AttemptState) ||
		!validSessionIdentifier(observation.CommandID) || !validSessionIdentifier(observation.TargetID) ||
		observation.LeaseEpoch == 0 || observation.LeaseEpoch > uint64(MaxSafeIntegerMS) || observation.EvaluatedAtMS <= 0 || observation.EvaluatedAtMS > MaxSafeIntegerMS ||
		observation.CandidateCount < 0 || observation.DeclarativeReadyCount < 0 ||
		observation.DeclarativeReadyCount > observation.CandidateCount ||
		observation.DeclarativePreflightReady != (observation.RunStateAdmissible && observation.AttemptStateAdmissible && observation.LeaseActive && observation.DeclarativeReadyCount > 0) ||
		!isSortedUniqueStrings(observation.RejectionReasons) ||
		!samePreflightStrings(observation.RejectionReasons, preflightRejectionReasons(
			observation.RunStateAdmissible, observation.AttemptStateAdmissible,
			observation.LeaseActive, observation.DeclarativeReadyCount,
		)) ||
		(observation.DeclarativePreflightReady && len(observation.RejectionReasons) != 0) ||
		observation.SelectedTargetID != nil || !observation.PreviewOnly ||
		observation.Authority != (RunAttemptLeaseDispatchPreflightAuthority{}) {
		return errInvalidRequest
	}
	return nil
}

func preflightRejectionReasons(runStateAdmissible, attemptStateAdmissible, leaseActive bool, readyCount int) []string {
	reasons := make([]string, 0, 4)
	if !runStateAdmissible {
		reasons = append(reasons, "run_state_not_dispatchable")
	}
	if !attemptStateAdmissible {
		reasons = append(reasons, "attempt_state_not_dispatchable")
	}
	if !leaseActive {
		reasons = append(reasons, "lease_inactive_at_evaluated_time")
	}
	if readyCount == 0 {
		reasons = append(reasons, "no_declarative_ready_candidate")
	}
	if len(reasons) == 0 {
		// Keep the wire representation an empty JSON array. Rust's strict
		// consumer intentionally rejects null for this required collection.
		return []string{}
	}
	return sortedUniqueStrings(reasons)
}

func samePreflightStrings(left, right []string) bool {
	if len(left) != len(right) {
		return false
	}
	for index := range left {
		if left[index] != right[index] {
			return false
		}
	}
	return true
}
