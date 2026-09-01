package application

import pcstate "forgeos/forge-core/internal/platformcorecontract/state"

const (
	stateDraft            pcstate.WorkItemState = "draft"
	statePlanned          pcstate.WorkItemState = "planned"
	stateAwaitingApproval pcstate.WorkItemState = "awaiting_approval"
	stateReady            pcstate.WorkItemState = "ready"
	stateDispatched       pcstate.WorkItemState = "dispatched"
	stateRunning          pcstate.WorkItemState = "running"
	stateVerifying        pcstate.WorkItemState = "verifying"
	stateCompleted        pcstate.WorkItemState = "completed"
	stateBlocked          pcstate.WorkItemState = "blocked"
	stateFailed           pcstate.WorkItemState = "failed"
	stateUncertain        pcstate.WorkItemState = "uncertain"
	stateCancelled        pcstate.WorkItemState = "cancelled"
)

const (
	reasonUncertainChange        ReasonCode = "uncertain_change"
	reasonUncertainWorkItem      ReasonCode = "uncertain_work_item"
	reasonUncertainAssessment    ReasonCode = "uncertain_assessment"
	reasonSnapshotDrift          ReasonCode = "supplied_snapshot_drift"
	reasonObjectiveInactive      ReasonCode = "objective_inactive"
	reasonChangeAwaitingApproval ReasonCode = "change_awaiting_approval"
	reasonChangeInactive         ReasonCode = "change_inactive"
	reasonGraphInactive          ReasonCode = "work_graph_inactive"
	reasonObservationInactive    ReasonCode = "change_observation_inactive"
	reasonWorkInFlight           ReasonCode = "work_item_in_flight"
	reasonTerminalBlocker        ReasonCode = "terminal_work_item_blocker"
	reasonWorkItemNotPlanned     ReasonCode = "work_item_not_planned"
	reasonAssessmentMissing      ReasonCode = "assessment_missing"
	reasonPolicyUnknown          ReasonCode = "policy_unknown"
	reasonPolicyUnsatisfied      ReasonCode = "policy_unsatisfied"
	reasonApprovalUnknown        ReasonCode = "approval_unknown"
	reasonApprovalUnsatisfied    ReasonCode = "approval_unsatisfied"
	reasonBudgetUnknown          ReasonCode = "budget_unknown"
	reasonBudgetUnsatisfied      ReasonCode = "budget_unsatisfied"
	reasonReadyCandidate         ReasonCode = "ready_work_item_candidate"
	reasonAllCompletedUnjoined   ReasonCode = "all_work_items_completed_unjoined"
)

func validAssessmentStatus(value AssessmentStatus) bool {
	switch value {
	case AssessmentStatusUnknown, AssessmentStatusSatisfiedDeclared,
		AssessmentStatusUnsatisfiedDeclared, AssessmentStatusUncertain:
		return true
	default:
		return false
	}
}

func validateWorkItemVocabulary() error {
	return validateWorkItemVocabularyValues(pcstate.WorkItemStateValues())
}

func validateWorkItemVocabularyValues(values []pcstate.WorkItemState) error {
	if len(values) != 12 {
		return invalidSnapshot("Platform Core WorkItem vocabulary differs from Reconciler v1", nil)
	}
	seen := make(map[pcstate.WorkItemState]struct{}, len(values))
	for _, value := range values {
		if !knownWorkItemState(value) {
			return invalidSnapshot("Platform Core WorkItem vocabulary differs from Reconciler v1", nil)
		}
		if _, exists := seen[value]; exists {
			return invalidSnapshot("Platform Core WorkItem vocabulary contains a duplicate", nil)
		}
		seen[value] = struct{}{}
	}
	return nil
}

func knownWorkItemState(value pcstate.WorkItemState) bool {
	switch value {
	case stateDraft, statePlanned, stateAwaitingApproval, stateReady, stateDispatched,
		stateRunning, stateVerifying, stateCompleted, stateBlocked, stateFailed,
		stateUncertain, stateCancelled:
		return true
	default:
		return false
	}
}
