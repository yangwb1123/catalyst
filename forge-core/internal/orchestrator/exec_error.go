package orchestrator

import (
	"context"
	"errors"
	"fmt"
	"os/exec"
)

// ExecKind classifies why an agent command or runtime boundary failed. Retry
// and escalation key off the *kind*, not message text. A timeout is a transient
// hiccup worth retrying; a missing binary or empty argv is a permanent
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
	// KindRecursionLimit is a permanent guard fault: the inherited agent-call
	// depth reached the cap, so spawning another agent was refused to prevent an
	// unbounded recursive fork-bomb (a real agent re-invoking forge
	// --executor=command). Never retryable — re-running would recurse identically.
	KindRecursionLimit
	// KindOverloaded is a TRANSIENT capacity fault: the agent's backend reported it
	// is momentarily overloaded (a one-shot "try again shortly" condition, semantically
	// like KindTimeout — not the agent's own verdict and not a misconfiguration). Retryable,
	// but unlike a timeout it should be retried AFTER A BACKOFF so the backend can recover
	// (a tight retry just re-hits the same overload). This layer knows only the ABSTRACT
	// "transient overloaded" shape: it does NOT know the fault is a specific vendor's API
	// status (e.g. a claude/Anthropic 529) — that recognition is the caller's job, handed
	// in as a bool (see classifyRunErr's isOverload parameter and CommandExecutor.ClassifyOverload).
	KindOverloaded
	// KindRuntimeValidation is a caller-owned freshness or verdict binding
	// rejection. It is fail-closed and never retryable inside a phase.
	KindRuntimeValidation
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
	case KindRecursionLimit:
		return "recursion-limit"
	case KindOverloaded:
		return "overloaded"
	case KindRuntimeValidation:
		return "runtime-validation"
	default:
		return "unknown"
	}
}

// ExecError is the typed phase error returned by CommandExecutor and Engine
// runtime-validation boundaries. Callers can errors.As it and branch on Kind /
// Retryable instead of parsing the full message. Err preserves the cause for
// errors.Is / errors.As traversal.
type ExecError struct {
	Phase string   // workflow phase whose command or runtime boundary failed
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
// Two kinds qualify, both transient: a KindTimeout (a slow agent may finish on a
// later attempt) and a KindOverloaded (the backend was momentarily at capacity and
// may have recovered). Config faults are deterministic and a non-zero KindFailed is
// the agent's own verdict, so retrying either just burns a turn. Retryable is only
// the GATE — the orchestrator additionally backs off before retrying an overload (the
// timeout already consumed its deadline, so it is retried immediately); see runAgentPhase.
func (e *ExecError) Retryable() bool {
	return e.Kind == KindTimeout || e.Kind == KindOverloaded
}

// configErr builds a KindConfig failure for the given phase, wrapping cause
// (which may be nil for argv/Build faults that have no underlying error value).
func configErr(phase string, cause error) *ExecError {
	return &ExecError{Phase: phase, Kind: KindConfig, Err: cause}
}

// recursionErr builds a KindRecursionLimit failure: the agent-call nesting reached
// the cap, so another spawn was refused. Non-retryable (Retryable honors only
// KindTimeout), so the orchestrator aborts fail-closed rather than recursing.
func recursionErr(phase string, depth, max int) *ExecError {
	return &ExecError{
		Phase: phase,
		Kind:  KindRecursionLimit,
		Err:   fmt.Errorf("agent-call depth %d reached cap %d", depth, max),
	}
}

// overloadErr builds a KindOverloaded failure: the backend reported a transient
// overload. Retryable (Retryable honors KindOverloaded), so the orchestrator backs
// off and re-attempts rather than aborting. Symmetric to recursionErr/configErr, it
// wraps the underlying run error so errors.Is/As still reach the original cause.
func overloadErr(phase string, cause error) *ExecError {
	return &ExecError{Phase: phase, Kind: KindOverloaded, Err: cause}
}

// outputTruncatedErr uses the existing typed terminal ExecError vocabulary.
// The retained prefix is useful for human logs only: it is not a complete agent
// result and must never cross a machine-output contract or accepted commit.
func outputTruncatedErr(phase string, retained int, total int64) *ExecError {
	return &ExecError{
		Phase: phase,
		Kind:  KindFailed,
		Err: fmt.Errorf(
			"command output exceeded retention limit: retained %d of %d child bytes",
			retained, total,
		),
	}
}

// classifyRunErr maps the error from running a command to an *ExecError. The order
// matters: a missing binary surfaces as exec.ErrNotFound and is permanent config; a
// tripped deadline is a transient timeout; a caller-detected transient overload (isOverload,
// e.g. a vendor "try again" capacity signal) is retryable-with-backoff; and anything else
// (an *exec.ExitError for a clean non-zero exit, or any other run error) is Failed.
//
// TIMEOUT TAKES PRECEDENCE OVER OVERLOAD (deadline checked before isOverload): a command
// SIGKILLed at its deadline yields truncated output that may incidentally contain an
// overload marker the caller's detector trips on, but the real cause is the deadline — a
// timeout (retried immediately) not an overload (retried after a backoff). notfound stays
// first (it is permanent config and must never be mistaken for a transient retry).
//
// isOverload is a pure boolean the caller supplies; this layer does NOT inspect the output
// or know what a vendor overload looks like — it only branches on the verdict handed in,
// keeping all vendor (e.g. claude 529) recognition out of this generic layer.
func classifyRunErr(phase string, runErr, ctxErr error, isOverload bool) *ExecError {
	switch {
	case errors.Is(runErr, exec.ErrNotFound):
		return &ExecError{Phase: phase, Kind: KindConfig, Err: runErr}
	case errors.Is(ctxErr, context.DeadlineExceeded):
		// Report the deadline as the cause: the kill manifests as a generic
		// "signal: killed" run error, but the deadline is the real reason.
		return &ExecError{Phase: phase, Kind: KindTimeout, Err: ctxErr}
	case isOverload:
		return overloadErr(phase, runErr)
	default:
		return &ExecError{Phase: phase, Kind: KindFailed, Err: runErr}
	}
}
