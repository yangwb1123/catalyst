package domain

func validateCriticalPathDuration(items []WorkItem, order []string, maximum int64) error {
	byID := make(map[string]WorkItem, len(items))
	for _, item := range items {
		byID[item.WorkItemID] = item
	}
	longest := make(map[string]int64, len(items))
	for _, itemID := range order {
		item := byID[itemID]
		parentDuration := int64(0)
		for _, dependency := range item.Dependencies {
			if longest[dependency] > parentDuration {
				parentDuration = longest[dependency]
			}
		}
		duration, ok := addWithin(parentDuration, item.Budget.MaxDurationMS, maximum)
		if !ok {
			return invalidDomain("WorkGraph critical-path duration exceeds Change budget", nil)
		}
		longest[itemID] = duration
	}
	return nil
}
