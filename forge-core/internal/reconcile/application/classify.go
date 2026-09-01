package application

func uncertaintyDecision(
	base Decision, snapshot validatedSnapshot,
) (Decision, bool) {
	if string(snapshot.value.Change.ObservedState) == "uncertain" {
		return decide(base, DecisionEscalateUncertain, reasonUncertainChange, "", nil), true
	}
	for _, itemID := range snapshot.order {
		if snapshot.items[itemID].State == stateUncertain {
			assessment, exists := snapshot.assessments[itemID]
			return decideWithAssessment(
				base, DecisionEscalateUncertain, reasonUncertainWorkItem,
				itemID, assessment, exists,
			), true
		}
	}
	for _, itemID := range snapshot.order {
		assessment, exists := snapshot.assessments[itemID]
		if exists && assessmentUncertain(assessment) {
			return decide(base, DecisionEscalateUncertain, reasonUncertainAssessment,
				itemID, &assessment), true
		}
	}
	return Decision{}, false
}

func assessmentUncertain(value WorkItemAssessment) bool {
	return value.Policy == AssessmentStatusUncertain ||
		value.Approval == AssessmentStatusUncertain || value.Budget == AssessmentStatusUncertain
}

func lifecycleDecision(
	base Decision, snapshot validatedSnapshot,
) (Decision, bool) {
	value := snapshot.value
	if string(value.Objective.State) != "active" {
		return decide(base, DecisionNoOp, reasonObjectiveInactive, "", nil), true
	}
	if string(value.Change.DesiredState) == "awaiting_approval" {
		return decide(base, DecisionAwaitApproval, reasonChangeAwaitingApproval, "", nil), true
	}
	if string(value.Change.DesiredState) != "active" {
		return decide(base, DecisionNoOp, reasonChangeInactive, "", nil), true
	}
	if string(value.WorkGraph.State) != "accepted" {
		return decide(base, DecisionNoOp, reasonGraphInactive, "", nil), true
	}
	observation := string(value.Change.ObservedState)
	if observation != "not_started" && observation != "in_progress" {
		return decide(base, DecisionNoOp, reasonObservationInactive, "", nil), true
	}
	return Decision{}, false
}

func activeProgressDecision(
	base Decision, snapshot validatedSnapshot,
) (Decision, bool) {
	for _, itemID := range snapshot.order {
		switch snapshot.items[itemID].State {
		case stateDispatched, stateRunning, stateVerifying:
			return decide(base, DecisionNoOp, reasonWorkInFlight, itemID, nil), true
		}
	}
	for _, itemID := range snapshot.order {
		switch snapshot.items[itemID].State {
		case stateBlocked, stateFailed, stateCancelled:
			return decide(base, DecisionBlockWorkItem, reasonTerminalBlocker, itemID, nil), true
		}
	}
	return Decision{}, false
}

func decideWithAssessment(
	base Decision, kind DecisionKind, reason ReasonCode, workItemID string,
	assessment WorkItemAssessment, exists bool,
) Decision {
	if !exists {
		return decide(base, kind, reason, workItemID, nil)
	}
	return decide(base, kind, reason, workItemID, &assessment)
}
