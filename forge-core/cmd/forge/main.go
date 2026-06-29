// Command forge is the forge-core CLI: a zero-dependency (Go stdlib only) driver
// for ForgeOS workflows and the real harness gates.
//
// `forge run <workflow>` loads .agent/workflows/<workflow>.yml, transcodes it to
// JSON via `python3 harness/yaml2json.py`, and runs it through the orchestrator
// with the real harness gates. Agent phases use --executor: "dry" narrates the
// routing decision (no LLM); "command" builds a per-phase prompt — role card +
// retrieved project context + cross-session memory — and drives --agent-cmd with
// it (`claude -p` for real, `echo` to inspect the plumbing). `forge gate|check|
// accept` delegate directly to that harness gate and exit with its status (0==OK).
// The repo root is --root, else $FORGE_REPO_ROOT, else cwd; per-gate resolution +
// acceptance probing live in gates.go.
package main

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
)

// maxLoopBack is the conservative ceiling on DIRECTED gate loop-backs per run
// (orchestrator.Engine.MaxLoopBack): a gate phase declaring on_fail:{loop_back}
// may bounce back to its target phase at most this many times before a still-red
// gate aborts the run (fail-closed). Kept small on purpose — loop-back is a
// recovery path, not a retry-until-green crutch — and it only ever engages for a
// workflow whose gate phases actually carry an on_fail (build.yml's do).
const maxLoopBack = 3

// defaultAgentAllowedTools is the claude --allowedTools whitelist a print-mode
// implementer gets by default: ONLY the two read-only self-verification commands the
// completion discipline requires — `node --test <file>` (run the tests it wrote) and
// `node harness/gate.mjs` (the size/volume self-check) — so a headless `claude -p`
// agent can self-verify and then honestly tick a ROADMAP [x] instead of stalling for a
// human to approve each Bash call (which never comes, so RoadmapCompletion sits at 0%).
// The `*` lets claude match the trailing argument (the test path / gate args).
//
// ★These are READ-ONLY validators that change no state and spawn no agent. The whitelist
// MUST NEVER contain `forge` or any command that can re-invoke an agent (`forge run/
// evolve --executor=command`): the recursion guard (command_executor.go's
// FORGE_AGENT_DEPTH/MaxDepth) only meters agents spawned THROUGH the executor, so a
// whitelisted `forge` would let an agent fork another agent outside that counter — the
// guard never fires and budget burns unbounded (a fork-bomb). `node` spawns no forge.★
const defaultAgentAllowedTools = "Bash(node --test*) Bash(node harness/gate.mjs*)"

func main() {
	os.Exit(run(os.Args[1:]))
}

// run dispatches a subcommand and returns the process exit code, so main stays
// a one-liner and the dispatch is testable.
func run(args []string) int {
	if len(args) == 0 {
		usage()
		return 2
	}
	cmd, rest := args[0], args[1:]
	switch cmd {
	case "run":
		return cmdRun(rest)
	case "gate":
		return delegate(gate.Gate, rest)
	case "check":
		return delegate(gate.Check, rest)
	case "accept":
		return delegate(gate.Accept, rest)
	case "evolve":
		return cmdEvolve(rest)
	case "route":
		return cmdRoute(rest)
	case "migrate":
		return cmdMigrate(rest)
	case "detect":
		return cmdDetect(rest)
	case "-h", "--help", "help":
		usage()
		return 0
	default:
		fmt.Fprintf(os.Stderr, "forge: unknown command %q\n", cmd)
		usage()
		return 2
	}
}

func usage() {
	fmt.Fprint(os.Stderr, `forge — ForgeOS orchestration runtime (forge-core)

usage:
  forge run    <workflow> [--mode balanced] [--lifecycle mvp] [--executor dry|command] [--agent-cmd claude] [--agent-permission acceptEdits] [--agent-allowed-tools "..."] [--agent-max-budget-usd ""] [--run-budget-usd ""] [--timeout 0] [--max-retries 0] [--max-agent-depth 2] [--max-agent-calls 0] [--max-output-bytes 0] [--approved] [--root DIR]
  forge evolve <workflow> [--mode balanced] [--lifecycle mvp] [--max-iter 5] [--executor dry|command] [--agent-cmd claude] [--agent-permission acceptEdits] [--agent-allowed-tools "..."] [--agent-max-budget-usd ""] [--run-budget-usd ""] [--timeout 0] [--max-retries 0] [--max-agent-depth 2] [--max-agent-calls 0] [--max-output-bytes 0] [--resume] [--root DIR]
  forge route  [--complexity F] [--risk-score F] [--security F] [--dependency F] [--context F] [--business F] [--task-type T] [--risk low|medium|high|critical] [--budget F] [--scorecard PATH]
  forge migrate --to engineering [--apply] [--root DIR]
  forge detect [--root DIR]
  forge gate   [--root DIR]
  forge check  [--root DIR]
  forge accept [--root DIR]
`)
}

