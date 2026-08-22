package goimpactprescan

import (
	"fmt"
	"sort"
)

func reverseClosure(index *graphIndex, seeds map[string]struct{}) (
	[]ReachableNode, []ReachableEdge, error,
) {
	witnesses := seedWitnesses(seeds)
	frontier := sortedSetKeys(seeds)
	aggregateHops := 0
	for len(frontier) != 0 {
		next, nextHops, err := expandFrontier(index, frontier, witnesses, aggregateHops)
		if err != nil {
			return nil, nil, err
		}
		aggregateHops += nextHops
		frontier = sortedWitnessKeys(next)
		for nodeID, witness := range next {
			witnesses[nodeID] = witness
		}
		if len(witnesses) > maxReachableNodes {
			return nil, nil, fmt.Errorf("reachable nodes exceed %d", maxReachableNodes)
		}
	}
	return materializeClosure(index, witnesses)
}

func seedWitnesses(seeds map[string]struct{}) map[string]Witness {
	result := make(map[string]Witness, len(seeds))
	for nodeID := range seeds {
		result[nodeID] = Witness{
			EdgeIDs: []string{}, HopCount: 0, NodeIDs: []string{nodeID}, SeedNodeID: nodeID,
		}
	}
	return result
}

func expandFrontier(
	index *graphIndex,
	frontier []string,
	known map[string]Witness,
	aggregateHops int,
) (map[string]Witness, int, error) {
	next := make(map[string]Witness)
	nextHops := 0
	for _, targetID := range frontier {
		for _, edge := range index.reverse[targetID] {
			if _, exists := known[edge.FromNodeID]; exists {
				continue
			}
			candidate, err := extendWitness(known[targetID], edge)
			if err != nil {
				return nil, 0, err
			}
			current, exists := next[edge.FromNodeID]
			if !exists && candidate.HopCount > maxAggregateWitnesses-aggregateHops-nextHops {
				return nil, 0, fmt.Errorf("aggregate witness hops exceed %d", maxAggregateWitnesses)
			}
			if !exists || witnessLess(candidate, current) {
				next[edge.FromNodeID] = candidate
				if !exists {
					nextHops += candidate.HopCount
				}
			}
		}
	}
	return next, nextHops, nil
}

func extendWitness(value Witness, edge ReachableEdge) (Witness, error) {
	if value.HopCount >= maxWitnessHops {
		return Witness{}, fmt.Errorf("witness exceeds %d hops", maxWitnessHops)
	}
	return Witness{
		EdgeIDs: appendCopy(value.EdgeIDs, edge.EdgeID), HopCount: value.HopCount + 1,
		NodeIDs: appendCopy(value.NodeIDs, edge.FromNodeID), SeedNodeID: value.SeedNodeID,
	}, nil
}

func appendCopy(values []string, value string) []string {
	result := make([]string, len(values)+1)
	copy(result, values)
	result[len(values)] = value
	return result
}

func witnessLess(left, right Witness) bool {
	if left.HopCount != right.HopCount {
		return left.HopCount < right.HopCount
	}
	if left.SeedNodeID != right.SeedNodeID {
		return left.SeedNodeID < right.SeedNodeID
	}
	if comparison := compareSequence(left.EdgeIDs, right.EdgeIDs); comparison != 0 {
		return comparison < 0
	}
	return compareSequence(left.NodeIDs, right.NodeIDs) < 0
}

func compareSequence(left, right []string) int {
	for index := 0; index < len(left) && index < len(right); index++ {
		if left[index] < right[index] {
			return -1
		}
		if left[index] > right[index] {
			return 1
		}
	}
	if len(left) < len(right) {
		return -1
	}
	if len(left) > len(right) {
		return 1
	}
	return 0
}

func sortedSetKeys(values map[string]struct{}) []string {
	result := make([]string, 0, len(values))
	for value := range values {
		result = append(result, value)
	}
	sort.Strings(result)
	return result
}

func sortedWitnessKeys(values map[string]Witness) []string {
	result := make([]string, 0, len(values))
	for value := range values {
		result = append(result, value)
	}
	sort.Slice(result, func(i, j int) bool {
		left, right := values[result[i]], values[result[j]]
		if witnessLess(left, right) {
			return true
		}
		if witnessLess(right, left) {
			return false
		}
		return result[i] < result[j]
	})
	return result
}

func materializeClosure(index *graphIndex, witnesses map[string]Witness) (
	[]ReachableNode, []ReachableEdge, error,
) {
	identities := make([]string, 0, len(witnesses))
	for nodeID := range witnesses {
		identities = append(identities, nodeID)
	}
	sort.Strings(identities)
	nodes := make([]ReachableNode, 0, len(identities))
	for _, nodeID := range identities {
		node, exists := index.nodesByID[nodeID]
		if !exists {
			return nil, nil, fmt.Errorf("reachable node is absent")
		}
		node.Witness = witnesses[nodeID]
		nodes = append(nodes, node)
	}
	edges := inducedEdges(index.edges, witnesses)
	return nodes, edges, nil
}

func inducedEdges(values []ReachableEdge, nodes map[string]Witness) []ReachableEdge {
	result := make([]ReachableEdge, 0)
	for _, edge := range values {
		_, fromExists := nodes[edge.FromNodeID]
		_, toExists := nodes[edge.ToNodeID]
		if fromExists && toExists {
			result = append(result, edge)
		}
	}
	return result
}
