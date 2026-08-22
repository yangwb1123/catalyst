package authenticatedadrlifecyclecontract

import "fmt"

func validateGraph(state map[string]map[string]any) error {
	if len(state) > maxDecisions {
		return fmt.Errorf("materialized state exceeds %d decisions", maxDecisions)
	}
	for _, decision := range state {
		for _, target := range decision["supersedes"].([]any) {
			if _, exists := state[target.(string)]; !exists {
				return fmt.Errorf("materialized supersedes edge is dangling")
			}
		}
	}
	visiting, visited := map[string]bool{}, map[string]bool{}
	for adr := range state {
		if err := visitDecision(adr, state, visiting, visited); err != nil {
			return err
		}
	}
	return nil
}

func visitDecision(adr string, state map[string]map[string]any,
	visiting, visited map[string]bool) error {
	if visiting[adr] {
		return fmt.Errorf("lifecycle supersession graph contains a cycle")
	}
	if visited[adr] {
		return nil
	}
	visiting[adr] = true
	for _, target := range state[adr]["supersedes"].([]any) {
		if err := visitDecision(target.(string), state, visiting, visited); err != nil {
			return err
		}
	}
	delete(visiting, adr)
	visited[adr] = true
	return nil
}
