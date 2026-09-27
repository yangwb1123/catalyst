package deviceplacement

import (
	"forgeos/forge-core/internal/executionattempt"
	corestate "forgeos/forge-core/internal/platformcorecontract/state"
)

// RunnerAttemptBoundarySchemaVersion identifies the pure lifecycle gate that
// sits after admission previews and before any future Runner effect adapter.
const RunnerAttemptBoundarySchemaVersion = "forge.runner-attempt-boundary/v1"

// RunnerAttemptBoundaryEvaluationMode makes the no-effect boundary explicit
// to clients and operators consuming this value.
const RunnerAttemptBoundaryEvaluationMode = "attempt_lifecycle_dispatch_boundary_preview"

// RunnerAttemptBoundaryRequest joins a previously validated execution
// boundary to one caller-declared Attempt lifecycle transition. The boundary
// contains no durable Attempt state; this value therefore remains an
// observation and cannot advance an aggregate.
type RunnerAttemptBoundaryRequest struct {
	Boundary   RunnerExecutionBoundaryObservation `json:"execution_boundary"`
	Transition executionattempt.Transition        `json:"transition"`
}

// RunnerAttemptBoundaryPreviewRequest is the authenticated, flattened API
// projection of a boundary request. The server owns activation, authority,
// lease state, and evaluation time; the caller supplies only the same
// redacted execution inputs plus the lifecycle edge to inspect.
type RunnerAttemptBoundaryPreviewRequest struct {
	RunnerExecutionBoundaryPreviewRequest
	Transition executionattempt.Transition `json:"transition"`
}

// RunnerAttemptBoundaryAuthority records capabilities absent from this pure
// lifecycle check. Every field is fixed false for a valid observation.
type RunnerAttemptBoundaryAuthority struct {
	AttemptPersisted    bool `json:"attempt_persisted"`
	ReservationCreated  bool `json:"reservation_created"`
	ExecutionAuthorized bool `json:"execution_authorized"`
	DispatchPerformed   bool `json:"dispatch_performed"`
	AuditPublished      bool `json:"audit_published"`
}

// RunnerAttemptBoundaryObservation is a redacted, preview-only lifecycle
// decision. Only accepted->starting and starting->running are dispatchable;
// terminal or preparatory transitions remain visible but never ready.
type RunnerAttemptBoundaryObservation struct {
	SchemaVersion             string                         `json:"schema_version"`
	EvaluationMode            string                         `json:"evaluation_mode"`
	Owner                     Owner                          `json:"owner"`
	ConversationID            string                         `json:"conversation_id"`
	RunID                     string                         `json:"run_id"`
	AttemptID                 string                         `json:"attempt_id"`
	CommandID                 string                         `json:"command_id"`
	TargetID                  string                         `json:"target_id"`
	LeaseEpoch                uint64                         `json:"lease_epoch"`
	CurrentAttemptState       string                         `json:"current_attempt_state"`
	NextAttemptState          string                         `json:"next_attempt_state"`
	Transition                executionattempt.Transition    `json:"transition"`
	ExecutionBoundaryReady    bool                           `json:"execution_boundary_ready"`
	AttemptTransitionValid    bool                           `json:"attempt_transition_valid"`
	AttemptTransitionDispatch bool                           `json:"attempt_transition_dispatchable"`
	AttemptBoundaryReady      bool                           `json:"attempt_boundary_ready"`
	RejectionReasons          []string                       `json:"rejection_reasons"`
	PreviewOnly               bool                           `json:"preview_only"`
	Authority                 RunnerAttemptBoundaryAuthority `json:"authority"`
}

