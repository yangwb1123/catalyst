package deviceplacement

import (
	"sort"

	"forgeos/forge-core/internal/executionlease"
)

// RunnerDispatchPlanPreviewSchemaVersion identifies a pure, fixed-time
// comparison of placement declarations, Attempt state, and a lease proof.
const RunnerDispatchPlanPreviewSchemaVersion = "forge.runner-dispatch-plan-preview/v1"

// RunnerDispatchPlanPreviewEvaluationMode makes the non-authoritative nature
// of the value explicit to clients and operators.
const RunnerDispatchPlanPreviewEvaluationMode = "pure_dispatch_plan_preview_only"

// RunnerDispatchPlanPreviewRequest joins already supplied values. Placement
// declarations are not authoritative inventory, AttemptState is not loaded
// from storage, and Lease is not issued or persisted by this package.
type RunnerDispatchPlanPreviewRequest struct {
	AttemptState string                           `json:"attempt_state"`
	Placement    Request                          `json:"placement_request"`
	Intent       RunnerExecutionIntentObservation `json:"runner_execution_intent"`
	Lease        RunnerTerminalLeaseGrant         `json:"lease"`
}

// RunnerDispatchPlanCandidate is one declaration-only candidate evaluation.
// DeclarativeReady means that all supplied values line up at the fixed
// observation time; it never means selected, reserved, authorized, or
// dispatched.
type RunnerDispatchPlanCandidate struct {
	TargetID               string   `json:"target_id"`
	AttributesUnverified   bool     `json:"attributes_unverified"`
	MatchesRequirements    bool     `json:"matches_requirements"`
	LeaseTargetMatch       bool     `json:"lease_target_match"`
	LeaseActive            bool     `json:"lease_active"`
	AttemptStateAdmissible bool     `json:"attempt_state_admissible"`
	DeclarativeReady       bool     `json:"declarative_ready"`
	Reasons                []string `json:"reasons"`
}

