package orchestrator

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"strconv"
	"strings"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/orchestrator/sandbox"
)

// defaultMaxAgentDepth bounds nested agent spawns before the recursion guard
// fires (see CommandExecutor.MaxDepth). 2 permits one legitimate level of
// sub-agent orchestration (a top-level run whose agent drives one nested run);
// the 3rd spawn is refused.
const defaultMaxAgentDepth = 2

// agentDepthEnv names the inherited counter tracking agent-call nesting across
// process boundaries; each spawn sets it to parent+1 on the child's environment.
const agentDepthEnv = "FORGE_AGENT_DEPTH"

// defaultMaxOutputBytes caps the stdout+stderr the executor RETAINS from one agent
// command (see CommandExecutor.MaxOutputBytes). 10 MiB is generous for a phase's
// log yet bounds a runaway agent's output so it cannot OOM the orchestrator the way
// an unbounded CombinedOutput would. Override per executor with MaxOutputBytes.
const defaultMaxOutputBytes = 10 << 20

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
	// Zero selects the safe default (defaultMaxOutputBytes, 10 MiB). The resource
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

// SandboxConfig describes how to isolate an agent command. It is the v3 extension
// point; forge-core currently has no runtime implementation and therefore rejects
// every non-"none" Type rather than accidentally executing it on the host.
type SandboxConfig struct {
	Type       string // "" (none) | "firecracker" | "docker"
	Image      string // container/microVM image
	MemoryMB   int    // RAM limit; 0 = use default
	TimeoutSec int    // session timeout; 0 = use command's Timeout
	// Runner is the wired isolation implementation for Type. nil keeps the
	// fail-closed contract: a declared sandbox without a runner is a
	// permanent config error and never falls back to host execution.
	Runner sandbox.Runner
}

// Execute builds and runs the phase's command under an optional timeout, failing
// closed with a typed *ExecError on every error path so a broken executor never
// masquerades as success — and never panics on a nil Build. The error's Kind
// lets callers tell a retryable timeout from a permanent config fault from the
// agent's own non-zero exit (see ExecError). ctx propagates cancellation so a
// parent SIGINT/SIGTERM stops the child process via its process group.
func (c CommandExecutor) Execute(ctx context.Context, p asset.Phase, mode string) error {
	if c.Build == nil {
		return configErr(p.Name, nil) // nil Build: nothing to run, permanent.
	}
	if c.ValidateConfig != nil {
		if err := c.ValidateConfig(p, mode); err != nil {
			return configErr(p.Name, err)
		}
	}
	if err := c.sandboxConfigError(p.Name); err != nil {
		return err
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
		runCtx, runCancel := c.commandContext(ctx)
		defer runCancel()
		timeout := time.Duration(c.Sandbox.TimeoutSec) * time.Second
		return c.executeSandboxed(runCtx, p.Name, argv, timeout)
	}

	// Recursion guard: once the inherited agent-call depth reaches the cap, refuse
	// to spawn — a real agent re-invoking `forge --executor=command` would else
	// fork-bomb unboundedly. Fail closed, NON-retryable (re-running recurses).
	depth := currentAgentDepth()
	if max := c.maxDepth(); depth >= max {
		c.logf("phase %s: recursion guard fired (depth %d >= cap %d) — refusing another agent spawn", p.Name, depth, max)
		return recursionErr(p.Name, depth, max)
	}

	// A zero Timeout means "no deadline": context.WithTimeout(0) would fire at
	// once, so fall back to a plain cancelable context. cancel is always deferred.
	runCtx, runCancel := c.commandContext(ctx)
	defer runCancel()

	out, latency, runErr := c.runMeasured(runCtx, argv, depth, input, useStdin)
	return c.finish(p.Name, argv, out, runErr, runCtx.Err(), latency)
}

