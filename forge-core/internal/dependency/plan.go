// Package dependency computes deterministic authored-order dependency waves.
// It is the single topology scheduler shared by workflow and Group Agent Graph
// adapters; the package performs no I/O and releases no execution authority.
package dependency

import (
	"fmt"
	"strings"
)

// Node is one authored item and the identifiers it depends on.
type Node struct {
	ID           string
	Dependencies []string
}

// Waves groups authored node indices into dependency-ordered waves.
//
// Authored order is preserved within every wave. Empty or duplicate identifiers,
// unknown dependencies, self-dependencies, and cycles fail closed and return no
// waves. Repeating an existing dependency is semantically inert, matching the
// legacy workflow planner; adapters with edge-identity contracts may reject it.
func Waves(nodes []Node) ([][]int, error) {
	positions, err := nodePositions(nodes)
	if err != nil {
		return nil, err
	}
	dependencies, err := dependencyPositions(nodes, positions)
	if err != nil {
		return nil, err
	}
	return schedule(nodes, dependencies)
}

func nodePositions(nodes []Node) (map[string]int, error) {
	positions := make(map[string]int, len(nodes))
	for position, node := range nodes {
		if strings.TrimSpace(node.ID) == "" {
			return nil, fmt.Errorf("dependency: node[%d] has an empty identifier", position)
		}
		if first, exists := positions[node.ID]; exists {
			return nil, fmt.Errorf(
				"dependency: node[%d] duplicates identifier %q from node[%d]",
				position, node.ID, first,
			)
		}
		positions[node.ID] = position
	}
	return positions, nil
}

func dependencyPositions(nodes []Node, positions map[string]int) ([][]int, error) {
	result := make([][]int, len(nodes))
	for position, node := range nodes {
		for _, identifier := range node.Dependencies {
			dependency, err := resolveDependency(node, position, identifier, positions)
			if err != nil {
				return nil, err
			}
			result[position] = append(result[position], dependency)
		}
	}
	return result, nil
}

func resolveDependency(
	node Node,
	position int,
	identifier string,
	positions map[string]int,
) (int, error) {
	dependency, exists := positions[identifier]
	if !exists {
		return 0, fmt.Errorf("dependency: node %q depends on unknown node %q", node.ID, identifier)
	}
	if dependency == position {
		return 0, fmt.Errorf("dependency: node %q depends on itself", node.ID)
	}
	return dependency, nil
}

func schedule(nodes []Node, dependencies [][]int) ([][]int, error) {
	placed := make([]bool, len(nodes))
	var waves [][]int
	for placedCount := 0; placedCount < len(nodes); {
		wave := readyNodes(dependencies, placed)
		if len(wave) == 0 {
			return nil, cycleError(nodes, placed)
		}
		for _, position := range wave {
			placed[position] = true
		}
		placedCount += len(wave)
		waves = append(waves, wave)
	}
	return waves, nil
}

func readyNodes(dependencies [][]int, placed []bool) []int {
	var ready []int
	for position := range dependencies {
		if !placed[position] && allPlaced(dependencies[position], placed) {
			ready = append(ready, position)
		}
	}
	return ready
}

func allPlaced(dependencies []int, placed []bool) bool {
	for _, dependency := range dependencies {
		if !placed[dependency] {
			return false
		}
	}
	return true
}

func cycleError(nodes []Node, placed []bool) error {
	var identifiers []string
	for position, node := range nodes {
		if !placed[position] {
			identifiers = append(identifiers, node.ID)
		}
	}
	return fmt.Errorf("dependency: cycle among %s", strings.Join(identifiers, " <-> "))
}
