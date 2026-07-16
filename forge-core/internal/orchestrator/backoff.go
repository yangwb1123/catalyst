package orchestrator

import (
	"context"
	"errors"
	"fmt"
	"time"

	"forgeos/forge-core/internal/asset"
)

// backoff.go holds the per-phase RETRY LOOP and the inter-retry backoff policy the orchestrator
// applies before re-attempting a KindOverloaded agent phase (the 529/overload resilience pause).
// It is GENERIC: it knows only the abstract "retry the transient kinds; wait — growing, bounded —
// before an overload retry; then give up" shape — nothing about claude, Anthropic, or an HTTP 529
// (that vendor recognition lives in cmd/forge). Split out of orchestrator.go so neither file grows
// past the volume ceiling; runAgentPhase is invoked from RunFrom there.

// runAgentPhase executes one agent phase, retrying ONLY on retryable failures up to MaxRetries.
// The first attempt always runs; each subsequent attempt is a retry, taken only when the last error
// errors.As's to an *ExecError whose Retryable() is true AND the retry budget is not yet spent. A
// non-ExecError or any non-retryable ExecError (KindConfig, KindFailed) aborts immediately — the
// pre-retry behavior — and so does exhausting the budget, returning the LAST error so the operator
// sees the final failure, not a stale earlier one.
//
// BACKOFF (529/overload resilience): a KindOverloaded retry waits overloadBackoff(attempt) before
// re-attempting, giving an overloaded backend time to recover instead of hammering it in a tight
// loop. A KindTimeout retry does NOT wait — it already consumed its whole deadline (we don't
// blanket-sleep every retryable kind). The backoff is BOUNDED: charged against the SAME MaxRetries
// budget, so a persistently overloaded backend exhausts the budget and aborts — never unbounded.
func (e Engine) runAgentPhase(ctx context.Context, p asset.Phase, mode string) error {
	if e.Exec == nil {
		return fmt.Errorf("phase %s: no agent executor configured (fail closed)", p.Name)
	}
	for attempt := 0; ; attempt++ {
		// Before each attempt, check if the parent context has been cancelled
		// (e.g. SIGINT). This avoids starting a long-running phase that would
		// immediately be torn down.
		select {
		case <-ctx.Done():
			return fmt.Errorf("phase %s: cancelled before attempt %d: %w", p.Name, attempt+1, ctx.Err())
		default:
		}
		err := e.Exec.Execute(ctx, p, mode)
		if err == nil {
			return nil
		}
		var execErr *ExecError
		if !errors.As(err, &execErr) || !execErr.Retryable() || attempt >= e.MaxRetries {
			return fmt.Errorf("phase %s: agent execution failed: %w", p.Name, err)
		}
		if execErr.Kind == KindOverloaded {
			d := overloadBackoff(attempt)
			e.logf("phase %s: overloaded, backing off %s before retry %d/%d", p.Name, d, attempt+1, e.MaxRetries)
			e.sleep(ctx, d)
			continue
		}
		e.logf("phase %s: retryable %s, retry %d/%d", p.Name, execErr.Kind, attempt+1, e.MaxRetries)
	}
}

// overloadBackoffBase / overloadBackoffCap bound the exponential overload backoff. base=2s
// because a transient backend overload (e.g. a claude/Anthropic 529 overloaded_error) typically
// clears in a few seconds; cap=60s because past roughly a minute a still-overloaded backend is
// better surfaced by exhausting the retry budget than by waiting longer. (v1 single-run: NO JITTER
// — jitter only matters once many agents retry in parallel and could thunder-herd the backend in
// lockstep; that is a multi-agent concern deferred to ROADMAP direction five. A single run's serial
// retries cannot self-collide.)
const (
	overloadBackoffBase = 2 * time.Second
	overloadBackoffCap  = 60 * time.Second
)

// overloadBackoff is the PURE exponential-backoff schedule for an overload retry: base<<attempt,
// capped at overloadBackoffCap. attempt is 0-indexed (the first retry is attempt 0 -> base). The
// shift is guarded against overflow so a large attempt count saturates at the cap rather than
// wrapping to a tiny or negative duration. Deterministic (no jitter, no clock read), so a test
// asserts the exact 2s,4s,8s,… sequence.
func overloadBackoff(attempt int) time.Duration {
	if attempt < 0 {
		attempt = 0
	}
	// 62 keeps base<<attempt within int64 nanoseconds; anything larger is already past the cap.
	if attempt >= 62 {
		return overloadBackoffCap
	}
	d := overloadBackoffBase << attempt
	if d <= 0 || d > overloadBackoffCap {
		return overloadBackoffCap
	}
	return d
}

// sleep performs the inter-retry pause via the injected Engine.Sleep, defaulting to a
// context-cancellable wait when unset — the nil-safe twin of logf, so production waits on the
// real clock (but returns EARLY the moment ctx is cancelled, e.g. SIGINT, rather than riding out
// the full up-to-60s backoff before runAgentPhase's next-attempt loop notices) and a test injects
// a fake that records the duration without sleeping (and without needing to be context-aware,
// since it never blocks in the first place).
func (e Engine) sleep(ctx context.Context, d time.Duration) {
	if e.Sleep != nil {
		e.Sleep(d)
		return
	}
	t := time.NewTimer(d)
	defer t.Stop()
	select {
	case <-t.C:
	case <-ctx.Done():
	}
}
