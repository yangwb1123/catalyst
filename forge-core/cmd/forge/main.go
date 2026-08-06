// Command forge is the forge-core CLI (Go stdlib only). It loads
// .agent/workflows/<workflow>.yml with the native parser (Python shim fallback)
// and orchestrates agent phases through the real harness gates.
package main

import (
	"context"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/orchestrator"
)

// forgeVersion is the semantic version of forge-core, injected at build time
// via `go build -ldflags "-X main.forgeVersion=v2.5.0"`. When empty (plain
// `go build` without -ldflags), it reports "dev" to indicate a local build.
var forgeVersion = "dev"

// forgeCommit is the git SHA of the build, injected at build time via
// `go build -ldflags "-X main.forgeCommit=$(git rev-parse --short HEAD)"`.
// When empty, the SHA is omitted from version output.
var forgeCommit = ""

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
	// evolveProposalOnly is resolved policy state, never a CLI flag.
	evolveProposalOnly bool
	// workflowStage binds privileged roles to the loaded workflow identity.
	workflowStage string
	// lifecycle freezes the project maturity used to resolve workflow policy.
	// Empty means read .agent/project.yml, then fall back to mvp.
	lifecycle string
	// Explicitness keeps CLI defaults distinguishable from deliberate default
	// values when validating a persisted chain's immutable run envelope.
	runFlagsCaptured       bool
	modeExplicit           bool
	lifecycleExplicit      bool
	maxAgentCallsExplicit  bool
	maxChainStagesExplicit bool
	runBudgetExplicit      bool
	root                   string
	executor               string
	agentCmd               string
	// sandbox isolates agent commands: "" (host) | "docker" | "firecracker".
	sandbox       string
	sandboxImage  string
	sandboxKernel string
	// Restricted release/proposal stages require a pinned executable and digest.
	releaseAgentPath   string
	releaseAgentSHA256 string
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
	// agentEnv is a comma-separated exact-name allow-list for extra child env.
	agentEnv string
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
	// chain auto-advances through the spine: when a workflow converges MET and
	// declares on_met.next_stage, load and run the next workflow automatically.
	// Stops at human_gate (waits for approval) or when no next_stage is declared.
	chain bool
	// maxChainStages is a hard safety bound for --chain. It complements cycle
	// detection so even a long acyclic but misconfigured next_stage graph cannot
	// run without an operator-defined ceiling.
	maxChainStages int
}