// delegate runs one harness gate, prints its output, and maps OK to exit code.
func delegate(fn func(root string) gate.Result, args []string) int {
	fs := flag.NewFlagSet("gate", flag.ContinueOnError)
	root := fs.String("root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	if err := fs.Parse(args); err != nil {
		return 2
	}
	res := fn(*root)
	if res.Output != "" {
		fmt.Println(res.Output)
	}
	if res.OK {
		return 0
	}
	return 1
}

// runOpts holds the parsed `forge run` / `forge evolve` flags.
type runOpts struct {
	mode string
	// lifecycle is the project-maturity modifier (idea|mvp|growth|production) the
	// central knob composes with mode to produce the Workflow-depth policy. An
	// empty value (the flag default) means "read it from <root>/.agent/project.yml,
	// falling back to mvp" — see resolveLifecycle. production forces full
	// enforcement regardless of mode (the safety veto in mode.Effective).
	lifecycle string
	root      string
	executor  string
	agentCmd  string
	// agentPermission is the --permission-mode passed to a claude-family agent so it
	// can USE tools (write files) under --executor=command. Default acceptEdits:
	// auto-accept file edits (the implementer writes code) WITHOUT opening arbitrary
	// Bash — the safe middle for autonomous code edits. Without it, claude -p print
	// mode only DESCRIBES the edits it cannot apply (headless = no permission prompt
	// to answer). Empty disables the flag; only applied when agent-cmd is claude.
	agentPermission string
	// agentAllowedTools is the claude --allowedTools whitelist passed to a
	// claude-family agent under --executor=command: the read-only self-verification
	// commands a print-mode (`claude -p`) agent may run WITHOUT a human approving each
	// Bash call. acceptEdits auto-applies Write but still gates Bash, and a headless
	// agent has no one to approve it — so without this the implementer cannot run
	// `node --test`/`node harness/gate.mjs` to self-check the code it wrote, refuses to
	// tick a ROADMAP [x] under the completion discipline, and convergence's
	// RoadmapCompletion stays 0% (the run burns to max-iter). Default = the node
	// validation whitelist (defaultAgentAllowedTools); a non-node project overrides it
	// (e.g. pytest/vitest). ★MUST stay read-only and MUST NEVER include `forge` or any
	// agent-spawning command — that would bypass the FORGE_AGENT_DEPTH recursion guard
	// (a fork-bomb).★ Empty disables the flag; only applied when agent-cmd is claude.
	agentAllowedTools string
	// agentMaxBudgetUSD caps the dollar spend of ONE claude call (claude
	// --max-budget-usd, print mode) — the THIRD cost dimension, per-phase dollars,
	// complementing --max-agent-calls (phase count) and --timeout (wall-clock).
	// Empty = unset (no per-call ceiling); only applied when agent-cmd is claude.
	agentMaxBudgetUSD string
	// runBudgetUSD is the RUN-LEVEL cumulative dollar cap: the sum of every phase's
	// billed cost across the WHOLE run (and across ALL iterations of `forge evolve`)
	// must stay under it, else the run STOPS fail-closed before the next agent spawn.
	// DISTINCT from agentMaxBudgetUSD, which is PER-CLAUDE-CALL (one phase's ceiling,
	// passed to `claude --max-budget-usd`); this is the missing TOTAL bound no per-call
	// or count cap provides. Empty = unset (no cumulative cap, byte-for-byte the prior
	// behavior). All dollar arithmetic lives in cost.go's runBudget; the orchestrator
	// only ever sees the opaque bool it yields.
	runBudgetUSD string
	// timeout bounds a single agent command's wall-clock runtime (0 = no deadline,
	// the backward-compatible default). Plumbed into CommandExecutor.Timeout so a
	// wedged agent is killed and surfaces as a retryable Timeout, not a hang.
	timeout time.Duration
	// maxRetries is the per-agent-phase retry ceiling for RETRYABLE failures (0 =
	// no retries, backward-compatible default: first error aborts). Plumbed into
	// Engine.MaxRetries so a transient timeout retries while a permanent failure aborts.
	maxRetries int
	// maxAgentDepth caps nested agent spawns for --executor=command (0 = safe
	// default 2). Plumbed into CommandExecutor.MaxDepth so a real agent that
	// re-invokes forge cannot recurse unboundedly (a fork-bomb). See that field.
	maxAgentDepth int
	// maxAgentCalls is the per-run ceiling on agent-phase EXECUTIONS for
	// --executor=command (0 = unbounded, backward-compatible default). Plumbed into
	// Engine.MaxAgentCalls — the paired prerequisite to maxAgentDepth: depth bounds
	// nesting (fork-bomb), this bounds the TOTAL agent spawns of one run (loop-back
	// re-runs included), the predictable cost bound. For `forge evolve` it is
	// PER-ITERATION (the RunFrom-local counter resets each iteration), so total evolve
	// spend is bounded by max-iter × this. See that field.
	maxAgentCalls int
	// maxOutputBytes caps retained agent stdout+stderr per command for
	// --executor=command (0 = safe default 10 MiB). Plumbed into
	// CommandExecutor.MaxOutputBytes so a runaway agent's unbounded output cannot OOM
	// the orchestrator — the resource guard's third dimension (output size) alongside
	// max-agent-depth (recursion) and timeout (wall-clock).
	maxOutputBytes int
	// approved is the human-approval signal for a human_gate workflow (design):
	// --approved on the command line is one of the two approval sources (the other
	// is a <root>/.forge/<stage>.approved marker). Default false: an unapproved
	// human_gate honestly awaits a human and never auto-converges. Irrelevant to
	// conjunction/external stops, so it leaves those runs unchanged.
	approved bool
	// parallel OPTS INTO the concurrent orchestrator (orchestrator.RunParallel): a
	// workflow's depends_on declarations group phases into dependency WAVES and the
	// independent phases within a wave run concurrently. SAFE-BY-DEFAULT: it takes
	// effect ONLY for a workflow that DECLARES depends_on (declaresDependsOn) — a
	// spine workflow with no declared deps would be wrong to run all-concurrent, so it
	// stays on the serial engine (with a note). Default false = serial, byte-for-byte.
	parallel bool
}