// ObserveRunnerAttemptBoundary checks the same Attempt state graph used by
// executionattempt.Lifecycle and narrows it to the two forward edges that
// can surround a future dispatch start. It never reads or mutates storage,
// renews a lease, contacts a Runner, or grants execution authority.
func ObserveRunnerAttemptBoundary(
	input RunnerAttemptBoundaryRequest,
) (RunnerAttemptBoundaryObservation, error) {
	if err := input.Boundary.Validate(); err != nil {
		return RunnerAttemptBoundaryObservation{}, errInvalidRequest
	}
	target, known := executionattempt.TransitionTarget(input.Transition)
	if !known {
		return RunnerAttemptBoundaryObservation{}, errInvalidRequest
	}
	current := corestate.AttemptState(input.Boundary.AttemptState)
	validatedTarget, transitionErr := executionattempt.ValidateTransition(current, input.Transition)
	if validatedTarget != target {
		return RunnerAttemptBoundaryObservation{}, errInvalidRequest
	}
	transitionValid := transitionErr == nil
	transitionDispatchable := transitionValid && dispatchableAttemptTransition(current, target)
	reasons := runnerAttemptBoundaryRejectionReasons(
		input.Boundary.ExecutionBoundaryReady, transitionValid, transitionDispatchable,
	)
	return RunnerAttemptBoundaryObservation{
		SchemaVersion:             RunnerAttemptBoundarySchemaVersion,
		EvaluationMode:            RunnerAttemptBoundaryEvaluationMode,
		Owner:                     input.Boundary.Owner,
		ConversationID:            input.Boundary.ConversationID,
		RunID:                     input.Boundary.RunID,
		AttemptID:                 input.Boundary.AttemptID,
		CommandID:                 input.Boundary.CommandID,
		TargetID:                  input.Boundary.TargetID,
		LeaseEpoch:                input.Boundary.LeaseEpoch,
		CurrentAttemptState:       input.Boundary.AttemptState,
		NextAttemptState:          string(target),
		Transition:                input.Transition,
		ExecutionBoundaryReady:    input.Boundary.ExecutionBoundaryReady,
		AttemptTransitionValid:    transitionValid,
		AttemptTransitionDispatch: transitionDispatchable,
		AttemptBoundaryReady:      input.Boundary.ExecutionBoundaryReady && transitionDispatchable,
		RejectionReasons:          reasons,
		PreviewOnly:               true,
		Authority:                 RunnerAttemptBoundaryAuthority{},
	}, nil
}

// Validate prevents a decoded lifecycle preview from being treated as an
// Attempt mutation or dispatch grant.
func (observation RunnerAttemptBoundaryObservation) Validate() error {
	if observation.SchemaVersion != RunnerAttemptBoundarySchemaVersion ||
		observation.EvaluationMode != RunnerAttemptBoundaryEvaluationMode ||
		!validOwner(observation.Owner) ||
		!validSessionIdentifier(observation.ConversationID) ||
		!validSessionIdentifier(observation.RunID) ||
		!validSessionIdentifier(observation.AttemptID) ||
		!validSessionIdentifier(observation.CommandID) ||
		!validSessionIdentifier(observation.TargetID) ||
		observation.LeaseEpoch == 0 ||
		!validDispatchAttemptState(observation.CurrentAttemptState) ||
		!validDispatchAttemptState(observation.NextAttemptState) ||
		!validRunnerAttemptTransition(observation.Transition) ||
		!observation.PreviewOnly ||
		observation.Authority != (RunnerAttemptBoundaryAuthority{}) ||
		observation.AttemptTransitionValid != runnerAttemptTransitionValid(observation.CurrentAttemptState, observation.Transition) ||
		observation.AttemptTransitionDispatch != dispatchableAttemptTransition(
			corestate.AttemptState(observation.CurrentAttemptState),
			corestate.AttemptState(observation.NextAttemptState),
		) ||
		observation.NextAttemptState != runnerAttemptTransitionTarget(observation.Transition) ||
		(observation.ExecutionBoundaryReady && !dispatchableAttemptState(observation.CurrentAttemptState)) ||
		observation.AttemptBoundaryReady != (observation.ExecutionBoundaryReady && observation.AttemptTransitionDispatch) ||
		!samePreflightStrings(observation.RejectionReasons, runnerAttemptBoundaryRejectionReasons(
			observation.ExecutionBoundaryReady, observation.AttemptTransitionValid,
			observation.AttemptTransitionDispatch,
		)) {
		return errInvalidRequest
	}
	if observation.AttemptBoundaryReady && len(observation.RejectionReasons) != 0 {
		return errInvalidRequest
	}
	return nil
}

func validRunnerAttemptTransition(value executionattempt.Transition) bool {
	_, ok := executionattempt.TransitionTarget(value)
	return ok
}

func runnerAttemptTransitionTarget(value executionattempt.Transition) string {
	target, ok := executionattempt.TransitionTarget(value)
	if !ok {
		return ""
	}
	return string(target)
}

func runnerAttemptTransitionValid(current string, transition executionattempt.Transition) bool {
	_, err := executionattempt.ValidateTransition(corestate.AttemptState(current), transition)
	return err == nil
}

func dispatchableAttemptTransition(current, target corestate.AttemptState) bool {
	return (current == corestate.AttemptState("accepted") && target == corestate.AttemptState("starting")) ||
		(current == corestate.AttemptState("starting") && target == corestate.AttemptState("running"))
}

func runnerAttemptBoundaryRejectionReasons(boundaryReady, transitionValid, transitionDispatchable bool) []string {
	reasons := make([]string, 0, 3)
	if !boundaryReady {
		reasons = append(reasons, "execution_boundary_not_ready")
	}
	if !transitionValid {
		reasons = append(reasons, "attempt_transition_invalid")
	} else if !transitionDispatchable {
		reasons = append(reasons, "attempt_transition_not_dispatchable")
	}
	return sortedUniqueStrings(reasons)
}