// bindRunOpts registers the flags shared by `forge run` and `forge evolve` onto
// fs, writing into o — one definition so both subcommands stay in lockstep.
func bindRunOpts(fs *flag.FlagSet, o *runOpts) {
	fs.StringVar(&o.mode, "mode", "", "engineering mode (explorer|balanced|engineering|cto); empty = read .agent/project.yml, else balanced")
	fs.StringVar(&o.lifecycle, "lifecycle", "", "maturity modifier (idea|mvp|growth|production); empty = read .agent/project.yml, else mvp")
	fs.StringVar(&o.root, "root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	fs.StringVar(&o.executor, "executor", "dry", "agent executor: dry|command")
	fs.StringVar(&o.sandbox, "sandbox", "", "isolate agent commands: docker|firecracker (requires --sandbox-image; firecracker also requires --sandbox-kernel)")
	fs.StringVar(&o.sandboxImage, "sandbox-image", "", "docker image or firecracker rootdir template for --sandbox")
	fs.StringVar(&o.sandboxKernel, "sandbox-kernel", "", "firecracker vmlinux.bin path for --sandbox=firecracker")
	fs.StringVar(&o.agentCmd, "agent-cmd", "claude", "command for --executor=command (e.g. claude, echo)")
	fs.StringVar(&o.releaseAgentPath, "release-agent-path", "", "absolute trusted Claude executable for command-mode release or proposal-only Evolve (required with --release-agent-sha256)")
	fs.StringVar(&o.releaseAgentSHA256, "release-agent-sha256", "", "operator-pinned SHA-256 of --release-agent-path for restricted command execution")
	fs.StringVar(&o.agentPermission, "agent-permission", "acceptEdits", "claude --permission-mode for --executor=command (acceptEdits|plan|default); lets the agent write code headlessly")
	fs.StringVar(&o.agentAllowedTools, "agent-allowed-tools", defaultAgentAllowedTools, "claude --allowedTools whitelist (space/comma-separated) so a print-mode agent can SELF-VERIFY the code it wrote (run tests/gate) and honestly tick a ROADMAP [x]; default is the node test+gate self-check. READ-ONLY validators only — NEVER add forge or any agent-spawning command (it bypasses the recursion guard). Override for non-node projects (e.g. pytest/vitest); empty disables")
	fs.StringVar(&o.agentMaxBudgetUSD, "agent-max-budget-usd", "", "per-claude-call dollar ceiling (claude --max-budget-usd; empty = unset); the per-phase cost bound complementing --max-agent-calls/--timeout")
	fs.StringVar(&o.agentEnv, "agent-env", "", "comma-separated exact parent env names to grant to the agent; cloud/Git/SSH credentials are denied by default")
	fs.StringVar(&o.runBudgetUSD, "run-budget-usd", "", "cumulative run-level dollar cap across ALL phases/iterations (empty = unset); STOPS the run before overspend — distinct from the per-call --agent-max-budget-usd")
	fs.DurationVar(&o.timeout, "timeout", 0, "per-agent-command timeout (0 = no deadline, e.g. 90s, 5m)")
	fs.IntVar(&o.maxRetries, "max-retries", 0, "retry ceiling for retryable agent failures (0 = no retries)")
	fs.IntVar(&o.maxAgentDepth, "max-agent-depth", 0, "nested agent-spawn cap for --executor=command (0 = safe default 2; prevents recursive fork-bombs)")
	fs.IntVar(&o.maxAgentCalls, "max-agent-calls", 0, "per-run ceiling on agent-phase executions for --executor=command (0 = unbounded; for evolve this is PER-ITERATION, total <= max-iter x this)")
	fs.IntVar(&o.maxOutputBytes, "max-output-bytes", 0, "cap on retained agent stdout+stderr per command (0 = safe default 10MiB; prevents a runaway-output OOM)")
	fs.BoolVar(&o.approved, "approved", false, "supply the human-approval signal for a human_gate workflow (or create <root>/.forge/<stage>.approved)")
	fs.BoolVar(&o.parallel, "parallel", false, "run a workflow's depends_on-independent phases CONCURRENTLY (dependency waves); takes effect ONLY for a workflow that declares depends_on (else stays serial). No directed loop-back in parallel mode.")
	fs.BoolVar(&o.chain, "chain", false, "auto-advance to the next spine stage (next_stage) when the current workflow converges MET")
	fs.IntVar(&o.maxChainStages, "max-chain-stages", defaultMaxChainStages, "hard safety bound for --chain stage transitions")
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
// parallelEnabled, else the SERIAL engine (the byte-for-byte default). ctx
// carries cancellation so the engine can abort cleanly on SIGINT.
//
// startPhase is the on_rejected loop-back start index resolved by
// resolveRejectionStartPhase (gates.go); 0 (no rejection filed) runs eng.Run
// unchanged, else eng.RunFrom, mirroring LoopEngine.runIteration's own
// RunFrom-vs-top choice. RunParallel has no linear resume point (waves, not a
// line) — same limitation LoopEngine's Parallel mode accepts — so a non-zero
// startPhase there is honestly logged and ignored, not silently dropped.
func runWorkflow(ctx context.Context, eng orchestrator.Engine, wf asset.Workflow, o runOpts, logln func(string), startPhase int) error {
	eng.Ctx = ctx
	if parallelEnabled(o, wf, logln, "forge run") {
		if startPhase != 0 {
			logln(fmt.Sprintf("forge run: on_rejected loop-back target (phase %d) is not supported under --parallel (no linear resume point) — running the full dependency graph instead", startPhase))
		}
		return eng.RunParallel(ctx, wf, o.mode)
	}
	if startPhase != 0 {
		return eng.RunFrom(wf, o.mode, startPhase)
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
	if o.chain && o.maxChainStages < 1 {
		fmt.Fprintln(os.Stderr, "forge run: --max-chain-stages must be >= 1")
		return 2
	}
	o.root = gate.RepoRoot(o.root)
	if code := rejectPendingPromotionAtEntry("forge run", o.root); code != 0 {
		return code
	}
	freezeRunOptions(fs, &o)
	wf, err := loadWorkflowForRunEntry(o.root, name, o)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
		return 1
	}
	// Signal-aware context: SIGINT/SIGTERM cancel the context, triggering
	// graceful shutdown of the engine and its subprocesses.
	ctx, stop := withSignalCancellation()
	defer stop()
	return execEngine(ctx, wf, o)
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

// reportConvergence evaluates the workflow's stop condition against live repo
// signals (ROADMAP completion, gate state, and — for a human_gate — the human
// approval signal) and prints the verdict. It is the real convergence check
// (ForgeOS forbids round-count termination), reusing the SAME probe map as the
// gate phases. It dispatches through converge.Converge so a human_gate is judged
// by approval alone, never the conjunction path. A human_gate gets a distinct,
// HONEST report: not approved => "awaiting human approval" (a stop to wait for a
// human, NOT a gate FAIL); approved => "approved -> unlocks <next_stage>".
//
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

// resolveMode mirrors resolveLifecycle for the other half of the central
// project selector. An explicit flag wins; otherwise the persistent mode in
// project.yml is consumed; a project without that optional setting keeps the
// historical balanced default. Values stay unvalidated here so mode.Effective
// can apply its strict fail-safe to an unknown value instead of silently
// downgrading it to balanced.
func resolveMode(o runOpts) string {
	if o.mode != "" {
		return o.mode
	}
	if v := projectYAMLValue(o.root, "mode"); v != "" {
		return v
	}
	return "balanced"
}

func freezeRunOptions(fs *flag.FlagSet, o *runOpts) {
	o.runFlagsCaptured = true
	o.modeExplicit = flagSet(fs, "mode")
	o.lifecycleExplicit = flagSet(fs, "lifecycle")
	o.maxAgentCallsExplicit = flagSet(fs, "max-agent-calls")
	o.maxChainStagesExplicit = flagSet(fs, "max-chain-stages")
	o.runBudgetExplicit = flagSet(fs, "run-budget-usd")
	o.mode = resolveMode(*o)
	o.lifecycle = resolveLifecycle(*o)
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
		raw := strings.TrimSpace(rest)
		if len(raw) >= 2 && raw[0] == '"' && raw[len(raw)-1] == '"' {
			if value, err := strconv.Unquote(raw); err == nil {
				return value
			}
		}
		if len(raw) >= 2 && raw[0] == '\'' && raw[len(raw)-1] == '\'' {
			return strings.ReplaceAll(raw[1:len(raw)-1], "''", "'")
		}
		return raw
	}
	return ""
}

// sandboxConfig maps the CLI sandbox flags onto the executor's isolation
// config. Empty --sandbox selects host execution; the auto-wiring in
// orchestrator handles docker/firecracker field completion and fails closed
// on unknown or incomplete configurations.
func sandboxConfig(o runOpts) *orchestrator.SandboxConfig {
	if strings.TrimSpace(o.sandbox) == "" {
		return nil
	}
	return &orchestrator.SandboxConfig{
		Type:   o.sandbox,
		Image:  o.sandboxImage,
		Kernel: o.sandboxKernel,
	}
}