// bindRunOpts registers the flags shared by `forge run` and `forge evolve` onto
// fs, writing into o — one definition so both subcommands stay in lockstep.
func bindRunOpts(fs *flag.FlagSet, o *runOpts) {
	fs.StringVar(&o.mode, "mode", "balanced", "engineering mode (explorer|balanced|engineering|cto)")
	fs.StringVar(&o.lifecycle, "lifecycle", "", "maturity modifier (idea|mvp|growth|production); empty = read .agent/project.yml, else mvp")
	fs.StringVar(&o.root, "root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	fs.StringVar(&o.executor, "executor", "dry", "agent executor: dry|command")
	fs.StringVar(&o.agentCmd, "agent-cmd", "claude", "command for --executor=command (e.g. claude, echo)")
	fs.StringVar(&o.agentPermission, "agent-permission", "acceptEdits", "claude --permission-mode for --executor=command (acceptEdits|plan|default); lets the agent write code headlessly")
	fs.StringVar(&o.agentAllowedTools, "agent-allowed-tools", defaultAgentAllowedTools, "claude --allowedTools whitelist (space/comma-separated) so a print-mode agent can SELF-VERIFY the code it wrote (run tests/gate) and honestly tick a ROADMAP [x]; default is the node test+gate self-check. READ-ONLY validators only — NEVER add forge or any agent-spawning command (it bypasses the recursion guard). Override for non-node projects (e.g. pytest/vitest); empty disables")
	fs.StringVar(&o.agentMaxBudgetUSD, "agent-max-budget-usd", "", "per-claude-call dollar ceiling (claude --max-budget-usd; empty = unset); the per-phase cost bound complementing --max-agent-calls/--timeout")
	fs.StringVar(&o.runBudgetUSD, "run-budget-usd", "", "cumulative run-level dollar cap across ALL phases/iterations (empty = unset); STOPS the run before overspend — distinct from the per-call --agent-max-budget-usd")
	fs.DurationVar(&o.timeout, "timeout", 0, "per-agent-command timeout (0 = no deadline, e.g. 90s, 5m)")
	fs.IntVar(&o.maxRetries, "max-retries", 0, "retry ceiling for retryable agent failures (0 = no retries)")
	fs.IntVar(&o.maxAgentDepth, "max-agent-depth", 0, "nested agent-spawn cap for --executor=command (0 = safe default 2; prevents recursive fork-bombs)")
	fs.IntVar(&o.maxAgentCalls, "max-agent-calls", 0, "per-run ceiling on agent-phase executions for --executor=command (0 = unbounded; for evolve this is PER-ITERATION, total <= max-iter x this)")
	fs.IntVar(&o.maxOutputBytes, "max-output-bytes", 0, "cap on retained agent stdout+stderr per command (0 = safe default 10MiB; prevents a runaway-output OOM)")
	fs.BoolVar(&o.approved, "approved", false, "supply the human-approval signal for a human_gate workflow (or create <root>/.forge/<stage>.approved)")
	fs.BoolVar(&o.parallel, "parallel", false, "run a workflow's depends_on-independent phases CONCURRENTLY (dependency waves); takes effect ONLY for a workflow that declares depends_on (else stays serial). No directed loop-back in parallel mode.")
}

// declaresDependsOn reports whether ANY phase in the workflow declares depends_on — the
// gate for --parallel. A workflow with no declared deps is NOT run concurrently (running a
// sequential spine all-at-once would be wrong); only an explicitly dependency-structured
// workflow opts into RunParallel.
func declaresDependsOn(wf asset.Workflow) bool {
	for _, p := range wf.Phases {
		if len(p.DependsOn) > 0 {
			return true
		}
	}
	return false
}

// parallelEnabled reports whether --parallel should drive the CONCURRENT engine for this
// workflow: true ONLY when --parallel is set AND the workflow declares depends_on. When
// --parallel is set but the workflow declares none, it logs that the flag was IGNORED
// (running a no-deps sequential spine all-at-once would be wrong) so the operator is never
// silently dropped. ctx is the "forge run"/"forge evolve" message prefix. Shared by run +
// evolve so the gate and its honest fallback message stay in lockstep.
func parallelEnabled(o runOpts, wf asset.Workflow, logln func(string), ctx string) bool {
	if !o.parallel {
		return false
	}
	if declaresDependsOn(wf) {
		return true
	}
	logln(ctx + ": --parallel ignored (workflow declares no depends_on) — running serially")
	return false
}

// runWorkflow dispatches a single-pass `forge run` to the PARALLEL engine when
// parallelEnabled, else the SERIAL engine (the byte-for-byte default).
func runWorkflow(eng orchestrator.Engine, wf asset.Workflow, o runOpts, logln func(string)) error {
	if parallelEnabled(o, wf, logln, "forge run") {
		return eng.RunParallel(wf, o.mode)
	}
	return eng.Run(wf, o.mode)
}

// cmdRun parses flags, loads + transcodes the workflow, and runs the engine.
func cmdRun(args []string) int {
	fs := flag.NewFlagSet("run", flag.ContinueOnError)
	var o runOpts
	bindRunOpts(fs, &o)
	name, flagArgs := splitPositional(args)
	if name == "" {
		fmt.Fprintln(os.Stderr, "forge run: exactly one <workflow> required")
		usage()
		return 2
	}
	if err := fs.Parse(flagArgs); err != nil {
		return 2
	}
	o.root = gate.RepoRoot(o.root)
	wf, err := loadWorkflow(o.root, name)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
		return 1
	}
	return execEngine(wf, o)
}