// RunnerDispatchPlanPreviewAuthority enumerates capabilities intentionally
// absent from this preview. Every bit is fixed false.
type RunnerDispatchPlanPreviewAuthority struct {
	DeviceIdentityVerified bool `json:"device_identity_verified"`
	AttemptPersisted       bool `json:"attempt_persisted"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
	AuditPublished         bool `json:"audit_published"`
}

// RunnerDispatchPlanPreviewObservation is content-free metadata for one
// hypothetical dispatch plan. It deliberately carries no fencing token,
// command argv, output, workspace path, or selected target.
type RunnerDispatchPlanPreviewObservation struct {
	SchemaVersion          string                             `json:"schema_version"`
	EvaluationMode         string                             `json:"evaluation_mode"`
	Owner                  Owner                              `json:"owner_declaration"`
	ConversationID         string                             `json:"conversation_id"`
	RunID                  string                             `json:"run_id"`
	AttemptID              string                             `json:"attempt_id"`
	AttemptState           string                             `json:"attempt_state"`
	AttemptStateAdmissible bool                               `json:"attempt_state_admissible"`
	CommandID              string                             `json:"command_id"`
	CommandSHA256          string                             `json:"command_sha256"`
	TargetID               string                             `json:"intent_target_id"`
	LeaseEpoch             uint64                             `json:"lease_epoch"`
	LeaseActive            bool                               `json:"lease_active"`
	EvaluatedAtMS          int64                              `json:"evaluated_at_ms"`
	CandidateCount         int                                `json:"candidate_count"`
	DeclarativeReadyCount  int                                `json:"declarative_ready_count"`
	Candidates             []RunnerDispatchPlanCandidate      `json:"candidates"`
	SelectedTargetID       *string                            `json:"selected_target_id"`
	PreviewOnly            bool                               `json:"preview_only"`
	ReservationCreated     bool                               `json:"reservation_created"`
	ExecutionAuthorized    bool                               `json:"execution_authorized"`
	DispatchPerformed      bool                               `json:"dispatch_performed"`
	Authority              RunnerDispatchPlanPreviewAuthority `json:"authority"`
}

// ObserveRunnerDispatchPlanPreview evaluates supplied candidate declarations
// at Placement.EvaluatedAtMS. It never chooses a candidate. The intent target
// and lease proof are compared as identities only, and an inactive lease or a
// terminal Attempt simply makes every candidate non-ready in the preview.
func ObserveRunnerDispatchPlanPreview(
	input RunnerDispatchPlanPreviewRequest,
) (RunnerDispatchPlanPreviewObservation, error) {
	if !validDispatchAttemptState(input.AttemptState) ||
		!validSessionRunnerIntentObservation(input.Intent) ||
		input.Placement.Owner != input.Intent.Owner {
		return RunnerDispatchPlanPreviewObservation{}, errInvalidRequest
	}
	if input.Placement.EvaluatedAtMS <= 0 || input.Placement.EvaluatedAtMS > MaxSafeIntegerMS {
		return RunnerDispatchPlanPreviewObservation{}, errInvalidRequest
	}
	if input.Intent.TargetID == "" || input.Lease.TargetID != input.Intent.TargetID ||
		input.Lease.AttemptID != input.Intent.AttemptID {
		return RunnerDispatchPlanPreviewObservation{}, errInvalidRequest
	}
	lease, err := dispatchLease(input.Lease)
	if err != nil {
		return RunnerDispatchPlanPreviewObservation{}, err
	}
	proof := executionlease.LeaseProof{
		AttemptID: input.Intent.AttemptID, TargetID: input.Intent.TargetID,
		Epoch: input.Lease.Epoch, FencingToken: input.Lease.FencingToken,
	}
	if lease.Proof() != proof {
		return RunnerDispatchPlanPreviewObservation{}, errInvalidRequest
	}
	placement, err := Evaluate(input.Placement)
	if err != nil {
		return RunnerDispatchPlanPreviewObservation{}, err
	}
	leaseActive := lease.IsActive(uint64(input.Placement.EvaluatedAtMS))
	stateAdmissible := dispatchableAttemptState(input.AttemptState)
	candidates := make([]RunnerDispatchPlanCandidate, 0, len(placement.DeviceResults))
	readyCount := 0
	for _, result := range placement.DeviceResults {
		leaseTargetMatch := result.DeviceID == input.Lease.TargetID
		candidate := RunnerDispatchPlanCandidate{
			TargetID:               result.DeviceID,
			AttributesUnverified:   result.AttributesUnverified,
			MatchesRequirements:    result.MatchesRequirements,
			LeaseTargetMatch:       leaseTargetMatch,
			LeaseActive:            leaseActive,
			AttemptStateAdmissible: stateAdmissible,
			// Keep an empty JSON array on the wire for a ready candidate. A
			// nil slice would encode as `null`, which violates the strict
			// cross-language contract consumed by Runtime and Console.
			Reasons: append([]string{}, result.ExclusionReasons...),
		}
		if !leaseTargetMatch {
			candidate.Reasons = append(candidate.Reasons, "lease_target_mismatch")
		}
		if !leaseActive {
			candidate.Reasons = append(candidate.Reasons, "lease_inactive_at_evaluated_time")
		}
		if !stateAdmissible {
			candidate.Reasons = append(candidate.Reasons, "attempt_state_not_dispatchable")
		}
		candidate.Reasons = sortedUniqueStrings(candidate.Reasons)
		candidate.DeclarativeReady = result.MatchesRequirements && leaseTargetMatch && leaseActive && stateAdmissible
		if candidate.DeclarativeReady {
			readyCount++
		}
		candidates = append(candidates, candidate)
	}
	return RunnerDispatchPlanPreviewObservation{
		SchemaVersion:          RunnerDispatchPlanPreviewSchemaVersion,
		EvaluationMode:         RunnerDispatchPlanPreviewEvaluationMode,
		Owner:                  input.Intent.Owner,
		ConversationID:         input.Intent.ConversationID,
		RunID:                  input.Intent.RunID,
		AttemptID:              input.Intent.AttemptID,
		AttemptState:           input.AttemptState,
		AttemptStateAdmissible: stateAdmissible,
		CommandID:              input.Intent.CommandID,
		CommandSHA256:          input.Intent.CommandSHA256,
		TargetID:               input.Intent.TargetID,
		LeaseEpoch:             input.Lease.Epoch,
		LeaseActive:            leaseActive,
		EvaluatedAtMS:          input.Placement.EvaluatedAtMS,
		CandidateCount:         len(candidates),
		DeclarativeReadyCount:  readyCount,
		Candidates:             candidates,
		SelectedTargetID:       nil,
		PreviewOnly:            true,
		ReservationCreated:     false,
		ExecutionAuthorized:    false,
		DispatchPerformed:      false,
		Authority:              RunnerDispatchPlanPreviewAuthority{},
	}, nil
}

// Validate checks a decoded observation and prevents a transport consumer
// from treating a candidate list as a selected or authorized dispatch plan.
func (observation RunnerDispatchPlanPreviewObservation) Validate() error {
	if observation.SchemaVersion != RunnerDispatchPlanPreviewSchemaVersion ||
		observation.EvaluationMode != RunnerDispatchPlanPreviewEvaluationMode ||
		!validOwner(observation.Owner) || !validSessionIdentifier(observation.ConversationID) ||
		!validSessionIdentifier(observation.RunID) || !validSessionIdentifier(observation.AttemptID) ||
		!validSessionIdentifier(observation.CommandID) || !validRunnerDigest(observation.CommandSHA256) ||
		!validSessionIdentifier(observation.TargetID) || !validDispatchAttemptState(observation.AttemptState) ||
		observation.AttemptStateAdmissible != dispatchableAttemptState(observation.AttemptState) ||
		observation.LeaseEpoch == 0 ||
		observation.EvaluatedAtMS <= 0 || observation.EvaluatedAtMS > MaxSafeIntegerMS ||
		observation.SelectedTargetID != nil || !observation.PreviewOnly ||
		observation.ReservationCreated || observation.ExecutionAuthorized || observation.DispatchPerformed ||
		observation.Authority != (RunnerDispatchPlanPreviewAuthority{}) ||
		observation.CandidateCount != len(observation.Candidates) {
		return errInvalidRequest
	}
	readyCount := 0
	seen := make(map[string]struct{}, len(observation.Candidates))
	for index, candidate := range observation.Candidates {
		if !validSessionIdentifier(candidate.TargetID) || !candidate.AttributesUnverified ||
			candidate.DeclarativeReady != (candidate.MatchesRequirements && candidate.LeaseTargetMatch && candidate.LeaseActive && candidate.AttemptStateAdmissible) ||
			candidate.LeaseTargetMatch != (candidate.TargetID == observation.TargetID) ||
			candidate.LeaseActive != observation.LeaseActive ||
			candidate.AttemptStateAdmissible != observation.AttemptStateAdmissible ||
			!isSortedUniqueStrings(candidate.Reasons) ||
			(candidate.DeclarativeReady && len(candidate.Reasons) != 0) {
			return errInvalidRequest
		}
		if index > 0 && observation.Candidates[index-1].TargetID >= candidate.TargetID {
			return errInvalidRequest
		}
		if _, exists := seen[candidate.TargetID]; exists {
			return errInvalidRequest
		}
		seen[candidate.TargetID] = struct{}{}
		if candidate.DeclarativeReady {
			readyCount++
		}
	}
	if readyCount != observation.DeclarativeReadyCount {
		return errInvalidRequest
	}
	return nil
}

func dispatchLease(value RunnerTerminalLeaseGrant) (executionlease.LeaseGrant, error) {
	grant := executionlease.LeaseGrant{
		V: value.V, AttemptID: value.AttemptID, TargetID: value.TargetID,
		Epoch: value.Epoch, FencingToken: value.FencingToken,
		IssuedAtMS: value.IssuedAtMS, ExpiresAtMS: value.ExpiresAtMS,
	}
	if err := grant.Validate(); err != nil {
		return executionlease.LeaseGrant{}, err
	}
	return grant, nil
}

func validDispatchAttemptState(value string) bool {
	switch value {
	case "requested", "accepted", "starting", "running", "interrupted", "completed", "failed", "uncertain":
		return true
	default:
		return false
	}
}

func dispatchableAttemptState(value string) bool {
	switch value {
	case "accepted", "starting", "running":
		return true
	default:
		return false
	}
}

func sortedUniqueStrings(values []string) []string {
	copy := append([]string{}, values...)
	sort.Strings(copy)
	if len(copy) < 2 {
		return copy
	}
	result := copy[:1]
	for _, value := range copy[1:] {
		if value != result[len(result)-1] {
			result = append(result, value)
		}
	}
	return result
}

func isSortedUniqueStrings(values []string) bool {
	for index := 1; index < len(values); index++ {
		if values[index-1] >= values[index] {
			return false
		}
	}
	return true
}
