// Command forge is the forge-core CLI: a zero-dependency (Go stdlib only)
// driver for ForgeOS workflows and the real harness gates.
//
// Subcommands:
//
//	forge run <workflow> [--mode balanced] [--executor dry|command] [--agent-cmd claude]
//	    Load .agent/workflows/<workflow>.yml, transcode it to JSON via
//	    `python3 harness/yaml2json.py`, then run it through the orchestrator
//	    with the real harness gates as the gate runner. Agent phases use the
//	    --executor: "dry" narrates the routing decision (no LLM); "command"
//	    builds a per-phase prompt from the agent's role card and drives
//	    --agent-cmd with it — `claude -p <prompt>` for real execution, or
//	    `echo` to inspect the plumbing without firing an agent.
//
//	forge gate | check | accept
//	    Delegate directly to the corresponding harness gate and exit with that
//	    gate's status (0 == OK).
//
// The repo root is taken from --root, else $FORGE_REPO_ROOT, else the current
// directory. The per-gate resolution + acceptance probing live in gates.go.
package main

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/prompt"
	"forgeos/forge-core/internal/routing"
)

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
  forge run    <workflow> [--mode balanced] [--executor dry|command] [--agent-cmd claude] [--root DIR]
  forge evolve <workflow> [--max-iter 5] [--executor dry|command] [--agent-cmd claude] [--root DIR]
  forge route  [--complexity F] [--risk-score F] [--security F] [--dependency F] [--context F] [--business F] [--task-type T] [--risk low|medium|high|critical] [--budget F]
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

// runOpts holds the parsed `forge run` flags.
type runOpts struct {
	mode     string
	root     string
	executor string
	agentCmd string
}

