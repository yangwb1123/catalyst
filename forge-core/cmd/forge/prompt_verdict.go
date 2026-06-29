// prompt_verdict.go — verdict and findings ledgers for the cmd/forge prompt layer.
// Split from prompt_context.go (file-size budget); both live in package main so
// they share truncateSummary and the VerdictRequestChanges constant from cost.go.
package main

import "sync"

// verdictLedger remembers the latest machine-readable verdict of every AGENT phase
// that emitted one (today: the reviewer's APPROVE/REQUEST_CHANGES), keyed by phase
// name. It is the cmd/forge end of Engine.AgentVerdict's PULL wire — the reverse twin
// of gateLedger (which serves the OnGateResult PUSH): the Observe sink writes it via
// record, and the orchestrator reads it back via get. Storing the NORMALIZED token
// (not the raw output) keeps the engine vendor-free.
//
// CONCURRENCY: mu guards verdict for the OPT-IN parallel orchestrator (uncontended,
// byte-for-byte unchanged on the serial path).
type verdictLedger struct {
	mu      sync.Mutex
	verdict map[string]string // phase name -> latest normalized verdict token
}

// newVerdictLedger returns an empty ledger ready to record agent verdicts.
func newVerdictLedger() *verdictLedger { return &verdictLedger{verdict: map[string]string{}} }

// record stores one phase's latest verdict (the normalized APPROVE/REQUEST_CHANGES
// token), overwriting a prior one so a re-run reviewer's NEWEST verdict wins. Safe on a
// nil receiver (no-op) so a run that never built a ledger cannot panic.
func (l *verdictLedger) record(phase, verdict string) {
	if l == nil {
		return
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	l.verdict[phase] = verdict
}

// get reads back a phase's recorded verdict for Engine.AgentVerdict: (token, true) when
// one was recorded, ("", false) when none was — nil-safe, so an unwired ledger reports
// "no verdict" and the orchestrator proceeds (fail-open), never loops or panics.
func (l *verdictLedger) get(phase string) (string, bool) {
	if l == nil {
		return "", false
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	v, ok := l.verdict[phase]
	return v, ok
}

// wasReworked returns true when any phase recorded a REQUEST_CHANGES verdict —
// the "rework" signal for trajectory-aware scorecard updates and the Reflect step.
// Nil-safe (no verdicts = no rework). Used by the wind-down to carry the real
// reviewer-bounce signal into the learning loop's trajectory row.
func (l *verdictLedger) wasReworked() bool {
	if l == nil {
		return false
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	for _, v := range l.verdict {
		if v == VerdictRequestChanges {
			return true
		}
	}
	return false
}

// reviewFindingsLedger carries the reviewer's findings text BACKWARD across a directed
// loop-back, to be injected into the IMPLEMENTER's next prompt for targeted repair. It
// is a deliberately ONE-DIRECTION edge: keyed by the loop-back TARGET phase (the
// implementer, read from the reviewer phase's on_fail.target — data-driven, zero
// hard-coded agent name), and buildPrompt injects it ONLY into that target phase. The
// reviewer, when it re-runs, has p.Name != target, so it NEVER receives these findings —
// preserving its fresh-context independence (the D3/AGENTS red line: a reviewer must not
// read its own prior self-report).
//
// CONCURRENCY: mu guards findings for the OPT-IN parallel orchestrator, as the sibling
// ledgers do (uncontended on the serial path).
type reviewFindingsLedger struct {
	mu       sync.Mutex
	findings map[string]string // loop-back TARGET phase -> latest (truncated) findings
}

// newReviewFindingsLedger returns an empty ledger ready to record reviewer findings.
func newReviewFindingsLedger() *reviewFindingsLedger {
	return &reviewFindingsLedger{findings: map[string]string{}}
}

// record stores the reviewer's findings for the loop-back TARGET phase (its recipient),
// TRUNCATED to phaseOutputSummaryCap so a verbose review cannot bloat the implementer's
// prompt — reusing the same cap/marker as phaseOutputLedger. A re-review overwrites with
// the newest findings. Safe on a nil receiver (no-op).
func (l *reviewFindingsLedger) record(targetPhase, findings string) {
	if l == nil {
		return
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	l.findings[targetPhase] = truncateSummary(findings)
}

// contextLines renders the findings recorded for the phase named `name` as a single
// appendable prompt block, or nil when none were recorded for it (the common case — the
// gate is in buildPrompt, which only consults this for the implementer). The text is
// HONEST about provenance: the previous fresh-context Reviewer's per-finding
// REQUEST_CHANGES notes, offered for TARGETED repair — explicitly NOT a gate verdict.
func (l *reviewFindingsLedger) contextLines(name string) []string {
	if l == nil {
		return nil
	}
	l.mu.Lock()
	f, ok := l.findings[name]
	l.mu.Unlock()
	if !ok || f == "" {
		return nil
	}
	return []string{"上一轮 fresh-context Reviewer 判 REQUEST_CHANGES 的逐条 findings(供定向修复参考;非闸门结果,是上游审查角色的判断):\n\n" + f}
}

// allFindings returns a snapshot copy of all recorded (targetPhase→findings) entries
// for the Reflect step: structured memory extraction after an iteration completes.
// Returns nil when the ledger is empty or nil. The copy lets callers safely read
// after the ledger's next write proceeds.
func (l *reviewFindingsLedger) allFindings() map[string]string {
	if l == nil {
		return nil
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	if len(l.findings) == 0 {
		return nil
	}
	out := make(map[string]string, len(l.findings))
	for k, v := range l.findings {
		out[k] = v
	}
	return out
}
