package orchestrator

import (
	"context"
	"fmt"
	"os"
	"strconv"
	"strings"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/execbound"
)

// defaultMaxAgentDepth bounds nested agent spawns before the recursion guard
// fires (see CommandExecutor.MaxDepth). 2 permits one legitimate level of
// sub-agent orchestration (a top-level run whose agent drives one nested run);
// the 3rd spawn is refused.
const defaultMaxAgentDepth = 2

// agentDepthEnv names the inherited counter tracking agent-call nesting across
// process boundaries; each spawn sets it to parent+1 on the child's environment.
const agentDepthEnv = "FORGE_AGENT_DEPTH"

// CommandExecutor runs an external command per agent phase — the real bridge
// beyond DryRunExecutor. Build produces the argv for a phase under a mode
// (e.g. ["claude", "-p", prompt]); pointing it at a real agent CLI is how
// forge-core graduates from narrating to actually driving agents. It is
// verified with stub commands here; driving a real LLM agent additionally
// needs that CLI plus credentials in the environment.
type CommandExecutor struct {
	// Build returns the argv to run for a phase. An empty result is an error.
	Build func(p asset.Phase, mode string) []string
	// ValidateConfig is an OPTIONAL phase-aware authorization check run before
	// Build constructs an argv. It lets a caller enforce workflow/agent security
	// boundaries without teaching this generic process runner about a particular
	// agent vendor. Any error is a permanent KindConfig failure and no command is
	// constructed or spawned.
	ValidateConfig func(p asset.Phase, mode string) error
	// PromptViaStdin moves the final prompt argument of a "-p <prompt>" command
	// to stdin immediately before process creation. Build remains inspectable and
	// backward-compatible, while sensitive repository context never reaches ps.
	PromptViaStdin bool
	// Dir is the working directory for the spawned agent (the project --root). A
	// real agent resolves the task's relative paths and writes files relative to its
	// cwd; without this it inherits forge's OWN cwd, so `forge run --root /project`
	// launched from elsewhere would have the agent write to the wrong place. Empty
	// = inherit forge's cwd (os/exec default; the test default, byte-for-byte).
	Dir string
	// Timeout bounds a single command's wall-clock runtime. A zero value means
	// no deadline (the backward-compatible default): an agent that hangs would
	// hang the orchestrator. Set it so a wedged agent is killed and surfaces as
	// a retryable Timeout instead of stalling the whole spine.
	Timeout time.Duration
	// MaxDepth caps nested agent spawns (the recursion guard). A real agent can
	// itself invoke `forge run --executor=command`, spawning another agent that
	// invokes forge again — an unbounded fork-bomb that burns budget with no
	// ceiling. Each spawn inherits an incremented FORGE_AGENT_DEPTH; once it
	// reaches the cap the next spawn is refused with a non-retryable
	// KindRecursionLimit. Zero selects the safe default (defaultMaxAgentDepth).
	// This guard is the prerequisite that makes driving a real agent CLI safe.
	MaxDepth int
	// MaxOutputBytes caps the stdout+stderr RETAINED from one command. A real agent
	// that runs away (a bug or a loop emitting unbounded output) would OOM the
	// orchestrator under an unbounded CombinedOutput; the executor instead retains
	// at most this many bytes, drains the rest, and reports truncation honestly.
	// Zero selects the safe default (execbound.DefaultMaxOutputBytes, 10 MiB). The resource
	// guard's third dimension alongside MaxDepth (depth) and Timeout (wall-clock).
	MaxOutputBytes int
	// EnvAllow names additional parent environment variables the child may
	// inherit. The default environment is intentionally minimal (process basics,
	// locale, and certificate paths); only the trusted FORGE_AGENT_DEPTH counter
	// is injected. Cloud, source-control, and SSH credentials are not inherited
	// unless explicitly named here.
	EnvAllow []string
	// RestrictedEnv removes ambient discovery paths (HOME, SHELL, TMPDIR, XDG,
	// and the parent PATH) for security-sensitive agent phases. It preserves
	// only locale/certificate settings plus EnvAllow, and injects a fixed host
	// PATH. This is intentionally stronger than the ordinary minimal policy.
	RestrictedEnv bool
	Log           func(string)
	// Observe, when set, receives a finished command's phase name, RAW captured output
	// (post-truncation, pre-render), and the command's measured wall-clock LATENCY (the
	// time cmd.Run() took — see Now). It is a generic output SINK: this spawner hands the
	// bytes (and the duration) back to the caller and does NOT interpret them — the caller
	// may parse an executor-specific structure out of the output (e.g. a claude `-p
	// --output-format json` envelope carrying total_cost_usd) that this layer has no
	// knowledge of. The latency is a generic, OS-level wall-clock span (not vendor- or
	// claude-specific): every command has one, exactly as every command has output, so it
	// rides the SAME sink rather than a parallel callback — the caller attributes it to a
	// model the same way it attributes the parsed cost. nil = not observed (the test/default
	// path, byte-for-byte unchanged).
	Observe func(phase, output string, latency time.Duration)
	// ValidateOutput is an OPTIONAL machine-contract check applied after a command
	// exits successfully. It receives the same human-readable output RenderLog
	// would print. A validation error turns the phase into a terminal KindFailed
	// result, preventing malformed planner/reviewer contracts from silently
	// flowing downstream. Nil preserves the legacy no-validation path.
	ValidateOutput func(phase, output string) error
	// ValidateRawOutput is the pre-render twin of ValidateOutput. It receives the
	// complete retained command envelope before RenderLog unwraps it, allowing a
	// caller to enforce transport metadata (for example type/result/is_error)
	// without changing the generic executor's vendor-neutral behavior.
	ValidateRawOutput func(phase, output string) error
	// Now supplies the current time for the wall-clock LATENCY measurement bracketing
	// cmd.Run() — the deterministic-test twin of Engine.Sleep / trace.Now. nil selects the
	// production default (time.Now), so a real run measures the true agent-phase duration; a
	// test injects a fake clock to get an exact, sleep-free latency. Read only to bracket the
	// run, so a nil Now leaves every pre-existing path byte-for-byte unchanged.
	Now func() time.Time
	// RenderLog, when set, transforms a command's captured output before it is written
	// to the Log line — letting the caller present a tidy view (e.g. unwrap the claude
	// JSON's `result` field) WITHOUT this generic layer learning that format. nil =
	// identity (the raw output is logged verbatim, the original byte-for-byte behavior),
	// so a plain echo/printenv/true output is logged exactly as before.
	RenderLog func(output string) string
	// ClassifyOverload, when set, inspects a FAILED command's RAW output and reports whether
	// the failure was a TRANSIENT backend overload. It is a generic JUDGEMENT sink, the exact
	// mirror of Observe: this spawner hands the bytes to the caller and does NOT interpret them
	// — it does not know the output is claude, nor that an overload means an HTTP 529; the caller
	// injects that vendor-specific recognition and returns a plain bool. A true verdict routes
	// the failure to a retryable KindOverloaded (retried after a backoff) instead of a terminal
	// KindFailed; timeout still wins over it (see classifyRunErr). nil = never overloaded =
	// the original byte-for-byte behavior (every non-timeout failure stays KindFailed). Consulted
	// ONLY on a failing run, so a clean command never pays for it.
	ClassifyOverload func(output string) bool

	// Sandbox is the OPTIONAL isolation configuration for running agent commands
	// inside a sandboxed environment (Firecracker microVM, Docker, etc.) instead
	// of directly on the host. nil, an empty Type, or Type "none" explicitly
	// selects host execution. Any requested runtime fails closed until a sandbox
	// runner is installed and wired; it is never silently ignored.
	Sandbox *SandboxConfig
}