// splitPositional takes the leading <workflow> argument (per `forge run
// <workflow> [flags]`) and returns it with the remaining flag args. The name must
// come first and be non-empty/non-dash, keeping parsing unambiguous when a later
// flag takes a path value (e.g. --root /abs/path).
func splitPositional(args []string) (name string, flags []string) {
	if len(args) == 0 || args[0] == "" || args[0][0] == '-' {
		return "", nil
	}
	return args[0], args[1:]
}

// loadWorkflow transcodes .agent/workflows/<name>.yml to JSON via the python shim
// and parses it; a missing shim or workflow yields a clear, actionable error.
func loadWorkflow(repoRoot, name string) (asset.Workflow, error) {
	ymlPath := filepath.Join(repoRoot, ".agent", "workflows", name+".yml")
	if _, err := os.Stat(ymlPath); err != nil {
		return asset.Workflow{}, fmt.Errorf("workflow not found: %s", ymlPath)
	}
	shim := filepath.Join(repoRoot, "harness", "yaml2json.py")
	if _, err := os.Stat(shim); err != nil {
		return asset.Workflow{}, fmt.Errorf(
			"YAML->JSON shim missing: %s (forge-core is zero-dep and reads JSON; "+
				"this transcoder is the temporary YAML bridge — see README)", shim)
	}
	out, err := exec.Command("python3", shim, ymlPath).Output()
	if err != nil {
		return asset.Workflow{}, fmt.Errorf("transcoding %s via %s failed: %w", ymlPath, shim, err)
	}
	return asset.LoadWorkflowJSON(out)
}

