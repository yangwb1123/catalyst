package domain

import (
	"sort"

	core "forgeos/forge-core/internal/platformcorecontract"
	pcstate "forgeos/forge-core/internal/platformcorecontract/state"
)

// ValidateWorkGraph validates one supplied graph against its Objective and Change.
func ValidateWorkGraph(objective Objective, change Change, value WorkGraph) error {
	changeBindings, err := validateChange(objective, change)
	if err != nil {
		return invalidDomain("WorkGraph Change is invalid", err)
	}
	if err := validateGraphIdentity(change, value); err != nil {
		return err
	}
	graphBindings, err := validateSnapshotBindings(value.SnapshotBindings, 1, maxTargetProjects)
	if err != nil {
		return err
	}
	if !sameSnapshotBindings(changeBindings, graphBindings) {
		return invalidDomain("WorkGraph snapshots differ from Change", nil)
	}
	criteria := indexCriteria(change.AcceptanceCriteria)
	if err := validateGraphItems(value.Items, graphBindings, criteria, change.Budget); err != nil {
		return err
	}
	order, err := TopologicalOrder(value.Items)
	if err != nil {
		return err
	}
	if err := validateCriticalPathDuration(value.Items, order, change.Budget.MaxDurationMS); err != nil {
		return err
	}
	return validateGraphMetadata(value)
}

func validateGraphIdentity(change Change, value WorkGraph) error {
	if err := validateEntityID(value.WorkGraphID, "work_graph", "work_graph_id"); err != nil {
		return err
	}
	if value.ChangeID != change.ChangeID || value.ChangeVersion != change.Version ||
		value.ObjectiveID != change.ObjectiveID || value.SpaceID != change.SpaceID {
		return invalidDomain("WorkGraph ancestry differs from Change", nil)
	}
	return nil
}

func validateGraphMetadata(value WorkGraph) error {
	if err := validateActor(value.AuthoredBy, "WorkGraph author"); err != nil {
		return err
	}
	if err := validateUnixMS(value.AuthoredAtUnixMS, "WorkGraph authored_at"); err != nil {
		return err
	}
	if !isBoundedClosedValue(string(value.State)) || !workGraphStates[string(value.State)] {
		return invalidDomain("WorkGraph state is unsupported", nil)
	}
	return validateVersion(value.Version, "WorkGraph version")
}

func validateGraphItems(
	items []WorkItem,
	bindings map[string]string,
	criteria criterionIndex,
	changeBudget BudgetLimit,
) error {
	if len(items) < 1 || len(items) > maxWorkItems {
		return invalidDomain("WorkGraph item count is outside its bound", nil)
	}
	ordered, err := sortedUniqueWorkItems(items)
	if err != nil {
		return err
	}
	covered := make(map[string]bool, len(criteria))
	verified := make(map[string]bool)
	attempts, cost := int64(0), int64(0)
	for _, item := range ordered {
		if err := validateWorkItem(item, bindings, criteria, covered, verified); err != nil {
			return err
		}
		var ok bool
		attempts, ok = addWithin(attempts, item.Budget.MaxAttempts, changeBudget.MaxAttempts)
		if !ok {
			return invalidDomain("WorkItem attempts exceed Change budget", nil)
		}
		cost, ok = addWithin(cost, item.Budget.MaxCostMicroUSD, changeBudget.MaxCostMicroUSD)
		if !ok {
			return invalidDomain("WorkItem cost exceeds Change budget", nil)
		}
	}
	return validateCriterionCoverage(criteria, covered, verified)
}

func sortedUniqueWorkItems(items []WorkItem) ([]WorkItem, error) {
	for _, item := range items {
		if err := validateEntityID(item.WorkItemID, "work_item", "work_item_id"); err != nil {
			return nil, invalidDomain("WorkGraph contains an invalid WorkItem ID", nil)
		}
	}
	result := append([]WorkItem(nil), items...)
	sort.Slice(result, func(left, right int) bool {
		return result[left].WorkItemID < result[right].WorkItemID
	})
	for index, item := range result {
		if index != 0 && item.WorkItemID == result[index-1].WorkItemID {
			return nil, invalidDomain("WorkGraph contains a duplicate WorkItem", nil)
		}
	}
	return result, nil
}

func validateWorkItem(
	value WorkItem,
	bindings map[string]string,
	criteria criterionIndex,
	covered, verified map[string]bool,
) error {
	if err := validateEntityID(value.WorkItemID, "work_item", "work_item_id"); err != nil {
		return err
	}
	if err := validateText(value.Purpose, "WorkItem purpose", 1, 2048); err != nil {
		return err
	}
	if err := validateEntityID(value.ProjectID, "project", "WorkItem project_id"); err != nil {
		return err
	}
	if err := validateEntityID(
		value.ProjectSnapshotID, "project_snapshot", "WorkItem project_snapshot_id",
	); err != nil {
		return err
	}
	boundSnapshot, exists := bindings[value.ProjectID]
	if !exists || boundSnapshot != value.ProjectSnapshotID {
		return invalidDomain("WorkItem target is not an exact graph snapshot", nil)
	}
	if err := validateEntityIDList(
		value.Dependencies, "work_item", "WorkItem dependencies", 0, maxItemListEntries,
	); err != nil {
		return err
	}
	if err := validateWorkItemDeclarations(value); err != nil {
		return err
	}
	if err := validateCriterionRefs(
		value.AcceptanceCriterionIDs, value.VerificationRequirements,
		criteria, covered, verified,
	); err != nil {
		return err
	}
	return validateWorkItemContext(value)
}

func validateWorkItemDeclarations(value WorkItem) error {
	if err := validateTokenList(
		value.RequestedEffects, "requested effects", 0, maxItemListEntries, 64,
	); err != nil {
		return err
	}
	if err := validateToken(value.Risk, "WorkItem risk", 16); err != nil {
		return err
	}
	if !workItemRisks[value.Risk] {
		return invalidDomain("WorkItem risk is unsupported", nil)
	}
	if err := validateBudget(value.Budget, "WorkItem budget"); err != nil {
		return err
	}
	if err := validateTextList(
		value.AgentRequirements, "agent requirements", 0, maxItemListEntries, 256,
	); err != nil {
		return err
	}
	if err := validateTokenList(
		value.VerificationRequirements, "verification requirements", 1, maxItemListEntries, 64,
	); err != nil {
		return err
	}
	if err := pcstate.ValidateWorkItemState(value.State); err != nil {
		return invalidDomain("WorkItem state is unsupported", err)
	}
	return nil
}

func validateWorkItemContext(value WorkItem) error {
	if value.ContextArtifactRef == nil {
		return nil
	}
	if value.ContextArtifactRef.SourceSnapshotRef.EntityType !=
		core.EntityType("project_snapshot") {
		return invalidDomain("WorkItem context ArtifactRef source type is invalid", nil)
	}
	if err := core.ValidateArtifactRef(value.ContextArtifactRef); err != nil {
		return invalidDomain("WorkItem context ArtifactRef is invalid", err)
	}
	if value.ContextArtifactRef.SourceSnapshotRef.EntityID != value.ProjectSnapshotID {
		return invalidDomain("WorkItem context ArtifactRef uses another snapshot", nil)
	}
	return nil
}
