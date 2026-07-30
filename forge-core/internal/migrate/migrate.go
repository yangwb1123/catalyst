// Package migrate distills .agent/policies/modes.yml's `migrations` block into a
// runnable Plan — the central knob's headline ACTION, the "startup -> enterprise"
// state transition (explorer -> engineering). It is the SAME play as
// internal/mode and internal/routing: a compact, deterministic, pure-Go
// distillation of the declarative YAML, enough for forge-core to PRINT the
// governance upgrade and DERIVE the remediation tasks, not the full PDP (that is
// the v2+ policy service that will consume modes.yml directly).
//
// A migration is not a flag flip. modes.yml frames explorer -> engineering as a
// governance upgrade that (1) tightens the harness (all gates, 80% coverage,
// warn -> block), (2) raises the router floor (haiku -> sonnet), (3) enables the
// skipped workflow rigor (discover/adr/reviewer), AND (4) DERIVES "backfill"
// tasks for the debt the prototype phase skipped — the same gap -> roadmap move
// evolve makes. This package encodes (1)-(4) as data; the cmd layer prints the
// Plan (dry) and, only on --apply, flips project.yml's mode and injects the
// derived tasks into ROADMAP.md.
//
// HONESTY — scope of THIS slice:
//   - v1 implements the ONE migration modes.yml actually declares:
//     explorer -> engineering. There is no other migration in the data, so there
//     is no generic engine here — when modes.yml grows a second migration, this
//     becomes a table (today a single function is the honest shape).
//   - the manual transition remains available, while PromoteToProduction models
//     the adopted persistent lifecycle trigger. Transient run/evolve flags do
//     not call it and therefore never mutate project state.
//   - The derived Tasks are REMEDIATION DEBT injected into the roadmap; this
//     package neither executes them nor claims they are done. Doing the work is
//     the job of a later build/evolve pass over the injected roadmap items.
package migrate

import "fmt"

// Mode names — the engineering postures from modes.yml's `modes` block. v1's
// single migration goes between exactly these two.
const (
	ModeExplorer    = "explorer"
	ModeBalanced    = "balanced"
	ModeEngineering = "engineering"
	ModeCTO         = "cto"
)

const (
	LifecycleIdea       = "idea"
	LifecycleMVP        = "mvp"
	LifecycleGrowth     = "growth"
	LifecycleProduction = "production"
)

// Tier names mirror internal/routing (v1 is Claude-only). RouterFloor below is
// one of these — the cheap default a migration lifts.
const (
	tierHaiku  = "haiku"
	tierSonnet = "sonnet"
)

// Enforcement levels mirror modes.yml's harness.enforce. A migration tightens
// from warn (advisory) to block (hard-stop).
const (
	enforceWarn  = "warn"
	enforceBlock = "block"
)

// Task is one derived "backfill" remediation item from modes.yml's
// migrations.<m>.derive_tasks. It is injected into ROADMAP.md on --apply (the
// gap -> roadmap move). Gate is the harness gate this debt pays down ("" when the
// task is not tied to a single gate, e.g. add-ci / add-monitoring — modes.yml
// omits `gate:` for those).
type Task struct {
	ID       string // stable identifier (modes.yml derive_tasks[*].id)
	Title    string // human-readable backfill task (derive_tasks[*].title)
	Gate     string // harness gate this pays down, or "" if not gate-scoped
	Priority string // high | medium (derive_tasks[*].priority)
}

// Plan is the fully distilled, effective migration for one mode transition: the
// tightened harness (gate-set, coverage, enforcement), the raised router floor,
// the workflow rigor re-enabled, and the remediation Tasks to inject. It is the
// pure value the cmd layer renders (dry) and applies (--apply). Deterministic and
// self-contained — the cmd layer reads it, never re-derives policy.
type Plan struct {
	From string // source mode (explorer)
	To   string // target mode (engineering)

	// ── effects.tighten_harness ──
	TightenGates      []string // harness.gates after the upgrade (the full catalog)
	CoverageThreshold int      // harness.coverage_threshold (%)
	Enforce           string   // harness.enforce: warn -> block

	// ── effects.raise_router_floor ──
	RouterFloor string // the new cheap-default tier floor (haiku -> sonnet)

	// ── effects.enable_workflow ── the skipped rigor restored. Modeled as the
	// three booleans modes.yml's enable_workflow names (discover:full / adr:true
	// / reviewer:true); DiscoverFull captures discover going skip -> full.
	DiscoverFull bool
	ADR          bool
	Reviewer     bool

	// ── derive_tasks ── the remediation debt to inject into ROADMAP.md.
	Tasks []Task
}

// Promotion is the pure state transition for a persistent lifecycle promotion
// to production. Explorer projects additionally receive the declared
// explorer->engineering governance migration; all other known modes retain
// their mode and do not receive prototype-debt tasks. AlreadyProduction is a
// no-op/replay signal: an invocation cannot infer a historical transition and
// must never retroactively migrate an explorer that was already production.
type Promotion struct {
	FromMode          string
	ToMode            string
	FromLifecycle     string
	ToLifecycle       string
	Migration         Plan
	AutoMigration     bool
	AlreadyProduction bool
}

