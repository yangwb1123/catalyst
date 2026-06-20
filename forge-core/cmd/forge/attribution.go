// attribution.go — the cmd/forge map from a workflow's AGENT role to the scorecard
// `task_type` its work is attributed under. This is the producer-side counterpart of
// the routing consumer's primary_key [model, task_type] (scorecard.schema.yml): the
// learning-loop wind-down (scorecard_wind.go) needs, for each phase it bills, a
// task_type to key the persisted row on, and the only thing a phase carries is its
// Agent. This lookup is the bridge — and it lives HERE, never in the orchestrator,
// which stays free of any scorecard/task_type concept (the same bright-line cost.go
// draws for claude-JSON cost and prompt_context.go for the prompt ledgers).
//
// HONESTY — this is a deliberately LOSSY proxy, not a true per-task classification:
//   - agent -> task_type is a COARSE stand-in. A single `implementer` role covers what
//     policy.yml's tiers split across crud / implementation / refactor_medium / bugfix;
//     we fold all of it into the one task_type "implementation" because the workflow only
//     tells us the ROLE, not which flavor of implementation this phase did. The scorecard
//     row is therefore "implementer-as-a-whole", not per-CRUD-vs-refactor.
//   - `planner` is folded into "implementation" ON PURPOSE rather than mapped to
//     "requirements": requirements carries policy.yml's opus floor, and attributing a
//     planner's cost under requirements would distort that band's economics with planning
//     spend. Planning is upstream-of-implementation work; "implementation" is the honest
//     bucket until v3 gives planning its own task_type.
//   - architect -> "architecture", reviewer -> "reviewer", qa -> "test" are the direct,
//     unambiguous mappings (each role IS that task_type in policy.yml's table).
//   - a role with NO entry here (notably the harness/gate phases, which are not LLM agents
//     and never bill) returns ("", false): the wind-down SKIPS it, so a non-LLM phase is
//     never attributed a task_type it has no business owning.
//
// A finer per-task classification (reading the actual change kind from a phase's output)
// is v3 work; this map is the honest v1 floor that closes the cost/attribution loop.
package main

// agentTaskType maps a phase's Agent role to the scorecard task_type its cost/quality is
// recorded under. The keys are the agent role names ForgeOS's .agent/agents/*.md cards use
// (the same strings asset.Phase.Agent carries); the values are task_type enum members of
// scorecard.schema.yml. It is intentionally PARTIAL — an unmapped role (a harness/gate
// phase) is the signal "do not attribute", surfaced by the comma-ok lookup below.
var agentTaskType = map[string]string{
	"implementer": "implementation",
	"reviewer":    "reviewer",
	"qa":          "test",
	"planner":     "implementation", // folded into implementation, NOT requirements (see file header)
	"architect":   "architecture",
}

// taskTypeForAgent returns the scorecard task_type a given agent role's work is attributed
// under, and whether a mapping exists. ok=false (with "") for any role not in the proxy
// map — the wind-down reads this as "skip", so a non-LLM harness/gate phase is never
// recorded against a fabricated task_type. Pure and total; a nil/empty agent yields
// ("", false) exactly like any other unmapped role.
func taskTypeForAgent(agent string) (taskType string, ok bool) {
	tt, ok := agentTaskType[agent]
	return tt, ok
}