// Execute builds and runs the phase's command under an optional timeout, failing
// closed with a typed *ExecError on every error path so a broken executor never
// masquerades as success — and never panics on a nil Build. The error's Kind
// lets callers tell a retryable timeout from a permanent config fault from the
// agent's own non-zero exit (see ExecError). ctx propagates cancellation so a
// parent SIGINT/SIGTERM stops the child process via its process group.
//
// The bounded-run mechanics — deadline, process-group teardown, capped output —
// live in internal/execbound (the shared leaf extraction); this executor maps
// its documented Timeout semantics onto execbound.Options via
// execboundOptions() and funnels every result through finish.
func (c CommandExecutor) Execute(ctx context.Context, p asset.Phase, mode string) error {
	if c.Build == nil {
		return configErr(p.Name, nil) // nil Build: nothing to run, permanent.
	}
	if c.ValidateConfig != nil {
		if err := c.ValidateConfig(p, mode); err != nil {
			return configErr(p.Name, err)
		}
	}
	var sandboxErr error
	c, sandboxErr = c.withSandboxRunner(p.Name)
	if sandboxErr != nil {
		return sandboxErr
	}
	if err := c.environmentConfigError(p.Name); err != nil {
		return err
	}
	argv := c.Build(p, mode)
	if len(argv) == 0 {
		return configErr(p.Name, nil) // empty argv: misconfigured, permanent.
	}
	argv, input, useStdin, err := c.prepareInput(p.Name, argv)
	if err != nil {
		return err
	}
	if c.Sandbox != nil && c.Sandbox.Runner != nil && !c.sandboxNone() {
		return c.executeSandboxedDispatch(ctx, p.Name, argv, input, useStdin)
	}

	// Recursion guard: once the inherited agent-call depth reaches the cap, refuse
	// to spawn — a real agent re-invoking `forge --executor=command` would else
	// fork-bomb unboundedly. Fail closed, NON-retryable (re-running recurses).
	depth := currentAgentDepth()
	if max := c.maxDepth(); depth >= max {
		c.logf("phase %s: recursion guard fired (depth %d >= cap %d) — refusing another agent spawn", p.Name, depth, max)
		return recursionErr(p.Name, depth, max)
	}

	res, latency := c.runMeasured(ctx, argv, depth, input, useStdin)
	return c.finish(p.Name, argv, res, latency)
}

