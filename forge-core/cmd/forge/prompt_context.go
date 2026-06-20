// prompt_context.go — the cmd/forge side of the gate-result feedback loop. The
// orchestrator's Engine fires a GENERIC OnGateResult callback (it knows nothing
// of prompts); THIS file owns the prompt-shaped end of that wire: a gateLedger
// records each gate's latest objective verdict and renders it as a context block
// that buildPrompt injects into a LATER agent phase's prompt.
//
// Why it lives here, not in orchestrator: the orchestrator must stay free of any
// prompt/ledger concept (the same bright-line cost.go draws for claude-JSON cost).
// The engine REPORTS gate verdicts; cmd/forge — the only layer that builds the
// prompt — decides to remember them and feed them forward. This closes a real gap:
// without it the reviewer phase never sees that harness-gates already ran `test`
// and the complexity check, so under a print-mode agent (no Bash) it would blindly
// try to re-run them and burn its budget on permission denials. The injected text
// is the harness's OWN real verdicts (honesty-positive: a true signal, never a
// fabricated pass) — see context().
package main

import "strings"

// gateLedger accumulates the objective verdict of every gate the harness actually
// ran this run, keyed by gate name, preserving first-seen order for a stable
// render. It is the in-memory bridge between Engine.OnGateResult (which writes it,
// one gate at a time) and buildPrompt (which reads context() into a prompt).
//
// CONCURRENCY: this is deliberately lock-free. The orchestrator runs phases
// strictly sequentially (one phase, then the next; evolve drives RunFrom one
// iteration at a time), so record and context never race — there is no concurrent
// gate execution to guard against. Should the engine ever parallelize phases, this
// premise breaks and the ledger would need a mutex.
type gateLedger struct {
	status map[string]string // gate name -> latest verdict ("ok" | "N/A" | "FAILED")
	order  []string          // gate names in first-seen order (stable rendering)
}

// newGateLedger returns an empty ledger ready to record gate verdicts.
func newGateLedger() *gateLedger {
	return &gateLedger{status: map[string]string{}}
}

// record stores one gate's latest verdict, appending to the render order only the
// FIRST time a name is seen (a re-run gate — e.g. across loop-back or evolve
// iterations — updates its status in place, keeping its original position). This is
// the method wired to Engine.OnGateResult. Safe on a nil receiver (no-op) so a
// caller that never constructed a ledger cannot panic.
func (l *gateLedger) record(name, status string) {
	if l == nil {
		return
	}
	if _, seen := l.status[name]; !seen {
		l.order = append(l.order, name)
	}
	l.status[name] = status
}

// context renders the recorded verdicts as a prompt context block, or "" when the
// ledger is nil or empty (no gate has run yet — the prior phases were all agent
// phases, or this is the first phase). The text states plainly that these are the
// harness's REAL, already-run results so a print-mode agent (no Bash) neither needs
// nor is able to re-run them — the whole point is to stop a reviewer from wasting
// its turn on permission-denied re-runs of checks the harness already objectively
// settled. Each gate renders as "- <name>: <verdict>" in first-seen order.
func (l *gateLedger) context() string {
	if l == nil || len(l.order) == 0 {
		return ""
	}
	var b strings.Builder
	b.WriteString("前序闸门结果(harness-gates 本轮已真实运行,客观事实 —— 你在 print 模式无 Bash,无需也无法自行重跑):")
	for _, name := range l.order {
		b.WriteString("\n- ")
		b.WriteString(name)
		b.WriteString(": ")
		b.WriteString(l.status[name])
	}
	b.WriteString("\n据此 + 代码 diff 静态判断;客观信号已由 harness 提供,勿试图自行执行命令验证。")
	return b.String()
}

// contextLines wraps context() as a slice ready to append onto a prompt's context
// lanes (nil when there is nothing to inject — symmetric with memoryContext), so
// buildPrompt stays a flat sequence of appends with no inline empty-check.
func (l *gateLedger) contextLines() []string {
	if c := l.context(); c != "" {
		return []string{c}
	}
	return nil
}
