// rebuild.go holds the DISASTER-RECOVERY path's pure logic: reconstructing
// scorecard (model, task_type) pairs from a trace JSONL alone
// (scan-new-angles.md §方向4), used by `forge scorecard rebuild --from
// <trace.jsonl>` when scorecards.json is lost or corrupted.
//
// UNLIKE the live wind-down path (cmd/forge's distinctScorecardPairs), a
// rebuild has ONLY the trace — not the workflow definition that produced it
// — so it cannot read a phase's task_type straight off asset.Workflow.Phases.
// It must instead RE-DERIVE task_type from each trace event's bare phase
// NAME. TaskTypeForRebuildEvent does that with a two-tier strategy: prefer
// the GROUND-TRUTH phase-name -> task_type map (PhaseTaskTypes, built by the
// caller from the actual workflow definition(s) it loaded off disk), falling
// back to a substring/EqualFold heuristic ONLY for a phase name no known
// workflow names. The ground truth matters because a workflow like
// evolve.yml — the flagship autonomous loop — names its phases DIFFERENTLY
// from their agent roles (implement -> agent: implementer, review -> agent:
// reviewer, evaluate -> agent: qa, gap-analysis -> agent: architect,
// roadmap-update -> agent: planner): the substring heuristic alone finds
// NONE of these and would silently drop every real evolve-loop trace event
// from a rebuild.
package attribution

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/trace"
)

// PhaseTaskTypes builds the GROUND-TRUTH phase-name -> task_type map a
// rebuild should prefer over the substring heuristic (see
// TaskTypeForRebuildEvent), from the caller's already-loaded workflow
// definition(s) — cmd/forge's resolvePhaseTaskTypes glob-loads
// .agent/workflows/*.yml (or just the one named by --workflow) via its own
// loadWorkflow/yaml2json I/O and passes the result here. This function is
// pure: no I/O.
//
// Phase maps merge across workflows in slice order, first-workflow-seen
// winning a phase-name collision — so when the caller scans every workflow
// under .agent/workflows/*.yml (sorted for determinism), the first one
// encountered wins any name clash. A phase whose agent has no task_type
// mapping (AgentTaskType) is omitted, exactly as TaskTypeForAgent declines
// it.
func PhaseTaskTypes(workflows []asset.Workflow) map[string]string {
	result := map[string]string{}
	for _, wf := range workflows {
		for _, p := range wf.Phases {
			if _, exists := result[p.Name]; exists {
				continue // first-workflow-seen wins a phase-name collision
			}
			if tt, ok := TaskTypeForAgent(p.Agent); ok {
				result[p.Name] = tt
			}
		}
	}
	return result
}

// TaskTypeForRebuildEvent derives task_type from a trace event's phase name.
// Priority: (1) phaseTaskTypes — the GROUND-TRUTH phase-name -> task_type map
// (PhaseTaskTypes), which correctly handles a workflow (e.g. evolve.yml)
// whose phase names differ from their agent roles; (2) substring/case-
// insensitive match against AgentTaskType's agent-role keys — the legacy
// heuristic, kept as a fallback for when no known workflow names this phase
// (e.g. build.yml, whose phase names happen to equal their agent names, or a
// trace from a workflow that no longer exists); (3) TaskTypeForAgent's exact
// map as a last resort.
func TaskTypeForRebuildEvent(name string, phaseTaskTypes map[string]string) (string, bool) {
	if tt, ok := phaseTaskTypes[name]; ok {
		return tt, true
	}
	for agent, t := range AgentTaskType {
		if strings.Contains(name, agent) || strings.EqualFold(name, agent) {
			return t, true
		}
	}
	return TaskTypeForAgent(name)
}

// ExtractRebuildPairs scans a trace JSONL file for every DISTINCT (model,
// task_type) pair with a model-bearing billed cost event, deriving task_type
// from the event's phase name via TaskTypeForRebuildEvent (ground truth from
// phaseTaskTypes first, substring heuristic as fallback). Distinct pairs
// collapse, in first-seen order for a deterministic rebuild sequence (mirrors
// cmd/forge's distinctScorecardPairs).
func ExtractRebuildPairs(traceFile string, phaseTaskTypes map[string]string) ([]ScorecardPair, error) {
	f, err := os.Open(traceFile)
	if err != nil {
		return nil, fmt.Errorf("open trace: %w", err)
	}
	defer f.Close()

	seen := map[ScorecardPair]bool{}
	var pairs []ScorecardPair
	sc := bufio.NewScanner(f)
	sc.Buffer(make([]byte, 0, 64*1024), 1024*1024)
	for sc.Scan() {
		var ev trace.Event
		if err := json.Unmarshal(sc.Bytes(), &ev); err != nil {
			continue
		}
		if ev.Model == "" || ev.CostUsdMicros == 0 {
			continue
		}
		tt, ok := TaskTypeForRebuildEvent(ev.Name, phaseTaskTypes)
		if !ok {
			continue
		}
		p := ScorecardPair{Model: ev.Model, TaskType: tt}
		if seen[p] {
			continue
		}
		seen[p] = true
		pairs = append(pairs, p)
	}
	if err := sc.Err(); err != nil {
		return nil, fmt.Errorf("scan trace: %w", err)
	}
	return pairs, nil
}
