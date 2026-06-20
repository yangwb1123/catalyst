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
	Log            func(string)
}

// Execute builds and runs the phase's command under an optional timeout, failing
// closed with a typed *ExecError on every error path so a broken executor never
// masquerades as success — and never panics on a nil Build. The error's Kind
// lets callers tell a retryable timeout from a permanent config fault from the
// agent's own non-zero exit (see ExecError).
func (c CommandExecutor) Execute(p asset.Phase, mode string) error {
	if c.Build == nil {
		return configErr(p.Name, nil) // nil Build: nothing to run, permanent.
	}
	argv := c.Build(p, mode)
	if len(argv) == 0 {
		return configErr(p.Name, nil) // empty argv: misconfigured, permanent.
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
	ctx, cancel := c.commandContext()
	defer cancel()

	// CommandContext SIGKILLs the DIRECT child only (no process-group kill); for
	// `claude -p` the direct child IS the agent, so killing it suffices. A future
	// agent that forks grandchildren would need SysProcAttr{Setpgid} + -pgid.
	cmd := exec.CommandContext(ctx, argv[0], argv[1:]...)
	cmd.Dir = c.Dir // empty -> inherit forge's cwd (os/exec default)
	// Propagate an incremented depth so a nested forge inherits it; childEnv
	// REPLACES any inherited key (duplicate-key resolution is unspecified across libcs).
	cmd.Env = childEnv(depth)
	// Bound the captured output: a runaway agent's unbounded stdout would OOM the
	// orchestrator under CombinedOutput. cappedBuffer retains at most the cap and
	// drains the rest; the SAME pointer for Stdout+Stderr lets os/exec serialize the
	// writes (no lock needed), exactly as CombinedOutput merges the two streams.
	out := &cappedBuffer{cap: c.maxOutputBytes()}
	cmd.Stdout, cmd.Stderr = out, out
	runErr := cmd.Run()
	c.logf("phase %s: ran %q -> %s", p.Name, strings.Join(argv, " "), out.rendered())
	if runErr != nil {
		return classifyRunErr(p.Name, runErr, ctx.Err())
	}
	return nil
}

// commandContext returns the context governing one command run, plus its cancel.
// With a positive Timeout it carries a deadline; with the zero default it is
// merely cancelable (no deadline) to preserve the original unbounded behavior.
func (c CommandExecutor) commandContext() (context.Context, context.CancelFunc) {
	if c.Timeout > 0 {
		return context.WithTimeout(context.Background(), c.Timeout)
	}
	return context.WithCancel(context.Background())
}

func (c CommandExecutor) logf(format string, args ...any) {
	if c.Log != nil {
		c.Log(fmt.Sprintf(format, args...))
	}
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

// childEnv returns the parent environment with FORGE_AGENT_DEPTH set to depth+1,
// REPLACING any inherited value rather than appending a duplicate key. POSIX leaves
// duplicate-key resolution unspecified and libcs differ (glibc's getenv returns the
// LAST occurrence, some others the first), so collapsing to a single key is the only
// choice correct under all of them — the child unambiguously observes the
// incremented agent-call depth regardless of the platform's getenv.
func childEnv(depth int) []string {
	prefix := agentDepthEnv + "="
	base := os.Environ()
	out := make([]string, 0, len(base)+1)
	for _, kv := range base {
		if !strings.HasPrefix(kv, prefix) {
			out = append(out, kv)
		}
	}
	return append(out, fmt.Sprintf("%s=%d", agentDepthEnv, depth+1))
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
