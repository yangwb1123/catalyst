package domain

func cloneObjective(value Objective) Objective {
	result := value
	result.Constraints = append([]string(nil), value.Constraints...)
	result.TargetProjectIDs = append([]string(nil), value.TargetProjectIDs...)
	result.SuccessMeasures = append([]string(nil), value.SuccessMeasures...)
	return result
}

func cloneChange(value Change) Change {
	result := value
	result.SnapshotBindings = append([]SnapshotBinding(nil), value.SnapshotBindings...)
	result.AcceptanceCriteria = append([]AcceptanceCriterion(nil), value.AcceptanceCriteria...)
	for index := range result.AcceptanceCriteria {
		result.AcceptanceCriteria[index].VerificationRequirements = append(
			[]string(nil), value.AcceptanceCriteria[index].VerificationRequirements...,
		)
	}
	if value.ImpactAssessmentRef != nil {
		reference := *value.ImpactAssessmentRef
		result.ImpactAssessmentRef = &reference
	}
	return result
}

func cloneWorkGraph(value WorkGraph) WorkGraph {
	result := value
	result.SnapshotBindings = append([]SnapshotBinding(nil), value.SnapshotBindings...)
	result.Items = append([]WorkItem(nil), value.Items...)
	for index := range result.Items {
		cloneWorkItemDeclarations(&result.Items[index], value.Items[index])
	}
	return result
}

func cloneWorkItemDeclarations(result *WorkItem, value WorkItem) {
	result.Dependencies = append([]string(nil), value.Dependencies...)
	result.AcceptanceCriterionIDs = append([]string(nil), value.AcceptanceCriterionIDs...)
	result.RequestedEffects = append([]string(nil), value.RequestedEffects...)
	result.AgentRequirements = append([]string(nil), value.AgentRequirements...)
	result.VerificationRequirements = append([]string(nil), value.VerificationRequirements...)
	if value.ContextArtifactRef != nil {
		reference := *value.ContextArtifactRef
		result.ContextArtifactRef = &reference
	}
}