// execEngine wires the real harness gates + the selected agent executor and
// runs the workflow, returning 0 on a clean run and 1 on the first failure.
//
// Honesty: acceptance is probed ONCE per run (gate.ProbeAll), and that single
// map backs BOTH the per-gate verdicts (harnessRunner) and convergence
// (gatherSignals) — never double-spawned, never inconsistent within a run. An
// N/A gate does NOT fail the run (it completes, exit 0); only a real FAIL does.
func execEngine(wf asset.Workflow, o runOpts) int {
	logln := func(s string) { fmt.Println(s) }
	probe, categories := probeStatuses(o.root)
	lifecycle := resolveLifecycle(o)
	pol := mode.Effective(o.mode, lifecycle)
	// `forge run` did not open a tracer before, so a REAL claude run burned cost the
	// scorecard never saw. Open one (append, same .forge/trace.jsonl evolve uses; it is
	// git-ignored) and feed agent-phase cost into it via the sink. Fail-closed mirrors
	// evolve's openTracer: a run must not proceed blind on the cost it is about to bill.
	tracer, closeTrace, err := openTracer(o.root)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
		return 1
	}
	defer closeTrace()
	budget, err := newRunBudget(o.runBudgetUSD)
	if err != nil { // a misconfigured budget fails closed: never silently drop a cap.
		fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
		return 1
	}
	eng, verdicts, _ := buildRunEngine(wf, o, logln, costEmitter(tracer, logln), harnessRunner(o.root, probe), pol, budget)
	// Learning-loop wind-down: attribute this run's REAL billed cost into the scorecards
	// REGARDLESS of outcome — DEFERRED so a run that fails or exhausts its --run-budget-usd
	// mid-way (the highest-cost, most-informative cases — a REJECTED build is the most useful
	// quality sample) still attributes what it billed, matching `forge evolve`'s unconditional
	// placement. The previous success-only call dropped exactly those, biasing scorecards toward
	// successes. Registered AFTER `defer closeTrace()` above so it runs BEFORE it (LIFO) — the
	// trace it reads is still open, per scorecard_wind.go's flush-ordering invariant. Still
	// gate-on-real-cost (a dry/echo or nothing-billed failure skips it) + fail-loud-and-continue
	// (a producer hiccup never flips the run's exit code, set before these defers fire).
	// iterations=1: a single `forge run` is one execution; verdicts.wasReworked() carries the
	// real reviewer-bounce signal into avg_iterations / rework_rate.
	defer func() { windDownScorecards(wf, o, logln, 1, verdicts.wasReworked()) }()
	fmt.Printf("forge run: stage=%s mode=%s lifecycle=%s executor=%s gates=%v reviewer=%v discover=%s design=%s adr=%v (%d phases)\n",
		wf.Stage, o.mode, lifecycle, o.executor, pol.Gates, pol.Reviewer,
		pol.DiscoverDepth, pol.DesignDepth, pol.ADR, len(wf.Phases))
	if err := runWorkflow(eng, wf, o, logln); err != nil {
		fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
		return 1
	}
	fmt.Println("forge run: workflow completed")
	reportConvergence(wf, o.root, probe, categories, lifecycle, o.approved)
	return 0
}

// reportConvergence evaluates the workflow's stop condition against live repo
// signals (ROADMAP completion, gate state, and — for a human_gate — the human
// approval signal) and prints the verdict. It is the real convergence check
// (ForgeOS forbids round-count termination), reusing the SAME probe map as the
// gate phases. It dispatches through converge.Converge so a human_gate is judged
// by approval alone, never the conjunction path. A human_gate gets a distinct,
// HONEST report: not approved => "awaiting human approval" (a stop to wait for a
// human, NOT a gate FAIL); approved => "approved -> unlocks <next_stage>".
func reportConvergence(wf asset.Workflow, root string, probe, categories map[string]string, lifecycle string, approvedFlag bool) {
	if wf.Stop.Type == "" {
		return
	}
	approved := humanApproved(root, wf.Stage, approvedFlag)
	results, met := converge.Converge(wf.Stop, gatherSignals(root, wf, probe, categories, lifecycle, approved))
	if converge.IsHumanGate(wf.Stop) {
		reportHumanGate(wf, met)
		return
	}
	fmt.Printf("convergence: %s (%s)\n", verdict(met), wf.Stop.Type)
	for _, r := range results {
		fmt.Printf("  [%s] %s — %s\n", mark(r.Met), r.Expr, r.Detail)
	}
}

