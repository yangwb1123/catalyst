// waves.go adapts workflow assets to the shared dependency-wave planner.
package orchestrator

import (
	"fmt"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/dependency"
)

// Waves groups phase INDICES into dependency-ordered execution waves. Within a wave
// the phases are independent (no depends_on edge between them), so a parallel engine
// may run them concurrently; waves themselves run in order. Authored order is
// preserved within each wave for a deterministic schedule.
//
// FAIL-CLOSED (a governance runtime must never silently run a malformed graph): an
// unknown dependency name, a self-dependency, or a CYCLE among the phases returns an
// error and NO waves — the caller aborts rather than guessing an order. A workflow
// with NO depends_on at all yields a single wave of every phase in authored order
// (the parallel engine then runs them all concurrently — the pure fan-out case).
func Waves(phases []asset.Phase) ([][]int, error) {
	if err := asset.ValidateWorkflowStructure(asset.Workflow{Phases: phases}); err != nil {
		return nil, fmt.Errorf("invalid workflow structure: %w", err)
	}
	nodes := make([]dependency.Node, len(phases))
	for position, phase := range phases {
		nodes[position] = dependency.Node{
			ID:           phase.Name,
			Dependencies: phase.DependsOn,
		}
	}
	waves, err := dependency.Waves(nodes)
	if err != nil {
		return nil, fmt.Errorf("invalid workflow dependencies: %w", err)
	}
	return waves, nil
}
