package orchestrator

import (
	"context"
	"errors"
	"fmt"
	"os/exec"
)

// ExecKind classifies why an agent command failed. The orchestrator's retry and
// escalation logic keys off the *kind*, not the message text: a timeout is a
// transient hiccup worth retrying, a missing binary or empty argv is a permanent
// misconfiguration that retrying can only waste time on, and a clean non-zero
// exit is the agent itself reporting failure. Classifying at the executor — the
// only layer that can see exec.ErrNotFound vs context.DeadlineExceeded vs an
// ExitError — keeps that judgement out of the caller's guesswork.
type ExecKind int

const (
	// KindConfig is a permanent, operator-fixable fault: no Build configured, an
	// empty argv, or a command binary that does not exist. Never retryable —
	// the same input will fail identically.
	KindConfig ExecKind = iota
	// KindTimeout is a transient fault: the command exceeded its deadline and was
	// killed. Retryable, since a slow agent may succeed on a later attempt.
	KindTimeout
	// KindFailed is a clean run with a non-zero exit: the command launched and
	// ran to completion but reported failure. Treated as terminal here (the
	// agent decided it failed); not auto-retryable.
	KindFailed
)

// String renders the kind for logs and error messages.
func (k ExecKind) String() string {
	switch k {
	case KindConfig:
		return "config"
	case KindTimeout:
		return "timeout"
	case KindFailed:
		return "failed"
	default:
		return "unknown"
	}
}

// ExecError is the single typed error every CommandExecutor.Execute failure
// path returns, so callers can errors.As it out and branch on Kind / Retryable
// instead of string-matching. Phase names the failing phase for diagnosis, and
// Err preserves the underlying cause (exec.ErrNotFound, an *exec.ExitError, a
// context deadline error) for errors.Is / errors.As to keep working through it.
type ExecError struct {
	Phase string   // workflow phase whose command failed
	Kind  ExecKind // classification driving retry/escalation
	Err   error    // wrapped underlying cause; may be nil for pure-config faults
}

// Error renders a phase-scoped, kind-tagged message; the wrapped cause is
// appended when present so a single line is enough to triage.
func (e *ExecError) Error() string {
	if e.Err != nil {
		return fmt.Sprintf("phase %s: %s: %v", e.Phase, e.Kind, e.Err)
	}
	return fmt.Sprintf("phase %s: %s", e.Phase, e.Kind)
}

// Unwrap exposes the underlying cause so errors.Is/As traverse into it (e.g.
// errors.Is(err, exec.ErrNotFound) still holds after wrapping).
func (e *ExecError) Unwrap() error { return e.Err }

// Retryable reports whether re-running the same command could plausibly succeed.
// Only timeouts qualify: config faults are deterministic and a non-zero exit is
// the agent's own verdict, so retrying either just burns a turn.
func (e *ExecError) Retryable() bool { return e.Kind == KindTimeout }

// configErr builds a KindConfig failure for the given phase, wrapping cause
// (which may be nil for argv/Build faults that have no underlying error value).
func configErr(phase string, cause error) *ExecError {
	return &ExecError{Phase: phase, Kind: KindConfig, Err: cause}
}

// classifyRunErr maps the error from running a command to an *ExecError. The
// order matters: a missing binary surfaces as exec.ErrNotFound and is permanent
// config, a tripped deadline is a transient timeout, and anything else (an
// *exec.ExitError for a clean non-zero exit, or any other run error) is Failed.
func classifyRunErr(phase string, runErr, ctxErr error) *ExecError {
	switch {
	case errors.Is(runErr, exec.ErrNotFound):
		return &ExecError{Phase: phase, Kind: KindConfig, Err: runErr}
	case errors.Is(ctxErr, context.DeadlineExceeded):
		// Report the deadline as the cause: the kill manifests as a generic
		// "signal: killed" run error, but the deadline is the real reason.
		return &ExecError{Phase: phase, Kind: KindTimeout, Err: ctxErr}
	default:
		return &ExecError{Phase: phase, Kind: KindFailed, Err: runErr}
	}
}
