// Package attribution holds the Eval -> scorecard -> Router learning loop's
// pure agent-role/task_type vocabulary and (model, task_type) pairing — the
// shared vocabulary the scorecard producer (cmd/forge's live wind-down after
// a `forge run`/`forge evolve`) and the disaster-recovery path
// (`forge scorecard rebuild --from <trace.jsonl>`) both need, factored out of
// cmd/forge so it stays a thin CLI-dispatch layer (see internal/doctor,
// internal/migrate, internal/mode, internal/risk for the same pattern).
//
// Everything here is pure: no file I/O, no cmd/forge dependency. Callers own
// reading trace/workflow files off disk and hand this package already-parsed
// data (asset.Workflow, trace event names).
package attribution

// AgentTaskType maps a phase's Agent role to the scorecard task_type.
// Unmapped roles (harness/gate) return ("", false) from TaskTypeForAgent and
// are skipped by every caller (the wind-down producer, the history
// tiebreaker, the rebuild path).
var AgentTaskType = map[string]string{
	"implementer": "implementation",
	"reviewer":    "reviewer",
	"qa":          "test",
	"planner":     "implementation",
	"architect":   "architecture",
}

// TaskTypeForAgent returns the task_type for an agent role and whether found.
func TaskTypeForAgent(agent string) (taskType string, ok bool) {
	tt, ok := AgentTaskType[agent]
	return tt, ok
}

// ScorecardPair is one distinct (model, task_type) a run billed against — the
// scorecard primary key (scorecard.schema.yml). The producer (cmd/forge) is
// invoked once per distinct pair.
type ScorecardPair struct {
	Model    string
	TaskType string
}