func (c CommandExecutor) prepareInput(phase string, argv []string) ([]string, string, bool, error) {
	if !c.PromptViaStdin {
		return argv, "", false, nil
	}
	if len(argv) < 2 || (argv[len(argv)-2] != "-p" && argv[len(argv)-2] != "--print") {
		return nil, "", false, configErr(phase, fmt.Errorf("stdin prompt requires a terminal -p <prompt> command shape"))
	}
	runArgv := append([]string(nil), argv[:len(argv)-1]...)
	return runArgv, argv[len(argv)-1], true, nil
}

// execboundOptions maps this executor's documented Timeout semantics onto the
// shared bounded-run options. The load-bearing mapping: Timeout == 0 means "no
// deadline" (command_executor.go's documented back-compat default), which
// execbound expresses as the EXPLICIT Unbounded bool — never a negative
// Timeout, which execbound's Validate rejects (a sign error would silently
// reintroduce the very hang this guard removes). MaxOutputBytes maps as-is;
// 0 → execbound's safe default (10 MiB), the same effective cap as before.
func (c CommandExecutor) execboundOptions() execbound.Options {
	opts := execbound.Options{MaxOutputBytes: c.MaxOutputBytes, Log: c.Log}
	if c.Timeout <= 0 {
		opts.Unbounded = true // zero = no deadline (documented back-compat)
	} else {
		opts.Timeout = c.Timeout
	}
	return opts
}

// runMeasured constructs the bounded, process-grouped command for argv and runs it under
// ctx, bracketing cmd.Run() with the injectable clock to MEASURE its wall-clock latency. It
// returns the bounded run result and the measured duration. Split out of Execute
// so that stays within the function-length ceiling; the construction and the timing are one
// unit (the latency must bracket exactly this run), so they live together here.
//
// The latency is the real per-phase duration the cost path stamps onto the agent trace event
// so the scorecard's p95 is genuinely per-model (not the iteration-shared span). The clock read
// is OS-level and generic (no claude/vendor knowledge); this layer only knows WHEN the command
// started and finished, never what model it billed.
func (c CommandExecutor) runMeasured(ctx context.Context, argv []string, depth int, input string, useStdin bool) (execbound.Result, time.Duration) {
	// execbound.Run owns the bounded-run mechanics: a deadline derived from ctx
	// (Timeout > 0) or the Unbounded escape (Timeout == 0), process-group
	// teardown on unix (Setpgid + SIGKILL(-pgid) + WaitDelay backstop) so a
	// tripped deadline is reliably enforced even when `claude -p` forks
	// grandchildren via its own tools (Bash -> git/test/build) that inherit the
	// command's stdout/stderr pipe, and capped output retention with the honest
	// truncation marker. The SAME pointer for Stdout+Stderr (CaptureCombined)
	// lets os/exec serialize the writes, exactly as CombinedOutput merges the
	// two streams.
	spec := execbound.Spec{Dir: c.Dir, Env: c.childEnv(depth)} // empty Dir -> inherit forge's cwd
	if useStdin {
		spec.Stdin = strings.NewReader(input)
	}
	// Bracket the run with the injectable clock — the wall-clock span is the phase's latency.
	start := c.now()
	res := execbound.Run(ctx, argv, c.execboundOptions(), execbound.CaptureCombined, spec)
	return res, c.now().Sub(start)
}