// reportHumanGate prints the human-approval gate's honest outcome. Unapproved is
// NOT a failure: the stage is correctly holding for a non-bypassable human
// decision, so it reads "awaiting human approval", distinct from a gate FAIL.
// Approved reads "approved -> unlocks <next_stage>" (the spine stage on_approved
// unlocks). HONESTY: the approval is a v1 signal check (--approved / on-disk
// marker), not a durable cross-process wait (durable_wait is v2, Temporal).
func reportHumanGate(wf asset.Workflow, approved bool) {
	if !approved {
		fmt.Printf("convergence: NOT MET (human_gate) — awaiting human approval (non-bypassable)\n")
		fmt.Println("  pass --approved or create .forge/" + wf.Stage + ".approved to grant approval (v1 signal check; durable wait is v2)")
		return
	}
	fmt.Printf("convergence: MET (human_gate) — approved → unlocks %s\n", nextStageLabel(wf.Stop))
}

// nextStageLabel renders the stage a human_gate approval unlocks, or a clear
// marker when the workflow declares none (so the line is always informative).
func nextStageLabel(stop asset.StopCondition) string {
	if stop.OnApproved.NextStage == "" {
		return "(no next_stage declared)"
	}
	return "next_stage=" + stop.OnApproved.NextStage
}

// verdict/mark render a convergence boolean for the report; pick is the missing
// string ternary (a if cond else b).
func verdict(met bool) string { return pick(met, "MET", "NOT MET") }
func mark(met bool) string    { return pick(met, "x", " ") }
func pick(cond bool, a, b string) string {
	if cond {
		return a
	}
	return b
}

// forgeDir is the per-repo runtime-state directory (<root>/.forge) holding the
// checkpoint, trace, and memory store. It is git-ignored and excluded from the
// harness/arch scans, so these runtime products never pollute the repo's gates.
// The writers MkdirAll it, so a first run needs no pre-existing tree.
func forgeDir(root string) string { return filepath.Join(root, ".forge") }

// memoryPath is the cross-session knowledge store the loop appends to and every
// prompt reads from — the notebook that makes a long run non-amnesiac.
func memoryPath(root string) string { return filepath.Join(forgeDir(root), "memory.jsonl") }

// resolveLifecycle picks the maturity modifier the central knob composes with
// mode. Precedence: an explicit --lifecycle flag wins; otherwise read
// <root>/.agent/project.yml's `lifecycle:`; if neither is present, default to
// "mvp" (the modes.yml selector default). The flag/value is NOT validated here —
// mode.Effective fail-safes any unknown value to the FULL (strictest) policy, so
// a typo over-enforces rather than silently dropping gates.
func resolveLifecycle(o runOpts) string {
	if o.lifecycle != "" {
		return o.lifecycle
	}
	if v := projectYAMLValue(o.root, "lifecycle"); v != "" {
		return v
	}
	return "mvp"
}

// projectYAMLValue reads one top-level scalar `key: value` from
// <root>/.agent/project.yml, stripping a trailing `# comment` and surrounding
// whitespace. This is a deliberately tiny line scanner — forge-core is zero-dep
// (no YAML lib), and project.yml's mode/lifecycle are flat scalars (the same
// approach arch-check.mjs uses for policies.yml). A missing file or absent key
// yields "" (the caller then falls back), never an error: project.yml is an
// optional convenience, not a hard dependency of a run.
func projectYAMLValue(root, key string) string {
	data, err := os.ReadFile(filepath.Join(root, ".agent", "project.yml"))
	if err != nil {
		return ""
	}
	for _, line := range strings.Split(string(data), "\n") {
		rest, ok := strings.CutPrefix(line, key+":")
		if !ok {
			continue
		}
		if i := strings.IndexByte(rest, '#'); i >= 0 {
			rest = rest[:i]
		}
		return strings.TrimSpace(rest)
	}
	return ""
}

// agentExecutor (executor selection) and buildRunEngine (shared Engine assembly) moved
// to engine_build.go to keep this file under the file-size budget.