// PromoteToProduction validates and derives the only persistent lifecycle
// promotion currently supported. Unknown selectors fail closed. A production
// source is an exact no-op regardless of mode; only a real non-production to
// production edge may auto-trigger explorer->engineering.
func PromoteToProduction(mode, lifecycle string) (Promotion, error) {
	if !knownMode(mode) {
		return Promotion{}, fmt.Errorf("unknown persistent mode %q", mode)
	}
	if !knownLifecycle(lifecycle) {
		return Promotion{}, fmt.Errorf("unknown persistent lifecycle %q", lifecycle)
	}
	promotion := Promotion{
		FromMode: mode, ToMode: mode,
		FromLifecycle: lifecycle, ToLifecycle: LifecycleProduction,
	}
	if lifecycle == LifecycleProduction {
		promotion.AlreadyProduction = true
		return promotion, nil
	}
	if mode == ModeExplorer {
		promotion.ToMode = ModeEngineering
		promotion.Migration = ExplorerToEngineering()
		promotion.AutoMigration = true
	}
	return promotion, nil
}

func knownMode(value string) bool {
	switch value {
	case ModeExplorer, ModeBalanced, ModeEngineering, ModeCTO:
		return true
	default:
		return false
	}
}

func knownLifecycle(value string) bool {
	switch value {
	case LifecycleIdea, LifecycleMVP, LifecycleGrowth, LifecycleProduction:
		return true
	default:
		return false
	}
}

// fullGates is modes.yml's complete gate_catalog (ascending rigor) — the exact
// set migrations.explorer_to_engineering.effects.tighten_harness.gates enables.
// Kept as a private list so ExplorerToEngineering hands every caller a fresh copy
// (no shared backing array to mutate), the same ownership discipline as
// internal/mode.allGates.
var fullGates = []string{"lint", "test", "build", "complexity", "arch", "security"}

// deriveTasks is the verbatim distillation of modes.yml
// migrations.explorer_to_engineering.derive_tasks (order preserved). Each entry
// mirrors one task's id/title/gate/priority. add-ci and add-monitoring carry no
// `gate:` in the YAML, so their Gate is "" here — honestly un-scoped, not faked
// onto a gate. This is the single source the Plan copies from.
var deriveTasks = []Task{
	{ID: "backfill-tests", Title: "为现有代码补测试至覆盖率阈值 / backfill tests to coverage threshold", Gate: "test", Priority: "high"},
	{ID: "add-ci", Title: "加 CI 流水线跑全 harness 闸门 / add CI running the full harness", Gate: "", Priority: "high"},
	{ID: "add-monitoring", Title: "加可观测(日志/指标/告警)/ add observability (logs/metrics/alerts)", Gate: "", Priority: "medium"},
	{ID: "refactor-oversized", Title: "拆分原型期超阈值文件/函数 / split prototype files over size caps", Gate: "complexity", Priority: "medium"},
	{ID: "security-pass", Title: "跑依赖扫描 + SAST 补安全债 / dependency scan + SAST remediation", Gate: "security", Priority: "high"},
}

// ExplorerToEngineering returns the distilled Plan for modes.yml's only declared
// migration, migrations.explorer_to_engineering. Every field is copied verbatim
// from that block:
//
//	effects.tighten_harness  -> TightenGates=full catalog, CoverageThreshold=80,
//	                            Enforce=block (was warn)
//	effects.raise_router_floor: sonnet  -> RouterFloor (was haiku)
//	effects.enable_workflow  -> DiscoverFull (discover skip->full), ADR, Reviewer
//	derive_tasks             -> Tasks (all five, order preserved)
//
// Pure, deterministic, and self-owning: the returned Plan carries fresh slices,
// so a caller mutating Plan.TightenGates / Plan.Tasks can never reach back into
// this package's tables.
func ExplorerToEngineering() Plan {
	return Plan{
		From:              ModeExplorer,
		To:                ModeEngineering,
		TightenGates:      cloneGates(),
		CoverageThreshold: 80,
		Enforce:           enforceBlock,
		RouterFloor:       tierSonnet,
		DiscoverFull:      true,
		ADR:               true,
		Reviewer:          true,
		Tasks:             cloneTasks(),
	}
}

// cloneGates returns a fresh copy of the full gate catalog so each Plan owns its
// slice (no shared backing array).
func cloneGates() []string {
	out := make([]string, len(fullGates))
	copy(out, fullGates)
	return out
}

// cloneTasks returns a fresh copy of the derived-task table so a caller mutating
// the returned Plan.Tasks cannot corrupt the package-level source.
func cloneTasks() []Task {
	out := make([]Task, len(deriveTasks))
	copy(out, deriveTasks)
	return out
}
