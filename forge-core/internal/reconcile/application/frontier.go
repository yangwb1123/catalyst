package application

func frontierDecision(base Decision, snapshot validatedSnapshot) (Decision, error) {
	allCompleted := true
	for _, itemID := range snapshot.order {
		item := snapshot.items[itemID]
		if item.State == stateCompleted {
			continue
		}
		allCompleted = false
		if !predecessorsCompleted(item, snapshot.items) {
			continue
		}
		if item.State == stateDraft {
			return decide(base, DecisionNoOp, reasonWorkItemNotPlanned, itemID, nil), nil
		}
		if item.State == statePlanned || item.State == stateAwaitingApproval ||
			item.State == stateReady {
			return assessCandidate(base, itemID, snapshot), nil
		}
	}
	if allCompleted {
		return decide(base, DecisionNoOp, reasonAllCompletedUnjoined, "", nil), nil
	}
	return Decision{}, invalidSnapshot("valid DAG has no dependency-ready WorkItem", nil)
}

func assessCandidate(
	base Decision, itemID string, snapshot validatedSnapshot,
) Decision {
	assessment, exists := snapshot.assessments[itemID]
	if !exists {
		return decide(base, DecisionAwaitApproval, reasonAssessmentMissing, itemID, nil)
	}
	if assessment.Policy == AssessmentStatusUnknown {
		return assessedDecision(base, DecisionBlockWorkItem, reasonPolicyUnknown, assessment)
	}
	if assessment.Policy == AssessmentStatusUnsatisfiedDeclared {
		return assessedDecision(base, DecisionBlockWorkItem, reasonPolicyUnsatisfied, assessment)
	}
	if assessment.Approval == AssessmentStatusUnknown {
		return assessedDecision(base, DecisionAwaitApproval, reasonApprovalUnknown, assessment)
	}
	if assessment.Approval == AssessmentStatusUnsatisfiedDeclared {
		return assessedDecision(base, DecisionBlockWorkItem, reasonApprovalUnsatisfied, assessment)
	}
	return assessBudget(base, assessment)
}

func assessBudget(base Decision, assessment WorkItemAssessment) Decision {
	if assessment.Budget == AssessmentStatusUnknown {
		return assessedDecision(base, DecisionBlockWorkItem, reasonBudgetUnknown, assessment)
	}
	if assessment.Budget == AssessmentStatusUnsatisfiedDeclared {
		return assessedDecision(base, DecisionBlockWorkItem, reasonBudgetUnsatisfied, assessment)
	}
	return assessedDecision(base, DecisionReadyWorkItem, reasonReadyCandidate, assessment)
}

func assessedDecision(
	base Decision, kind DecisionKind, reason ReasonCode, assessment WorkItemAssessment,
) Decision {
	return decide(base, kind, reason, assessment.WorkItemID, &assessment)
}
