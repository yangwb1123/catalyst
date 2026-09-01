package application

import (
	domain "forgeos/forge-core/internal/delivery/domain"
	core "forgeos/forge-core/internal/platformcorecontract"
)

const (
	maxAssessments                = 128
	maxAssessmentRequestedEffects = 32
	maxAssessmentEffectBytes      = 64
)

type validatedSnapshot struct {
	value       ControlSnapshot
	order       []string
	items       map[string]domain.WorkItem
	assessments map[string]WorkItemAssessment
	drift       domain.SnapshotDrift
}

func validateControlSnapshot(value ControlSnapshot) (validatedSnapshot, error) {
	if err := validateWorkItemVocabulary(); err != nil {
		return validatedSnapshot{}, err
	}
	if err := domain.ValidateWorkGraph(value.Objective, value.Change, value.WorkGraph); err != nil {
		return validatedSnapshot{}, invalidSnapshot("Delivery aggregates are invalid", err)
	}
	drift, err := domain.CompareSnapshotBindings(
		value.Change.SnapshotBindings, value.CurrentSnapshotBindings,
	)
	if err != nil {
		return validatedSnapshot{}, invalidSnapshot("current snapshot bindings are invalid", err)
	}
	order, err := domain.TopologicalOrder(value.WorkGraph.Items)
	if err != nil {
		return validatedSnapshot{}, invalidSnapshot("WorkGraph order is invalid", err)
	}
	items := indexWorkItems(value.WorkGraph.Items)
	assessments, err := indexAssessments(value, items)
	if err != nil {
		return validatedSnapshot{}, err
	}
	if err := validateProgress(order, items); err != nil {
		return validatedSnapshot{}, err
	}
	return validatedSnapshot{value, order, items, assessments, drift}, nil
}

func indexWorkItems(values []domain.WorkItem) map[string]domain.WorkItem {
	result := make(map[string]domain.WorkItem, len(values))
	for _, value := range values {
		result[value.WorkItemID] = value
	}
	return result
}

func indexAssessments(
	snapshot ControlSnapshot, items map[string]domain.WorkItem,
) (map[string]WorkItemAssessment, error) {
	if len(snapshot.Assessments) > maxAssessments {
		return nil, invalidSnapshot("assessment count exceeds its bound", nil)
	}
	result := make(map[string]WorkItemAssessment, len(snapshot.Assessments))
	evidenceDigests := make(map[string]string)
	for _, value := range snapshot.Assessments {
		if err := validateAssessmentShape(value); err != nil {
			return nil, err
		}
		item, exists := items[value.WorkItemID]
		if !exists {
			return nil, invalidSnapshot("assessment targets an unknown WorkItem", nil)
		}
		if _, exists := result[value.WorkItemID]; exists {
			return nil, invalidSnapshot("assessments contain a duplicate WorkItem", nil)
		}
		if err := validateAssessment(snapshot, item, value); err != nil {
			return nil, err
		}
		if err := validateEvidenceConsistency(value.EvidenceRefs, evidenceDigests); err != nil {
			return nil, err
		}
		result[value.WorkItemID] = value
	}
	return result, nil
}

func validateAssessmentShape(value WorkItemAssessment) error {
	workItem := core.EntityRef{
		EntityID: value.WorkItemID, EntityType: core.EntityType("work_item"),
	}
	if err := core.ValidateReference(workItem); err != nil {
		return invalidSnapshot("assessment work_item_id is invalid", err)
	}
	return validateAssessmentEffects(value.RequestedEffects)
}

func validateAssessmentEffects(values []string) error {
	if len(values) > maxAssessmentRequestedEffects {
		return invalidSnapshot("assessment requested effect count exceeds its bound", nil)
	}
	for _, value := range values {
		if !validAssessmentEffect(value) {
			return invalidSnapshot("assessment requested effect is not a bounded token", nil)
		}
	}
	return nil
}

func validAssessmentEffect(value string) bool {
	if len(value) < 1 || len(value) > maxAssessmentEffectBytes ||
		!isAssessmentEffectAlphaNumeric(value[0]) ||
		!isAssessmentEffectAlphaNumeric(value[len(value)-1]) {
		return false
	}
	for _, character := range []byte(value) {
		if !isAssessmentEffectAlphaNumeric(character) && character != '.' &&
			character != '_' && character != '-' {
			return false
		}
	}
	return true
}

func isAssessmentEffectAlphaNumeric(value byte) bool {
	return value >= 'a' && value <= 'z' || value >= '0' && value <= '9'
}

func validateAssessment(
	snapshot ControlSnapshot, item domain.WorkItem, value WorkItemAssessment,
) error {
	if value.ChangeVersion != snapshot.Change.Version ||
		value.WorkGraphVersion != snapshot.WorkGraph.Version {
		return invalidSnapshot("assessment versions differ from the snapshot", nil)
	}
	if value.PolicyProfile != snapshot.Change.PolicyProfile ||
		value.ProjectSnapshotID != item.ProjectSnapshotID || value.Risk != item.Risk {
		return invalidSnapshot("assessment declarations differ from the WorkItem", nil)
	}
	if !sameStringSet(value.RequestedEffects, item.RequestedEffects) {
		return invalidSnapshot("assessment requested effects differ from the WorkItem", nil)
	}
	if !validAssessmentStatus(value.Policy) || !validAssessmentStatus(value.Approval) ||
		!validAssessmentStatus(value.Budget) {
		return invalidSnapshot("assessment status is unsupported", nil)
	}
	return validateEvidence(value.EvidenceRefs)
}
