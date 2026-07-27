// waves.go — the dependency-order planner for the OPT-IN parallel orchestrator.
// Pure (no I/O, no concurrency): it turns a workflow's phases + their depends_on
// declarations into ordered WAVES, where every phase in wave k has all its
// dependencies satisfied by phases in earlier waves, and the phases WITHIN a wave
// are mutually independent (safe to run concurrently). RunParallel (loop.go's
// sibling) consumes this; the serial RunFrom ignores depends_on entirely.
package orchestrator

import (
	"fmt"
	"strings"

	"forgeos/forge-core/internal/asset"
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
	idx := make(map[string]int, len(phases))
	for i, p := range phases {
		idx[p.Name] = i
	}
	deps := make([][]int, len(phases))
	for i, p := range phases {
		for _, d := range p.DependsOn {
			j, ok := idx[d]
			if !ok {
				return nil, fmt.Errorf("phase %q depends_on unknown phase %q", p.Name, d)
			}
			if j == i {
				return nil, fmt.Errorf("phase %q depends_on itself", p.Name)
			}
			deps[i] = append(deps[i], j)
		}
	}
	placed := make([]bool, len(phases))
	placedCount := 0
	var waves [][]int
	for placedCount < len(phases) {
		var wave []int
		for i := range phases {
			if placed[i] || !depsPlaced(deps[i], placed) {
				continue
			}
			wave = append(wave, i) // ready: every dependency is already placed
		}
		if len(wave) == 0 {
			// No phase became ready yet phases remain -> a cycle among the rest.
			return nil, fmt.Errorf("depends_on cycle: %s", strings.Join(unplacedNames(phases, placed), " <-> "))
		}
		for _, i := range wave {
			placed[i] = true
		}
		placedCount += len(wave)
		waves = append(waves, wave)
	}
	return waves, nil
}

// depsPlaced reports whether every dependency index of a phase is already placed in
// an earlier wave (so the phase is ready for the wave being built).
func depsPlaced(deps []int, placed []bool) bool {
	for _, j := range deps {
		if !placed[j] {
			return false
		}
	}
	return true
}

// unplacedNames lists the still-unplaced phase names, for an honest cycle error.
func unplacedNames(phases []asset.Phase, placed []bool) []string {
	var names []string
	for i, p := range phases {
		if !placed[i] {
			names = append(names, p.Name)
		}
	}
	return names
}
