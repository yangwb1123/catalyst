package application

import (
	domain "forgeos/forge-core/internal/delivery/domain"
	pcstate "forgeos/forge-core/internal/platformcorecontract/state"
)

func validateProgress(order []string, items map[string]domain.WorkItem) error {
	for _, itemID := range order {
		item := items[itemID]
		if !requiresCompletedPredecessors(item.State) {
			continue
		}
		if !predecessorsCompleted(item, items) {
			return invalidSnapshot("progressed WorkItem has an incomplete predecessor", nil)
		}
	}
	return nil
}

func requiresCompletedPredecessors(state pcstate.WorkItemState) bool {
	switch string(state) {
	case string(stateReady), string(stateDispatched), string(stateRunning),
		string(stateVerifying), string(stateCompleted), string(stateFailed),
		string(stateUncertain):
		return true
	default:
		return false
	}
}

func predecessorsCompleted(
	item domain.WorkItem, items map[string]domain.WorkItem,
) bool {
	for _, predecessorID := range item.Dependencies {
		if items[predecessorID].State != stateCompleted {
			return false
		}
	}
	return true
}

func driftPresent(value domain.SnapshotDrift) bool {
	return len(value.MissingProjectIDs) != 0 || len(value.ChangedProjectIDs) != 0 ||
		len(value.UnexpectedProjectIDs) != 0
}
