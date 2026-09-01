package application

// Decide returns one passive deterministic decision and performs no I/O.
func Decide(value ControlSnapshot) (Decision, error) {
	snapshot, err := validateControlSnapshot(value)
	if err != nil {
		return Decision{}, err
	}
	base := baseDecision(snapshot.value)
	if decision, exists := uncertaintyDecision(base, snapshot); exists {
		return decision, nil
	}
	if driftPresent(snapshot.drift) {
		return decide(base, DecisionReplanChange, reasonSnapshotDrift, "", nil), nil
	}
	if decision, exists := lifecycleDecision(base, snapshot); exists {
		return decision, nil
	}
	if decision, exists := activeProgressDecision(base, snapshot); exists {
		return decision, nil
	}
	return frontierDecision(base, snapshot)
}

func baseDecision(value ControlSnapshot) Decision {
	return Decision{
		ObjectiveID: value.Objective.ObjectiveID, ObjectiveVersion: value.Objective.Version,
		ChangeID: value.Change.ChangeID, ChangeVersion: value.Change.Version,
		WorkGraphID: value.WorkGraph.WorkGraphID, WorkGraphVersion: value.WorkGraph.Version,
	}
}

func decide(
	base Decision, kind DecisionKind, reason ReasonCode, workItemID string,
	assessment *WorkItemAssessment,
) Decision {
	base.Kind, base.ReasonCode, base.WorkItemID = kind, reason, workItemID
	if assessment != nil {
		base.EvidenceRefs = sortedEvidence(assessment.EvidenceRefs)
	}
	return base
}