// sandboxConfigError enforces the isolation boundary. A declared sandbox is a
// safety requirement, not a hint: until a runtime is wired, falling back to the
// host would violate the workflow contract and must be a permanent config error.
func (c CommandExecutor) sandboxNone() bool {
	if c.Sandbox == nil {
		return true
	}
	runtime := strings.TrimSpace(c.Sandbox.Type)
	return runtime == "" || strings.EqualFold(runtime, "none")
}

func (c CommandExecutor) sandboxConfigError(phase string) error {
	if c.Sandbox == nil {
		return nil
	}
	runtime := strings.TrimSpace(c.Sandbox.Type)
	if runtime == "" || strings.EqualFold(runtime, "none") {
		return nil
	}
	if c.Sandbox.Runner != nil {
		return nil
	}
	return configErr(phase, fmt.Errorf("sandbox %q requested but no sandbox runner is installed; refusing host execution", runtime))
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

// runMeasured constructs the bounded, process-grouped command for argv and runs it under
// ctx, bracketing cmd.Run() with the injectable clock to MEASURE its wall-clock latency. It
// returns the captured output, the run error, and the measured duration. Split out of Execute
// so that stays within the function-length ceiling; the construction and the timing are one
// unit (the latency must bracket exactly this run), so they live together here.
//
// The latency is the real per-phase duration the cost path stamps onto the agent trace event
// so the scorecard's p95 is genuinely per-model (not the iteration-shared span). The clock read
// is OS-level and generic (no claude/vendor knowledge); this layer only knows WHEN the command
// started and finished, never what model it billed.
func (c CommandExecutor) runMeasured(ctx context.Context, argv []string, depth int, input string, useStdin bool) (*cappedBuffer, time.Duration, error) {
	// exec.CommandContext's default cancel SIGKILLs the DIRECT child only. That is
	// insufficient once `claude -p` forks grandchildren via its own tools (Bash ->
	// git/test/build): those grandchildren inherit the command's stdout/stderr pipe,
	// so after the direct child is killed cmd.Run() would block until they close it —
	// a surviving grandchild thus defeats the Timeout. setupProcessGroup (unix) puts
	// the child in its own process group and overrides Cancel to SIGKILL the whole
	// group (-pgid), with a WaitDelay backstop, so the deadline is reliably enforced.
	// On non-unix it is a no-op and the direct-child-only kill stands (honest platform
	// difference; Windows Job Object is future work — see command_executor_other.go).
	cmd := exec.CommandContext(ctx, argv[0], argv[1:]...)
	setupProcessGroup(cmd)
	cmd.Dir = c.Dir // empty -> inherit forge's cwd (os/exec default)
	// Propagate an incremented depth so a nested forge inherits it; childEnv
	// REPLACES any inherited key (duplicate-key resolution is unspecified across libcs).
	cmd.Env = c.childEnv(depth)
	if useStdin {
		cmd.Stdin = strings.NewReader(input)
	}
	// Bound the captured output: a runaway agent's unbounded stdout would OOM the
	// orchestrator under CombinedOutput. cappedBuffer retains at most the cap and
	// drains the rest; the SAME pointer for Stdout+Stderr lets os/exec serialize the
	// writes (no lock needed), exactly as CombinedOutput merges the two streams.
	out := &cappedBuffer{cap: c.maxOutputBytes()}
	cmd.Stdout, cmd.Stderr = out, out
	// Bracket the run with the injectable clock — the wall-clock span is the phase's latency.
	start := c.now()
	runErr := cmd.Run()
	return out, c.now().Sub(start), runErr
}

// finish handles a completed command: it hands the raw output AND measured latency to the
// optional sink, logs a possibly-rendered view, and (on failure) classifies the error —
// consulting the optional overload judge. Split out of Execute so each stays within the
// function-length ceiling; the behavior is unchanged except for the new latency the sink
// receives. ctxErr is ctx.Err() captured at the call site (the deadline cause); latency is
// the wall-clock span bracketing cmd.Run() (see Now).
func (c CommandExecutor) finish(phase string, argv []string, out *cappedBuffer, runErr, ctxErr error, latency time.Duration) error {
	// Both observe and the log renderer are nil-safe and identity-by-default, so the no-hook
	// path is byte-for-byte unchanged.
	observed := out.observed()
	rendered := out.rendered()
	c.observe(phase, observed, latency)
	visible := c.renderForLog(rendered)
	c.logf("phase %s: ran %q -> %s", phase, strings.Join(argv, " "), visible)
	if runErr == nil {
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
	return classifyRunErr(phase, runErr, ctxErr, isOverload)
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

// maxOutputBytes is the effective cap on retained command output: the configured
// MaxOutputBytes, or the safe default when unset/non-positive.
func (c CommandExecutor) maxOutputBytes() int {
	if c.MaxOutputBytes > 0 {
		return c.MaxOutputBytes
	}
	return defaultMaxOutputBytes
}

// cappedBuffer is an io.Writer that retains at most cap bytes of what is written,
// silently discarding the overflow — so a runaway agent's UNBOUNDED stdout/stderr
// cannot OOM the orchestrator the way an unbounded CombinedOutput would. It tracks
// the TOTAL bytes seen so truncation is reported honestly. Write never errors or
// short-writes (a short write would make os/exec treat the pipe as broken and could
// wedge the child mid-stream); it lets the agent run to its natural end (or the
// Timeout) while simply not retaining the excess. Used as BOTH cmd.Stdout and
// cmd.Stderr (the same pointer), os/exec serializes the writes, so no lock is needed.
type cappedBuffer struct {
	cap   int
	buf   []byte
	total int
}

func (b *cappedBuffer) Write(p []byte) (int, error) {
	b.total += len(p)
	if room := b.cap - len(b.buf); room > 0 {
		if len(p) <= room {
			b.buf = append(b.buf, p...)
		} else {
			b.buf = append(b.buf, p[:room]...)
		}
	}
	return len(p), nil
}

// rendered returns the retained output, trimmed, with an honest truncation note
// when the agent wrote more than was retained — so a clipped log is never mistaken
// for the agent's full output.
func (b *cappedBuffer) rendered() string {
	s := strings.TrimSpace(string(b.buf))
	if b.total > len(b.buf) {
		s += fmt.Sprintf(" …[output truncated: retained %d of %d bytes (--max-output-bytes)]", len(b.buf), b.total)
	}
	return s
}

// observed preserves every retained byte for machine parsers. In particular,
// leading/trailing whitespace cannot be erased into an exact verdict token.
// A truncation marker remains non-empty evidence that the capture was incomplete.
func (b *cappedBuffer) observed() string {
	s := string(b.buf)
	if b.total > len(b.buf) {
		s += fmt.Sprintf(" …[output truncated: retained %d of %d bytes (--max-output-bytes)]", len(b.buf), b.total)
	}
	return s
}

// executeSandboxed runs the phase through the wired sandbox runner instead of
// the host shell, then funnels the outcome through the same finish pipeline so
// observation, output contracts, and error classification stay uniform. A
// clean non-zero guest exit becomes a KindFailed run; infrastructure faults
// (config, timeout) map onto their own kinds without fabricating an agent
// verdict.
func (c CommandExecutor) executeSandboxed(
	runCtx context.Context,
	phase string,
	argv []string,
	timeout time.Duration,
) error {
	start := c.now()
	output, code, err := c.Sandbox.Runner.Run(runCtx, argv, timeout)
	latency := c.now().Sub(start)
	out := &cappedBuffer{cap: c.maxOutputBytes()}
	_, _ = out.Write([]byte(output))
	if err != nil {
		if runCtx.Err() != nil || strings.Contains(err.Error(), "timed out") {
			return &ExecError{Phase: phase, Kind: KindTimeout, Err: err}
		}
		return &ExecError{Phase: phase, Kind: KindConfig, Err: err}
	}
	if code != 0 {
		err = &exec.ExitError{}
	}
	return c.finish(phase, argv, out, err, runCtx.Err(), latency)
}