// cmdRun parses flags, loads + transcodes the workflow, and runs the engine.
func cmdRun(args []string) int {
	fs := flag.NewFlagSet("run", flag.ContinueOnError)
	var o runOpts
	fs.StringVar(&o.mode, "mode", "balanced", "engineering mode (explorer|balanced|engineering|cto)")
	fs.StringVar(&o.root, "root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	fs.StringVar(&o.executor, "executor", "dry", "agent executor: dry|command")
	fs.StringVar(&o.agentCmd, "agent-cmd", "claude", "command for --executor=command (e.g. claude, echo)")
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

// splitPositional takes the leading <workflow> argument (per the documented
// `forge run <workflow> [flags]` form) and returns it with the remaining flag
// args. The name must come first and be non-empty/non-dash; this keeps parsing
// unambiguous when a later flag takes a path value (e.g. --root /abs/path).
func splitPositional(args []string) (name string, flags []string) {
	if len(args) == 0 || args[0] == "" || args[0][0] == '-' {
		return "", nil
	}
	return args[0], args[1:]
}

// loadWorkflow transcodes .agent/workflows/<name>.yml to JSON via the python
// shim and parses it. A missing shim or workflow yields a clear, actionable
// error rather than an obscure failure.
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
	eng := orchestrator.Engine{
		Exec:    agentExecutor(o, logln),
		RunGate: harnessRunner(o.root, probe),
		Log:     logln,
	}
	fmt.Printf("forge run: stage=%s mode=%s executor=%s (%d phases)\n",
		wf.Stage, o.mode, o.executor, len(wf.Phases))
	if err := eng.Run(wf, o.mode); err != nil {
		fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
		return 1
	}
	fmt.Println("forge run: workflow completed")
	reportConvergence(wf, o.root, probe)
	return 0
}

// reportConvergence evaluates the workflow's stop condition against live repo
// signals (ROADMAP completion, gate state) and prints a per-criterion verdict.
// ForgeOS forbids round-count termination — this is the real convergence check.
// It reuses the SAME probe map as the gate phases (no second acceptance run).
func reportConvergence(wf asset.Workflow, root string, probe map[string]string) {
	if wf.Stop.Type == "" {
		return
	}
	results, met := converge.Evaluate(wf.Stop.AllOf, gatherSignals(root, wf, probe))
	fmt.Printf("convergence: %s (%s)\n", verdict(met), wf.Stop.Type)
	for _, r := range results {
		fmt.Printf("  [%s] %s — %s\n", mark(r.Met), r.Expr, r.Detail)
	}
}

func verdict(met bool) string {
	if met {
		return "MET"
	}
	return "NOT MET"
}

func mark(met bool) string {
	if met {
		return "x"
	}
	return " "
}

// cmdEvolve loops a workflow until it converges, a tripwire fires, or the
// safety bound — the autonomous-loop entry point (real agents via --executor).
func cmdEvolve(args []string) int {
	fs := flag.NewFlagSet("evolve", flag.ContinueOnError)
	var o runOpts
	fs.StringVar(&o.mode, "mode", "balanced", "engineering mode")
	fs.StringVar(&o.root, "root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	fs.StringVar(&o.executor, "executor", "dry", "agent executor: dry|command")
	fs.StringVar(&o.agentCmd, "agent-cmd", "claude", "command for --executor=command")
	maxIter := fs.Int("max-iter", 5, "safety bound on loop iterations (not the goal)")
	name, flagArgs := splitPositional(args)
	if name == "" {
		fmt.Fprintln(os.Stderr, "forge evolve: exactly one <workflow> required")
		usage()
		return 2
	}
	if err := fs.Parse(flagArgs); err != nil {
		return 2
	}
	o.root = gate.RepoRoot(o.root)
	wf, err := loadWorkflow(o.root, name)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge evolve: %v\n", err)
		return 1
	}
	return execLoop(wf, o, *maxIter)
}

// execLoop wires the loop engine (real gates + selected executor + live signals)
// and runs it to convergence, a tripwire, or the safety bound, reporting how it
// ended. For an external-stop workflow (e.g. evolve), reaching the safety bound
// is the EXPECTED clean outcome and the CLI exits 0 — it is never degraded to a
// round-count failure.
func execLoop(wf asset.Workflow, o runOpts, maxIter int) int {
	logln := func(s string) { fmt.Println(s) }
	// One probe per iteration, shared by that iteration's gate phases and its
	// convergence check. The loop runs gates (refresh) before Signals (reuse),
	// so refreshing on each gate-runner call keeps both honest and consistent
	// without double-spawning acceptance within an iteration.
	probe := &loopProbe{root: o.root}
	eng := orchestrator.Engine{
		Exec:    agentExecutor(o, logln),
		RunGate: func(name string) gate.Result { return resolveGate(o.root, name, probe.refresh()) },
		Log:     logln,
	}
	loop := orchestrator.NewLoopEngine(
		eng, wf.Stop.Type, wf.Stop.AllOf,
		func() converge.Signals { return gatherSignals(o.root, wf, probe.current()) },
		maxIter, 2, logln)
	fmt.Printf("forge evolve: stage=%s mode=%s max-iter=%d type=%s (doom-loop tripwire=2)\n",
		wf.Stage, o.mode, maxIter, stopTypeLabel(wf.Stop.Type))
	out, err := loop.Run(wf, o.mode)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge evolve: %v\n", err)
		return 1
	}
	fmt.Printf("forge evolve: ended after %d iter — converged=%v (%s)\n", out.Iterations, out.Converged, out.Reason)
	if out.Converged {
		return 0
	}
	return 1
}

// stopTypeLabel renders the stop type for the run banner, defaulting to a
// readable placeholder when a workflow declares none.
func stopTypeLabel(t string) string {
	if t == "" {
		return "(none)"
	}
	return t
}

// agentExecutor selects the agent-phase executor. "command" builds a per-phase
// prompt from the agent's role card and drives o.agentCmd with it (real
// execution when agent-cmd is `claude`; `echo` inspects the plumbing safely).
// Anything else is the no-LLM DryRunExecutor.
func agentExecutor(o runOpts, logln func(string)) orchestrator.AgentExecutor {
	if o.executor == "command" {
		return orchestrator.CommandExecutor{
			Build: func(p asset.Phase, mode string) []string {
				return []string{o.agentCmd, "-p", buildPrompt(o.root, p, mode)}
			},
			Log: logln,
		}
	}
	return orchestrator.DryRunExecutor{Log: logln}
}

// buildPrompt assembles the instruction for an agent phase from its role card,
// the phase, the mode, and the routed tier — the message a real agent CLI runs.
func buildPrompt(repoRoot string, p asset.Phase, mode string) string {
	tier := routing.TierFor(p.Agent, mode)
	return prompt.Build(p.Agent, p.Name, mode, tier, readCard(repoRoot, p.Agent), prompt.Gather(repoRoot))
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
