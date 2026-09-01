package domain

import "sort"

// TopologicalOrder returns one lexicographically deterministic DAG order.
func TopologicalOrder(items []WorkItem) ([]string, error) {
	outgoing, indegree, err := buildTopology(items)
	if err != nil {
		return nil, err
	}
	ready := make([]string, 0, len(items))
	for itemID, count := range indegree {
		if count == 0 {
			ready = append(ready, itemID)
		}
	}
	sort.Strings(ready)
	order := make([]string, 0, len(items))
	for len(ready) != 0 {
		current := ready[0]
		ready = ready[1:]
		order = append(order, current)
		for _, dependent := range outgoing[current] {
			indegree[dependent]--
			if indegree[dependent] == 0 {
				ready = insertSorted(ready, dependent)
			}
		}
	}
	if len(order) != len(items) {
		return nil, invalidDomain("WorkGraph contains a dependency cycle", nil)
	}
	return order, nil
}

func buildTopology(items []WorkItem) (map[string][]string, map[string]int, error) {
	if len(items) < 1 || len(items) > maxWorkItems {
		return nil, nil, invalidDomain("WorkGraph item count is outside its bound", nil)
	}
	outgoing := make(map[string][]string, len(items))
	indegree := make(map[string]int, len(items))
	for _, item := range items {
		if err := validateEntityID(item.WorkItemID, "work_item", "work_item_id"); err != nil {
			return nil, nil, err
		}
		if _, exists := indegree[item.WorkItemID]; exists {
			return nil, nil, invalidDomain("WorkGraph contains a duplicate WorkItem", nil)
		}
		indegree[item.WorkItemID] = 0
	}
	edges := 0
	for _, item := range items {
		if len(item.Dependencies) > maxItemListEntries {
			return nil, nil, invalidDomain("WorkItem dependency count exceeds its bound", nil)
		}
		seen := make(map[string]struct{}, len(item.Dependencies))
		for _, dependency := range item.Dependencies {
			if err := addDependency(item.WorkItemID, dependency, seen, outgoing, indegree); err != nil {
				return nil, nil, err
			}
			edges++
			if edges > maxDependencyEdges {
				return nil, nil, invalidDomain("WorkGraph edge count exceeds its bound", nil)
			}
		}
	}
	for itemID := range outgoing {
		sort.Strings(outgoing[itemID])
	}
	return outgoing, indegree, nil
}

func addDependency(
	itemID, dependency string,
	seen map[string]struct{},
	outgoing map[string][]string,
	indegree map[string]int,
) error {
	if err := validateEntityID(dependency, "work_item", "WorkItem dependency"); err != nil {
		return err
	}
	if dependency == itemID {
		return invalidDomain("WorkItem depends on itself", nil)
	}
	if _, exists := indegree[dependency]; !exists {
		return invalidDomain("WorkItem dependency does not exist", nil)
	}
	if _, exists := seen[dependency]; exists {
		return invalidDomain("WorkItem contains a duplicate dependency", nil)
	}
	seen[dependency] = struct{}{}
	outgoing[dependency] = append(outgoing[dependency], itemID)
	indegree[itemID]++
	return nil
}

func insertSorted(values []string, value string) []string {
	index := sort.SearchStrings(values, value)
	values = append(values, "")
	copy(values[index+1:], values[index:])
	values[index] = value
	return values
}
