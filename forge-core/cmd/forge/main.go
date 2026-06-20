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
	"forgeos/forge-core/internal/memory"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/prompt"
)

// maxLoopBack is the conservative ceiling on DIRECTED gate loop-backs per run
// (orchestrator.Engine.MaxLoopBack): a gate phase declaring on_fail:{loop_back}
// may bounce back to its target phase at most this many times before a still-red
// gate aborts the run (fail-closed). Kept small on purpose — loop-back is a
// recovery path, not a retry-until-green crutch — and it only ever engages for a
// workflow whose gate phases actually carry an on_fail (build.yml's do).
const maxLoopBack = 3

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
  forge run    <workflow> [--mode balanced] [--lifecycle mvp] [--executor dry|command] [--agent-cmd claude] [--agent-permission acceptEdits] [--timeout 0] [--max-retries 0] [--max-agent-depth 2] [--max-agent-calls 0] [--max-output-bytes 0] [--approved] [--root DIR]
  forge evolve <workflow> [--mode balanced] [--lifecycle mvp] [--max-iter 5] [--executor dry|command] [--agent-cmd claude] [--agent-permission acceptEdits] [--timeout 0] [--max-retries 0] [--max-agent-depth 2] [--max-agent-calls 0] [--max-output-bytes 0] [--resume] [--root DIR]
  forge route  [--complexity F] [--risk-score F] [--security F] [--dependency F] [--context F] [--business F] [--task-type T] [--risk low|medium|high|critical] [--budget F] [--scorecard PATH]
  forge migrate --to engineering [--apply] [--root DIR]
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
	fs.DurationVar(&o.timeout, "timeout", 0, "per-agent-command timeout (0 = no deadline, e.g. 90s, 5m)")
	fs.IntVar(&o.maxRetries, "max-retries", 0, "retry ceiling for retryable agent failures (0 = no retries)")
	fs.IntVar(&o.maxAgentDepth, "max-agent-depth", 0, "nested agent-spawn cap for --executor=command (0 = safe default 2; prevents recursive fork-bombs)")
	fs.IntVar(&o.maxAgentCalls, "max-agent-calls", 0, "per-run ceiling on agent-phase executions for --executor=command (0 = unbounded; for evolve this is PER-ITERATION, total <= max-iter x this)")
	fs.IntVar(&o.maxOutputBytes, "max-output-bytes", 0, "cap on retained agent stdout+stderr per command (0 = safe default 10MiB; prevents a runaway-output OOM)")
	fs.BoolVar(&o.approved, "approved", false, "supply the human-approval signal for a human_gate workflow (or create <root>/.forge/<stage>.approved)")
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
	probe := probeStatuses(o.root)
	lifecycle := resolveLifecycle(o)
	pol := mode.Effective(o.mode, lifecycle)
	eng := orchestrator.Engine{
		Exec:          agentExecutor(o, logln),
		RunGate:       harnessRunner(o.root, probe),
		Log:           logln,
		MaxRetries:    o.maxRetries,
		MaxLoopBack:   maxLoopBack,
		MaxAgentCalls: o.maxAgentCalls,
		ModePolicy:    pol,
	}
	fmt.Printf("forge run: stage=%s mode=%s lifecycle=%s executor=%s gates=%v reviewer=%v discover=%s design=%s adr=%v (%d phases)\n",
		wf.Stage, o.mode, lifecycle, o.executor, pol.Gates, pol.Reviewer,
		pol.DiscoverDepth, pol.DesignDepth, pol.ADR, len(wf.Phases))
	if err := eng.Run(wf, o.mode); err != nil {
		fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
		return 1
	}
	fmt.Println("forge run: workflow completed")
	reportConvergence(wf, o.root, probe, o.approved)
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
func reportConvergence(wf asset.Workflow, root string, probe map[string]string, approvedFlag bool) {
	if wf.Stop.Type == "" {
		return
	}
	approved := humanApproved(root, wf.Stage, approvedFlag)
	results, met := converge.Converge(wf.Stop, gatherSignals(root, wf, probe, approved))
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

// agentExecutor selects the agent-phase executor. "command" builds a per-phase
// prompt and drives o.agentCmd with it (real execution when agent-cmd is `claude`;
// `echo` inspects the plumbing safely); anything else is the no-LLM DryRunExecutor.
func agentExecutor(o runOpts, logln func(string)) orchestrator.AgentExecutor {
	if o.executor == "command" {
		return orchestrator.CommandExecutor{
			Build: func(p asset.Phase, mode string) []string {
				argv := []string{o.agentCmd}
				// claude print mode needs flags echo/stubs don't understand, so gate
				// on a claude-family command: --permission-mode to USE tools (write
				// files) headlessly — without it the agent only DESCRIBES edits it
				// can't apply — and --model so a real run honors ForgeOS's ROUTED tier
				// (the opus floor for reviewer/architect/cto + any per-phase model_tier
				// override); without --model, claude ignores routing and uses its default.
				if strings.Contains(o.agentCmd, "claude") {
					if o.agentPermission != "" {
						argv = append(argv, "--permission-mode", o.agentPermission)
					}
					argv = append(argv, "--model", orchestrator.PhaseTier(p, mode))
				}
				return append(argv, "-p", buildPrompt(o.root, p, mode))
			},
			Timeout:        o.timeout,
			MaxDepth:       o.maxAgentDepth,
			MaxOutputBytes: o.maxOutputBytes,
			Log:            logln,
		}
	}
	return orchestrator.DryRunExecutor{Log: logln}
}

// buildPrompt assembles the instruction for an agent phase. Beyond the role
// card, the Context Engine now injects (1) hard constraints + ADRs RETRIEVED
// against this phase's query (Gather), and (2) cross-session memory — the
// gaps/decisions/lessons prior iterations recorded (memoryContext). The query
// "<phase> <agent>" is the natural relevance signal for both lanes.
func buildPrompt(repoRoot string, p asset.Phase, mode string) string {
	tier := orchestrator.PhaseTier(p, mode)
	ctx := prompt.Gather(repoRoot, p.Name+" "+p.Agent)
	ctx = append(ctx, memoryContext(repoRoot)...)
	return prompt.Build(p.Agent, p.Name, mode, tier, readCard(repoRoot, p.Agent), ctx)
}

// memoryContext renders the cross-session store as one context block so the agent
// sees what prior iterations learned. Topic is unconstrained — a phase should see
// every gap/decision/lesson. Missing store = cold start (no block, no error); a
// malformed store is surfaced as a visible context line, not an aborted prompt.
func memoryContext(repoRoot string) []string {
	entries, err := memory.Load(memoryPath(repoRoot))
	if err != nil {
		return []string{"Project memory: UNREADABLE (" + err.Error() + ")"}
	}
	rel := memory.Query(entries, "", "")
	if len(rel) == 0 {
		return nil
	}
	var b strings.Builder
	b.WriteString("Project memory (gaps / decisions / lessons from prior iterations):")
	for _, e := range rel {
		fmt.Fprintf(&b, "\n- [%s] %s — %s (iter %d)", e.Kind, e.Topic, e.Detail, e.Iteration)
	}
	return []string{b.String()}
}

// readCard returns the agent's role-card text, or a short marker when absent so
// the prompt is still well-formed.
func readCard(repoRoot, agent string) string {
	b, err := os.ReadFile(filepath.Join(repoRoot, ".agent", "agents", agent+".md"))
	if err != nil {
		return fmt.Sprintf("(no role card found for %q)", agent)
	}
	return string(b)
}