// finish handles a completed command: it hands the raw output AND measured latency to the
// optional sink, logs a possibly-rendered view, and (on failure) classifies the error —
// consulting the optional overload judge. Split out of Execute so each stays within the
// function-length ceiling; the behavior is unchanged except for the new latency the sink
// receives. res carries the retained output, the run error, and the ctx error captured at
// the end of the run (the deadline cause); latency is the wall-clock span bracketing the run
// (see Now).
func (c CommandExecutor) finish(phase string, argv []string, res execbound.Result, latency time.Duration) error {
	// Both observe and the log renderer are nil-safe and identity-by-default, so the no-hook
	// path is byte-for-byte unchanged.
	observed := res.Observed()
	rendered := res.Rendered()
	c.observe(phase, observed, latency)
	visible := c.renderForLog(rendered)
	c.logf("phase %s: ran %q -> %s", phase, strings.Join(argv, " "), visible)
	if res.Err == nil {
		if c.ValidateRawOutput != nil {
			if err := c.ValidateRawOutput(phase, rendered); err != nil {
				return &ExecError{Phase: phase, Kind: KindFailed, Err: fmt.Errorf("raw output contract: %w", err)}
			}
		}
		if c.ValidateOutput != nil {
			if err := c.ValidateOutput(phase, visible); err != nil {
				return &ExecError{Phase: phase, Kind: KindFailed, Err: fmt.Errorf("output contract: %w", err)}
			}
		}
		return nil
	}
	// Ask the optional caller-injected judge whether this failure was a transient overload
	// (e.g. a vendor 529). nil-safe: with no hook the verdict is false, so classifyRunErr keeps
	// its original KindFailed branch — byte-for-byte unchanged.
	isOverload := c.ClassifyOverload != nil && c.ClassifyOverload(rendered)
	return classifyRunErr(phase, res.Err, res.CtxErr, isOverload)
}

// RestoreValidatedOutput revalidates a durable provider-neutral phase result and
// feeds it back through Observe without spawning an Agent. Evolve resume uses
// this to rebuild an in-memory feed-forward ledger while preserving the
// phase-granular guarantee that completed mutable/billed phases are not replayed.
func (c CommandExecutor) RestoreValidatedOutput(p asset.Phase, output string) error {
	if c.ValidateOutput == nil {
		return fmt.Errorf("phase %s: output validator is unavailable", p.Name)
	}
	if err := c.ValidateOutput(p.Name, output); err != nil {
		return fmt.Errorf("phase %s: restored output contract: %w", p.Name, err)
	}
	c.observe(p.Name, output, 0)
	return nil
}

// commandContext returns a context derived from parent, governing one command run,
// plus its cancel. With a positive Timeout it carries a deadline derived from parent;
// with the zero default it is merely cancelable (no deadline) to preserve the original
// unbounded behavior. Using parent instead of context.Background() lets a parent
// cancellation (SIGINT) propagate to the running command.
func (c CommandExecutor) commandContext(parent context.Context) (context.Context, context.CancelFunc) {
	if c.Timeout > 0 {
		return context.WithTimeout(parent, c.Timeout)
	}
	return context.WithCancel(parent)
}

func (c CommandExecutor) logf(format string, args ...any) {
	if c.Log != nil {
		c.Log(fmt.Sprintf(format, args...))
	}
}

// observe forwards a finished command's raw output AND measured wall-clock latency to the
// optional sink. Nil-safe: with no Observe set it is a no-op, so the default/test path is
// byte-for-byte the original. This layer never inspects the output (only the caller's sink
// may parse it) and never interprets the latency (it is a plain duration the caller stamps).
func (c CommandExecutor) observe(phase, output string, latency time.Duration) {
	if c.Observe != nil {
		c.Observe(phase, output, latency)
	}
}

// now returns the current time through the injectable clock, defaulting to time.Now when
// Now is unset — the nil-safe twin of Engine.sleep, so production measures latency on the
// real wall clock and a test injects a deterministic fake.
func (c CommandExecutor) now() time.Time {
	if c.Now != nil {
		return c.Now()
	}
	return time.Now()
}

// renderForLog applies the optional output transform for the Log line, defaulting
// to identity when RenderLog is nil — so a non-rendering caller (and every test
// using a plain command like echo) logs the raw output exactly as before.
func (c CommandExecutor) renderForLog(output string) string {
	if c.RenderLog != nil {
		return c.RenderLog(output)
	}
	return output
}

// currentAgentDepth reads the inherited FORGE_AGENT_DEPTH. A missing or malformed
// value reads as 0 (fail-safe: a garbage env must never block a legitimate
// top-level run — only an honest positive counter raises the guard).
//
// SCOPE (honest): this guard bounds ACCIDENTAL recursion — real nesting propagates
// an honestly-incremented integer, never garbage. It does NOT defend against an
// agent that maliciously rewrites FORGE_AGENT_DEPTH (garbage resets to 0 by
// design); an agent with arbitrary env control already has other escapes, so
// hardening to fail-secure would buy no real safety while breaking honest runs.
func currentAgentDepth() int {
	d, err := strconv.Atoi(os.Getenv(agentDepthEnv))
	if err != nil || d < 0 {
		return 0
	}
	return d
}

// maxDepth is the effective recursion cap: the configured MaxDepth, or the safe
// default when unset/non-positive.
func (c CommandExecutor) maxDepth() int {
	if c.MaxDepth > 0 {
		return c.MaxDepth
	}
	return defaultMaxAgentDepth
}
