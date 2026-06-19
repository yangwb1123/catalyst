package orchestrator

import (
	"context"
	"fmt"
	"os/exec"
	"strings"
	"time"

	"forgeos/forge-core/internal/asset"
)

// CommandExecutor runs an external command per agent phase — the real bridge
// beyond DryRunExecutor. Build produces the argv for a phase under a mode
// (e.g. ["claude", "-p", prompt]); pointing it at a real agent CLI is how
// forge-core graduates from narrating to actually driving agents. It is
// verified with stub commands here; driving a real LLM agent additionally
// needs that CLI plus credentials in the environment.
type CommandExecutor struct {
	// Build returns the argv to run for a phase. An empty result is an error.
	Build func(p asset.Phase, mode string) []string
	// Timeout bounds a single command's wall-clock runtime. A zero value means
	// no deadline (the backward-compatible default): an agent that hangs would
	// hang the orchestrator. Set it so a wedged agent is killed and surfaces as
	// a retryable Timeout instead of stalling the whole spine.
	Timeout time.Duration
	Log     func(string)
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

	// A zero Timeout means "no deadline": context.WithTimeout(0) would fire
	// immediately, so fall back to a plain cancelable context in that case. The
	// cancel is always deferred to release the timer/goroutine on every return.
	ctx, cancel := c.commandContext()
	defer cancel()

	// On timeout/cancel, exec.CommandContext sends SIGKILL to the DIRECT child
	// only — it does not kill a process group, so any GRANDCHILDREN the child
	// spawned can outlive the kill (an orphaned subtree). For the `claude -p`
	// use-case the direct child IS the agent, so killing it is sufficient. If a
	// future agent forks its own subprocesses, switch to a process-group kill
	// (SysProcAttr{Setpgid: true} + signal -pgid, or Cancel/WaitDelay) so the
	// whole tree dies with the deadline.
	out, runErr := exec.CommandContext(ctx, argv[0], argv[1:]...).CombinedOutput()
	c.logf("phase %s: ran %q -> %s", p.Name, strings.Join(argv, " "), strings.TrimSpace(string(out)))
	if runErr != nil {
		// Pass ctx.Err() too: a timeout kill shows up as a generic run error, so
		// the context is the only place that knows the deadline actually tripped.
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
